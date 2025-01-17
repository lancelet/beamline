use bytemuck::{bytes_of, NoUninit};
use core::{marker::PhantomData, num::NonZero};
use std::sync::Arc;
use wgpu::{
    util::StagingBelt, Buffer, BufferAddress, BufferUsages, BufferViewMut,
    CommandBuffer, CommandEncoder, CommandEncoderDescriptor, Device,
};

/// Efficient buffer to build up an array of `T` values.
///
/// In the line renderer, a common operation involves creating buffers for the
/// GPU by appending values. `PushBuf` does this efficiently.
///
/// # Constant Generic Parameters
///
/// - `T`: The type of value stored in the `PushBuf` array.
/// - `CHUNK_N`: The number of items of type `T` in a staging buffer.
/// - `BUFFER_N`: The total size of the buffer.
///
/// # Lifecycle
///
/// # Internal Operation
///
/// TODO
pub struct PushBuf<T, const CHUNK_N: usize, const BUFFER_N: usize> {
    /// WGPU Device.
    device: Arc<Device>,
    /// Command encoder for a frame. Between frames, this will be `None`.
    encoder: Option<CommandEncoder>,
    /// WGPU Buffer we ultimately copy our values into.
    buffer: Buffer,
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
    /// Size of a chunk buffer from the staging belt.
    chunk_size: usize,
    /// Debugging state.
    #[cfg(debug_assertions)]
    state: State,
    _phantom: PhantomData<T>,
}

impl<T, const CHUNK_N: usize, const BUFFER_N: usize>
    PushBuf<T, CHUNK_N, BUFFER_N>
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
    ///            be included here.
    pub fn new(
        device: Arc<Device>,
        label: Option<&str>,
        usage: BufferUsages,
    ) -> Self {
        debug_assert!(CHUNK_N > 0);
        debug_assert!(BUFFER_N > 0);
        debug_assert!(CHUNK_N < BUFFER_N);

        PushBuf {
            device: device.clone(),
            encoder: None,
            buffer: create_buffer::<BUFFER_N, T>(device.clone(), label, usage),
            buffer_byte_offset: 0,
            view: None,
            view_byte_offset: 0,
            item_count: 0,
            belt: create_staging_belt::<T, CHUNK_N>(),
            chunk_size: CHUNK_N * size_of::<T>(),
            #[cfg(debug_assertions)]
            state: State::Created,
            _phantom: PhantomData,
        }
    }

    /// Returns a reference to the underlying WGPU buffer.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Begins rendering a frame.
    ///
    /// This should be called at the start of rendering a frame. Internally,
    /// it creates a WGPU `CommandEncoder` to manage the buffer operations
    /// for this frame.
    pub fn begin_frame(&mut self) {
        debug_assert!(self.state == State::Created);
        #[cfg(debug_assertions)]
        self.check_state();

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
        debug_assert!(self.state == State::InFrame);
        #[cfg(debug_assertions)]
        self.check_state();

        // Check we haven't exceeded the buffer capacity.
        if self.item_count >= BUFFER_N {
            return Err(Error::CapacityExceeded);
        }

        // If there is no current staging belt buffer view, create one.
        if self.view.is_none() {
            self.create_view();
        }

        // Write the bytes of the value into the staging belt buffer view.
        self.write_view(value);

        // If the staging belt buffer is full, release it back to the GPU.
        if self.view_byte_offset >= self.chunk_size {
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
    /// [`PushBuf::recall`] function should be called.
    pub fn end_frame(&mut self) -> CommandBuffer {
        debug_assert!(self.state == State::InFrame);
        #[cfg(debug_assertions)]
        self.check_state();

        if let Some(_) = self.view.take() {
            self.finish_view();
        }

        self.encoder = None;
        self.buffer_byte_offset = 0;
        self.view = None;
        self.view_byte_offset = 0;
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
    /// enqueue.
    ///
    /// After the `CommandBuffer` returned by [`PushBuf::end_frame`] has been
    /// enqueued, this function should be called. It requests the return of
    /// staging buffers from the GPU so that they can be mapped to host memory
    /// for the next frame.
    ///
    /// This method should be called as soon as possible after the
    /// `CommandBuffer` has been enqueued, and MUST be called before the next
    /// [`PushBuf::begin_frame`] method is called.
    pub fn recall(&mut self) {
        debug_assert!(self.state == State::PostFrame);
        #[cfg(debug_assertions)]
        self.check_state();

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
        debug_assert!(self.state == State::InFrame);
        #[cfg(debug_assertions)]
        self.check_state();
        debug_assert!(self.view.is_none());
        debug_assert!(self.item_count < BUFFER_N);

        // Clamp chunk size at the buffer boundary.
        let remaining_space =
            BUFFER_N * size_of::<T>() - self.buffer_byte_offset;
        let chunk_size =
            NonZero::new(self.chunk_size.min(remaining_space) as BufferAddress)
                .unwrap();

        // Create a view onto the staging buffer chunk.
        let view = self.belt.write_buffer(
            &mut self.encoder.as_mut().unwrap(),
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
        debug_assert!(self.state == State::InFrame);
        #[cfg(debug_assertions)]
        self.check_state();

        self.view = None;
        self.view_byte_offset = 0;
        self.belt.finish();
    }

    /// Writes `value` into the current view at the current offset.
    fn write_view(&mut self, value: T) {
        debug_assert!(self.view.is_some());
        debug_assert!(self.chunk_size % size_of::<T>() == 0);
        debug_assert!(self.view_byte_offset < self.chunk_size);
        debug_assert!(self.buffer_byte_offset < BUFFER_N);
        debug_assert!(self.item_count < BUFFER_N);

        let s = self.view_byte_offset;
        let e = s + self.chunk_size;
        let buf_chunk: &mut [u8] = &mut (self.view.as_mut().unwrap())[s..e];
        debug_assert_eq!(buf_chunk.len(), size_of::<T>());

        buf_chunk.copy_from_slice(bytes_of(&value));

        self.view_byte_offset = e;
        self.item_count += 1;
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
pub enum Error {
    /// The capacity of the buffer would be exceeded.
    CapacityExceeded,
}

/// Creates the main WGPU buffer.
fn create_buffer<const BUFFER_N: usize, T>(
    device: Arc<wgpu::Device>,
    label: Option<&str>,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    let buffer_size_bytes = BUFFER_N * size_of::<T>();
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
fn create_staging_belt<T, const CHUNK_N: usize>() -> StagingBelt {
    let chunk_size_bytes = CHUNK_N * size_of::<T>();
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
