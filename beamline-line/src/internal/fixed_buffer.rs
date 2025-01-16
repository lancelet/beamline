use bytemuck::{cast_slice, NoUninit};
use core::{mem::MaybeUninit, ops::Index};

/// Buffer with a fixed number of elements.
pub struct FixedBuffer<T, const N: usize> {
    elements: [MaybeUninit<T>; N],
    size: usize,
}

impl<T, const N: usize> FixedBuffer<T, N> {
    /// Creates a new `FixedBuffer`.
    ///
    /// The size of the `FixedBuffer` is determined by the constant generic
    /// parameter `N`.
    pub fn new() -> Self {
        FixedBuffer {
            elements: [const { MaybeUninit::uninit() }; N],
            size: 0,
        }
    }

    /// Returns the number of elements stored in the `FixedBuffer`.
    pub fn length(&self) -> usize {
        self.size
    }

    /// Appends a value to the `FixedBuffer`.
    ///
    /// This function can fail if the buffer is full. If the buffer is full,
    /// the value will be dropped.
    ///
    /// # Parameters
    ///
    /// - `value`: Value to append to the buffer.
    ///
    /// # Returns
    ///
    /// - [`Result::Ok`]: if the value was successfully appended to the buffer.
    /// - [`Result::Err`]: if the buffer was full.
    pub fn append(&mut self, value: T) -> Result<(), Error> {
        if self.size < N {
            self.elements[self.size] = MaybeUninit::new(value);
            self.size += 1;
            Ok(())
        } else {
            Err(Error::BufferFull)
        }
    }

    pub fn clear(&mut self) {
        // Drop all the active elements.
        unsafe {
            for i in 0..self.size {
                let elem_ptr = self.elements.as_mut_ptr().add(i);
                let value = elem_ptr.read().assume_init();
                drop(value);
            }
        }

        // Reset the size.
        self.size = 0;
    }

    /// Converts this `FixedBuffer` to a byte array.
    pub fn bytes(&self) -> &[u8]
    where
        T: NoUninit,
    {
        let slice = unsafe {
            let ptr: *const T = self.elements.as_ptr() as *const T;
            core::slice::from_raw_parts(&*ptr, self.size)
        };
        cast_slice::<T, u8>(slice)
    }
}

impl<T, const N: usize> Index<usize> for FixedBuffer<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.size);
        unsafe { &*self.elements[index].as_ptr() }
    }
}

/// Errors that can occur when using the `FixedBuffer`.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Error {
    /// The buffer is full.
    BufferFull,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_new() {
        let buffer = FixedBuffer::<u32, 10>::new();
        assert_eq!(buffer.length(), 0);
    }

    #[test]
    fn test_append() {
        let mut buffer = FixedBuffer::<u32, 3>::new();

        let r1 = buffer.append(10);
        assert_eq!(r1, Ok(()));
        assert_eq!(buffer.length(), 1);

        let r2 = buffer.append(66);
        assert_eq!(r2, Ok(()));
        assert_eq!(buffer.length(), 2);

        let r3 = buffer.append(42);
        assert_eq!(r3, Ok(()));
        assert_eq!(buffer.length(), 3);

        let r4 = buffer.append(100);
        assert_eq!(r4, Err(Error::BufferFull));
        assert_eq!(buffer.length(), 3);

        assert_eq!(10, buffer[0]);
        assert_eq!(66, buffer[1]);
        assert_eq!(42, buffer[2]);
    }

    #[test]
    fn test_clear() {
        let mut buffer = FixedBuffer::<u32, 3>::new();
        assert_eq!(0, buffer.length());
        buffer.clear();
        assert_eq!(0, buffer.length());

        _ = buffer.append(10);
        _ = buffer.append(2);
        assert_eq!(2, buffer.length());
        assert_eq!(10, buffer[0]);
        assert_eq!(2, buffer[1]);

        buffer.clear();
        assert_eq!(0, buffer.length());

        _ = buffer.append(42);
        _ = buffer.append(54);
        assert_eq!(2, buffer.length());
        assert_eq!(42, buffer[0]);
        assert_eq!(54, buffer[1]);
    }

    #[test]
    fn test_clear_drops_properly() {
        let mut buffer = FixedBuffer::<Arc<u32>, 3>::new();
        assert_eq!(0, buffer.length());

        let value: Arc<u32> = Arc::new(42);
        assert_eq!(1, Arc::strong_count(&value));

        _ = buffer.append(value.clone());
        assert_eq!(2, Arc::strong_count(&value));
        assert_eq!(1, buffer.length());

        buffer.clear();
        assert_eq!(0, buffer.length());
        assert_eq!(1, Arc::strong_count(&value));
    }

    #[test]
    fn test_bytes() {
        let mut buffer = FixedBuffer::<u32, 3>::new();
        _ = buffer.append(10);
        _ = buffer.append(42);

        let expected: [u8; 8] = [10, 0, 0, 0, 42, 0, 0, 0];
        let bytes = buffer.bytes();
        assert_eq!(bytes, expected);
    }
}
