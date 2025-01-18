use bytemuck::{bytes_of, NoUninit};
use core::{marker::PhantomData, num::NonZero};
use std::sync::Arc;
use wgpu::{
    util::StagingBelt, Buffer, BufferAddress, BufferUsages, BufferViewMut,
    CommandBuffer, CommandEncoder, CommandEncoderDescriptor, Device,
};

/// Efficient, chunked-copy buffer, to build up a GPU array of `T` values.
///
/// In the line renderer, a common operation involves creating buffers for the
/// GPU by appending values. `PushBuf` does this efficiently.
///
/// # Constant Generic Parameters
///
/// - `T`: The type of value stored in the array contained within the `PushBuf`
///   buffer.
///
/// # Lifecycle
///
/// The lifecycle of `PushBuf` is as follows:
///
/// 1. Create a `PushBuf` using [`PushBuf::new`].
/// 2. Call [`PushBuf::begin_frame`] to start each frame.
/// 3. Append items within a frame using [`PushBuf::push`].
/// 4. Finish the frame using [`PushBuf::end_frame`] and receive a
///    `CommandBuffer` to be enqueued.
/// 5. Use the [`PushBuf::buffer`] (for example, in a binding).
/// 6. Enqueue the `CommandBuffer` (not a `PushBuf` method).
/// 7. Call [`PushBuf::recall`] to fetch the staging belt buffers back from
///    the GPU to host memory.
/// 8. Go back to start the next frame.
///
/// # Internal Operation
///
/// The idea is simple: accumulate chunks of data in a staging buffer. When the
/// staging buffer is full, queue a copy of that data to the main buffer.
///
/// Efficiency is gained through two means:
///
/// 1. Staging buffers are mapped to host memory. This means that, when you
///    call [`PushBuf::push`], bytes are copied directly into a buffer
///    which will be sent to the GPU. There is no additional intermediate
///    buffer involved. (Although there is a copy from the staging buffer
///    to the main buffer, that copy is done on the GPU.)
///
/// 2. Enough staging buffers are allocated by the `StagingBelt`, after the
///    first few frames, that you can expect fresh buffers for the current
///    frame to be mapped to host memory as soon as the frame begins
///    processing.
///
pub struct PushBuf<T> {
    /// WGPU Device.
    device: Arc<Device>,
    /// Command encoder for a frame. Between frames, this will be `None`.
    encoder: Option<CommandEncoder>,
    /// WGPU Buffer we ultimately copy our values into.
    buffer: Buffer,
    /// Number of items of type `T` that can fit in the buffer.
    buffer_item_capacity: usize,
    /// Byte offset in `buffer` for the next chunk.
    buffer_byte_offset: usize,
    /// View into the staging buffer (NOT `buffer` above), obtained from the
    /// `belt`.
    view: Option<BufferViewMut<'static>>,
    /// Byte offset into `view`, for pushing values.
    view_byte_offset: usize,
    /// Total number of pushed items.
    item_count: usize,
    /// The staging belt which produces staging buffers.
    belt: StagingBelt,
    /// Number of items of type `T` that can fit in a chunk.
    chunk_item_capacity: usize,
    /// Debugging state.
    #[cfg(debug_assertions)]
    state: State,
    _phantom: PhantomData<T>,
}

impl<T> PushBuf<T>
where
    T: NoUninit,
{
    /// Creates a new `PushBuf`.
    ///
    /// # Parameters
    ///
    /// - `device`: WGPU Device.
    /// - `label`: Label for the main buffer into which values are written.
    /// - `usage`: Use of the buffer. `BufferUsages::COPY_DST` will always
    ///   be included here.
    /// - `buffer_item_capacity`: Number of items of type `T` that can fit
    ///   in the buffer.
    /// - `chunk_item_capacity`: Number of items of type `T` that can fit
    ///   in the staging buffer.
    pub fn new(
        device: Arc<Device>,
        label: Option<&str>,
        usage: BufferUsages,
        buffer_item_capacity: usize,
        chunk_item_capacity: usize,
    ) -> Self {
        debug_assert!(chunk_item_capacity > 0);
        debug_assert!(buffer_item_capacity > 0);
        debug_assert!(chunk_item_capacity <= buffer_item_capacity);

        PushBuf {
            device: device.clone(),
            encoder: None,
            buffer: create_buffer::<T>(
                device.clone(),
                label,
                usage,
                buffer_item_capacity,
            ),
            buffer_item_capacity,
            buffer_byte_offset: 0,
            view: None,
            view_byte_offset: 0,
            item_count: 0,
            belt: create_staging_belt::<T>(chunk_item_capacity),
            chunk_item_capacity,
            #[cfg(debug_assertions)]
            state: State::Created,
            _phantom: PhantomData,
        }
    }

    /// Returns a reference to the underlying WGPU buffer.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Returns the number of items that have been pushed to the buffer in
    /// the current frame.
    pub fn len(&self) -> usize {
        self.item_count
    }

    /// Begins rendering a frame.
    ///
    /// This should be called at the start of rendering a frame. Internally,
    /// it creates a WGPU `CommandEncoder` to manage the buffer operations
    /// for this frame.
    pub fn begin_frame(&mut self) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(self.state == State::Created);
            self.check_state();
        }

        self.encoder = Some(self.device.create_command_encoder(
            &CommandEncoderDescriptor {
                label: Some("PushBuf command encoder."),
            },
        ));

        #[cfg(debug_assertions)]
        {
            self.state = State::InFrame;
            self.check_state();
        }
    }

    /// Appends a value to the array inside the buffer.
    ///
    /// Within a single frame, this should be called after
    /// [`PushBuf::begin_frame`], but before [`PushBuf::end_frame`].
    ///
    /// # Parameters
    ///
    /// - `value`: The value to append to the buffer.
    pub fn push(&mut self, value: T) -> Result<(), Error> {
        #[cfg(debug_assertions)]
        {
            debug_assert!(self.state == State::InFrame);
            self.check_state();
        }

        // Check we haven't exceeded the buffer capacity.
        if self.item_count >= self.buffer_item_capacity {
            return Err(Error::CapacityExceeded);
        }

        // If there is no current staging belt buffer view, create one.
        if self.view.is_none() {
            self.create_view();
        }

        // Write the bytes of the value into the staging belt buffer view.
        self.write_view(value);

        // If the staging belt buffer is full, release it back to the GPU.
        if self.view_byte_offset >= self.chunk_size_bytes() {
            self.finish_view();
        }

        #[cfg(debug_assertions)]
        self.check_state();

        Ok(())
    }

    /// Ends a frame.
    ///
    /// This completes the buffer management for a frame, signalling that no
    /// more [`PushBuf::push`] operations will be executed. It returns a
    /// `CommandBuffer`, which must be enqueued before the buffer is used for
    /// rendering.
    ///
    /// After the `CommandBuffer` returned by this operation is enqueued, the
    /// [`PushBuf::recall`] function must be called.
    pub fn end_frame(&mut self) -> CommandBuffer {
        #[cfg(debug_assertions)]
        {
            debug_assert!(self.state == State::InFrame);
            self.check_state();
        }

        if self.view.is_some() {
            self.finish_view();
        }

        self.buffer_byte_offset = 0;
        self.item_count = 0;
        self.belt.finish();

        let return_val = self.encoder.take().unwrap().finish();

        #[cfg(debug_assertions)]
        {
            self.state = State::PostFrame;
            self.check_state();
        }

        return_val
    }

    /// Recalls buffers from the GPU after the `CommandBuffer` has been
    /// enqueued.
    ///
    /// After the `CommandBuffer` returned by [`PushBuf::end_frame`] has been
    /// enqueued, this method should be called. It requests the return of
    /// staging buffers from the GPU so that they can be mapped to host memory
    /// for the next frame.
    ///
    /// This method should be called as soon as possible after the
    /// `CommandBuffer` has been enqueued, and MUST be called before the next
    /// [`PushBuf::begin_frame`] method is called.
    pub fn recall(&mut self) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(self.state == State::PostFrame);
            self.check_state();
        }

        self.belt.recall();

        #[cfg(debug_assertions)]
        {
            self.state = State::Created;
            self.check_state();
        }
    }

    /// Creates a staging buffer and a view onto it.
    ///
    /// This requests a staging buffer from the staging belt, and casts it
    /// to a `BufferViewMut<'static>`, into which we can write bytes.
    fn create_view(&mut self) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(self.state == State::InFrame);
            self.check_state();
        }
        debug_assert!(self.view.is_none());
        debug_assert!(self.item_count < self.buffer_item_capacity);

        // Clamp chunk size at the buffer boundary.
        let remaining_space =
            self.buffer_size_bytes() - self.buffer_byte_offset;
        let chunk_size = NonZero::new(
            self.chunk_size_bytes().min(remaining_space) as BufferAddress,
        )
        .unwrap();

        // Create a view onto the staging buffer chunk.
        let view = self.belt.write_buffer(
            self.encoder.as_mut().unwrap(),
            &self.buffer,
            self.buffer_byte_offset as BufferAddress,
            chunk_size,
            &self.device,
        );

        // SAFETY:
        // We own the buffer memory mapped to the host until
        // `self.belt.finish()` is called.
        let view_static: BufferViewMut<'static> =
            unsafe { core::mem::transmute(view) };

        self.view = Some(view_static);
        self.view_byte_offset = 0;
    }

    /// Releases the current staging buffer.
    ///
    /// When the current staging buffer is full or when the frame has finished,
    /// this releases the buffer back to the staging belt. The staging belt
    /// then releases it back to be copied to GPU memory.
    fn finish_view(&mut self) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(self.state == State::InFrame);
            self.check_state();
        }

        // Update the buffer offset to the start of the next chunk.
        debug_assert!(self.view.is_some());
        let cur_chunk_size_bytes = self.view.take().unwrap().len();
        self.buffer_byte_offset += cur_chunk_size_bytes;

        self.view = None;
        self.view_byte_offset = 0;
        self.belt.finish();
    }

    /// Writes `value` into the current view at the current offset.
    fn write_view(&mut self, value: T) {
        debug_assert!(self.view.is_some());
        debug_assert!(self.chunk_size_bytes() % size_of::<T>() == 0);
        debug_assert!(self.view_byte_offset < self.chunk_size_bytes());
        debug_assert!(self.buffer_byte_offset < self.buffer_size_bytes());
        debug_assert!(self.item_count < self.buffer_item_capacity);

        let s = self.view_byte_offset;
        let e = s + size_of::<T>();
        let buf_chunk: &mut [u8] = &mut (self.view.as_mut().unwrap())[s..e];
        debug_assert_eq!(buf_chunk.len(), size_of::<T>());

        buf_chunk.copy_from_slice(bytes_of(&value));

        self.view_byte_offset = e;
        self.item_count += 1;
    }

    /// Returns the size of a chunk in bytes.
    fn chunk_size_bytes(&self) -> usize {
        self.chunk_item_capacity * size_of::<T>()
    }

    /// Returns the size of the buffer in bytes.
    fn buffer_size_bytes(&self) -> usize {
        self.buffer_item_capacity * size_of::<T>()
    }

    /// Checks some state invariants during debug builds.
    #[cfg(debug_assertions)]
    fn check_state(&self) {
        match self.state {
            State::Created => {
                debug_assert!(self.encoder.is_none());
                debug_assert_eq!(self.buffer_byte_offset, 0);
                debug_assert!(self.view.is_none());
                debug_assert_eq!(self.view_byte_offset, 0);
                debug_assert_eq!(self.item_count, 0);
            }
            State::InFrame => {
                debug_assert!(self.encoder.is_some())
            }
            State::PostFrame => {
                debug_assert!(self.encoder.is_none());
                debug_assert_eq!(self.buffer_byte_offset, 0);
                debug_assert!(self.view.is_none());
                debug_assert_eq!(self.view_byte_offset, 0);
                debug_assert_eq!(self.item_count, 0);
            }
        }
    }
}

/// Errors which can be produced by `PushBuf`.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// The capacity of the buffer would be exceeded.
    CapacityExceeded,
}

/// Creates the main WGPU buffer.
fn create_buffer<T>(
    device: Arc<Device>,
    label: Option<&str>,
    usage: BufferUsages,
    buffer_item_capacity: usize,
) -> Buffer {
    let buffer_size_bytes = buffer_item_capacity * size_of::<T>();
    let usage = BufferUsages::COPY_DST | usage;
    let mapped_at_creation = false;
    let buffer_descriptor = wgpu::BufferDescriptor {
        label,
        size: buffer_size_bytes as BufferAddress,
        usage,
        mapped_at_creation,
    };
    device.create_buffer(&buffer_descriptor)
}

/// Creates the staging belt.
fn create_staging_belt<T>(chunk_item_capacity: usize) -> StagingBelt {
    let chunk_size_bytes = chunk_item_capacity * size_of::<T>();
    StagingBelt::new(chunk_size_bytes as BufferAddress)
}

/// Debugging state.
#[cfg(debug_assertions)]
#[derive(Debug, PartialEq)]
enum State {
    Created,
    InFrame,
    PostFrame,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal::tests::gpu::Gpu;
    use bytemuck::cast_slice;
    use futures::{channel::oneshot, executor::block_on, future::try_join_all};
    use proptest::prelude::*;
    use rand::prelude::*;
    use wgpu::{util::DownloadBuffer, BufferSlice, Maintain, MapMode};

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        /// Test `PushBuf` over multiple frames.
        ///
        /// This test effectively round-trips data to the GPU using a `PushBuf`.
        /// In this test, we run for multiple frames. We create a test `Vec` of
        /// data for each of the frames (`in_data`). Inside the frame, we feed
        /// that frame's test `Vec` to a `PushBuf`. Then we copy the `PushBuf`
        /// output into an output buffer for that frame (`out_buffers`). After
        /// all the frames have completed, we map the `out_buffers` back to the
        /// CPU and compare them to make sure they match the `in_data`.
        #[test]
        fn test_pushbuf_multiframe(
            seed in u64::MIN..u64::MAX,
            n_frames in 1usize..24usize,
            buffer_item_capacity in 1usize..512,
            max_chunk_item_capacity in 1usize..512,
            max_n_items in 1usize..512
        ) {
            let chunk_item_capacity =
                max_chunk_item_capacity.min(buffer_item_capacity);
            let n_items =
                max_n_items.min(buffer_item_capacity);

            // Set up the GPU for this test.
            let gpu = Gpu::new();
            let mut pushbuf = PushBuf::<u64>::new(
                gpu.device.clone(),
                Some("Test PushBuf"),
                BufferUsages::COPY_SRC,
                buffer_item_capacity,
                chunk_item_capacity
            );

            // Create random data for all frames.
            let in_data: Vec<Vec<u64>> = {
                let mut rng = StdRng::seed_from_u64(seed);
                (0..n_frames)
                    .map(|_| (0..n_items).map(|_| rng.gen::<u64>()).collect())
                    .collect()
            };

            // Create one buffer per frame to receive data back from the GPU.
            let out_buffers: Vec<Buffer> =
                (0..n_frames)
                    .map(|i| create_buffer::<u64>(
                        gpu.device.clone(),
                        Some(&format!("Test Output Buffer {}", i)),
                        BufferUsages::MAP_READ,
                        n_items
                    )).collect();

            // Run through the frames, pushing data into the PushBuf, and
            // copying the data to the `out_buffers[i]` for each frame.
            for frame in 0..n_frames {
                // Put the data into the push buffer.
                pushbuf.begin_frame();
                let frame_data = &in_data[frame];
                frame_data
                    .iter()
                    .for_each(|x| {
                        let result = pushbuf.push(*x);
                        assert_eq!(result, Ok(()));
                    });
                let command_buffer = pushbuf.end_frame();

                // Copy data from the push buffer's destination buffer to the
                // output buffer for the test.
                let mut copy_command_encoder =
                    gpu.device.create_command_encoder(
                        &CommandEncoderDescriptor {
                            label: Some(&format!("Test Copy Frame {}", frame))
                        }
                    );
                copy_command_encoder.copy_buffer_to_buffer(
                    pushbuf.buffer(),
                    0,
                    &out_buffers[frame],
                    0,
                    (n_items * size_of::<u64>()) as BufferAddress
                );
                let copy_command = copy_command_encoder.finish();

                // Queue both operations for this frame.
                gpu.queue.submit([command_buffer, copy_command]);

                // Recall the push buffer.
                pushbuf.recall();
            }

            // Map all the output buffers back to the CPU, so that we can
            // check they received the correct data.
            let buffer_slices: Vec<BufferSlice<'_>> =
                out_buffers.iter().map(|buf| buf.slice(..)).collect();
            let receivers: Vec<oneshot::Receiver<()>> =
                buffer_slices
                    .iter()
                    .map(|slice| {
                        let(sender, receiver) = oneshot::channel();
                        slice
                            .map_async(
                                MapMode::Read,
                                |result| sender.send(result.unwrap()).unwrap()
                            );
                        receiver
                    })
            .collect();
            let _ = gpu.device.poll(Maintain::Wait);  // Triggers mapping.
            _ = block_on(async{ try_join_all(receivers).await.unwrap() });

            // Check the data in the output buffers. It must match the input
            // data we fed to the PushBuf.
            for (in_data, slice) in in_data.iter().zip(buffer_slices.iter()) {
                let buf_bytes: &[u8] = &slice.get_mapped_range();
                let buf_u64s: &[u64] = cast_slice(buf_bytes);
                assert_eq!(buf_u64s, in_data);
            }

        }
    }
}
