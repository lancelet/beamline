use core::{mem::MaybeUninit, ops::Deref};

/// `Stack` is a fixed-size, stack-like container.
///
/// `Stack` supports:
///
///   - Length queries: [`Stack::len`].
///   - Pushing values onto to the end: [`Stack::push`].
///   - Popping values from the end: [`Stack::pop`].
///   - Clearing all values: [`Stack::clear`].
///   - Viewing as a slice: (`&`).
#[derive(Debug)]
pub struct Stack<T, const N: usize> {
    elem: [MaybeUninit<T>; N],
    size: usize,
}

impl<T, const N: usize> Stack<T, N> {
    /// Creates a new `Stack`.
    pub fn new() -> Self {
        Stack {
            elem: alloc_array(),
            size: 0,
        }
    }

    /// Returns the length of the `Stack`.
    ///
    /// This is the number of items actually stored on the `Stack`, not its
    /// capacity.
    pub fn len(&self) -> usize {
        self.size
    }

    /// Pushes an item onto the end of the stack.
    ///
    /// If the stack has no more space, the `value` is dropped and an error
    /// is returned.
    ///
    /// # Parameters
    ///
    /// - `value`: The value to push onto the stack.
    ///
    /// # Returns
    ///
    /// - `Ok(())`: if there was enough space for the item on the stack.
    /// - `Err(Error::CapacityExceeded)`: if the stack ran out of space.
    pub fn push(&mut self, value: T) -> Result<(), Error> {
        if self.size < N {
            unsafe {
                self.elem[self.size].as_mut_ptr().write(value);
            }
            self.size += 1;
            Ok(())
        } else {
            Err(Error::CapacityExceeded)
        }
    }

    /// Pops an item off the end of the stack.
    ///
    /// # Returns
    ///
    /// - `Some(value)`: if there was an item on the stack.
    /// - `None`: if the stack was empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.size > 0 {
            self.size -= 1;
            let value = unsafe {
                self.elem.as_mut_ptr().add(self.size).read().assume_init()
            };
            Some(value)
        } else {
            None
        }
    }

    /// Clears the stack, removing and dropping all items.
    pub fn clear(&mut self) {
        unsafe {
            let initialized_slice = core::slice::from_raw_parts_mut(
                self.elem.as_mut_ptr() as *mut T,
                self.size,
            );
            core::ptr::drop_in_place(initialized_slice);
        }
        self.size = 0;
    }

    /// Dereference a `Stack` as a a slice.
    fn deref(&self) -> &[T] {
        unsafe {
            core::slice::from_raw_parts(
                self.elem.as_ptr() as *const T,
                self.size,
            )
        }
    }
}

impl<T, const N: usize> Deref for Stack<T, N> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        Stack::deref(self)
    }
}

impl<T, const N: usize> Drop for Stack<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Errors for a [`Stack`].
#[derive(Debug, PartialEq)]
pub enum Error {
    /// Error produced if an attempt is made to store more elements in a stack
    /// than its capacity allows.
    CapacityExceeded,
}

/// Allocate an array of `MaybeUninit` values.
fn alloc_array<T, const N: usize>() -> [MaybeUninit<T>; N] {
    [const { MaybeUninit::uninit() }; N]
}

#[cfg(test)]
mod tests {
    use super::*;
    use prop::{collection, sample::SizeRange};
    use proptest::prelude::*;
    use std::{fmt::Debug, sync::Arc};

    ///---- Sanity Testing ----------------------------------------------------

    // These sanity tests are just examples; not thorough testing. See below
    // for more thorough property tests.

    #[test]
    fn stack_example() {
        let mut stack = Stack::<u32, 3>::new();
        assert_eq!(stack.len(), 0);
        assert_eq!(stack.deref(), &[]);

        let r1 = stack.push(42);
        assert_eq!(r1, Ok(()));
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.deref(), &[42]);

        let r2 = stack.push(10);
        assert_eq!(r2, Ok(()));
        assert_eq!(stack.len(), 2);
        assert_eq!(stack.deref(), &[42, 10]);

        let r3 = stack.push(7);
        assert_eq!(r3, Ok(()));
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.deref(), &[42, 10, 7]);

        let r4 = stack.push(3);
        assert_eq!(r4, Err(Error::CapacityExceeded));
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.deref(), &[42, 10, 7]);

        let p1 = stack.pop();
        assert_eq!(p1, Some(7));
        assert_eq!(stack.len(), 2);
        assert_eq!(stack.deref(), &[42, 10]);

        let p2 = stack.pop();
        assert_eq!(p2, Some(10));
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.deref(), &[42]);

        stack.clear();
        assert_eq!(stack.len(), 0);

        let r5 = stack.push(100);
        assert_eq!(r5, Ok(()));
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.deref(), &[100]);
    }

    ///---- Property Testing Stack --------------------------------------------

    // Here, we compare `Stack` against a (more) trivial implementation of the
    // same API, written using `Vec`, called `VectorStack`.
    //
    // A vector of operations, called `StackOp`s, is generated. These operatios
    // are run against both stacks in various ways. The results are compared to
    // ensure that the stacks perform identically.

    /// Mutation operations that can run on a stack.
    #[derive(Debug, Clone)]
    enum StackOp<T> {
        /// Push a single value to the stack.
        Push(T),
        /// Push a value to the stack a given number of times. This is
        /// used so that the we can test the stack's ability to drop shared
        /// `Arc`s.
        PushN(u8, T),
        /// Pop a value off the stack.
        Pop,
        /// Clear the stack.
        Clear,
    }
    impl<T> StackOp<T> {
        fn map<F, Q>(self, f: F) -> StackOp<Q>
        where
            F: FnOnce(T) -> Q,
        {
            match self {
                StackOp::Push(x) => StackOp::Push(f(x)),
                StackOp::PushN(n, x) => StackOp::PushN(n, f(x)),
                StackOp::Pop => StackOp::Pop,
                StackOp::Clear => StackOp::Clear,
            }
        }
    }

    /// Generates a `StackOp<T>` given a generator for `T` values.
    fn stack_op_gen<T>(
        t_gen: impl Strategy<Value = T> + Clone + 'static,
    ) -> impl Strategy<Value = StackOp<T>>
    where
        T: Debug + Clone + 'static,
    {
        prop_oneof![
            8  => t_gen.clone().prop_map(StackOp::Push),
            3  => (1u8..5, t_gen.clone())
                    .prop_map(|(n, t)| StackOp::PushN(n, t)),
            10 => Just(StackOp::Pop),
            1  => Just(StackOp::Clear)
        ]
        .boxed()
    }

    /// Generates a `Vec<StackOp<T>>` given a generator for `T` values.
    fn stack_op_vec<T>(
        t_gen: impl Strategy<Value = T> + Clone + 'static,
        size: impl Into<SizeRange>,
    ) -> impl Strategy<Value = Vec<StackOp<T>>>
    where
        T: Debug + Clone + 'static,
    {
        collection::vec(stack_op_gen(t_gen), size)
    }

    /// Test implementation of the stack containing a `Vec`.
    struct VectorStack<T> {
        data: Vec<T>,
    }
    impl<T> VectorStack<T> {
        fn new(capacity: usize) -> Self {
            VectorStack {
                data: Vec::with_capacity(capacity),
            }
        }
        fn len(&self) -> usize {
            self.data.len()
        }
        fn push(&mut self, value: T) -> Result<(), Error> {
            if self.data.len() >= self.data.capacity() {
                Err(Error::CapacityExceeded)
            } else {
                self.data.push(value);
                Ok(())
            }
        }
        fn pop(&mut self) -> Option<T> {
            self.data.pop()
        }
        fn clear(&mut self) {
            self.data.clear();
        }
    }

    /// Run a set of stack operations synchronously on both a `VectorStack`
    /// and a `Stack`, checking carefully that they produce the same results.
    fn run_on_both_stacks_sync<T, const N: usize>(ops: &Vec<StackOp<T>>)
    where
        T: Clone + PartialEq + Debug,
    {
        let mut vstack: VectorStack<T> = VectorStack::new(N);
        let mut astack: Stack<T, N> = Stack::new();

        for op in ops {
            match op {
                StackOp::Push(value) => {
                    let rv = vstack.push(value.clone());
                    let ra = astack.push(value.clone());
                    assert_eq!(rv, ra);
                    compare_stacks(&vstack, &astack);
                }
                StackOp::PushN(n, value) => {
                    for i in 0..*n {
                        let rv = vstack.push(value.clone());
                        let ra = astack.push(value.clone());
                        assert_eq!(rv, ra);
                        compare_stacks(&vstack, &astack);
                    }
                }
                StackOp::Pop => {
                    let ov = vstack.pop();
                    let oa = astack.pop();
                    assert_eq!(ov, oa);
                    compare_stacks(&vstack, &astack);
                }
                StackOp::Clear => {
                    vstack.clear();
                    astack.clear();
                    compare_stacks(&vstack, &astack);
                }
            }
        }
    }

    /// Run all operations on a `VectorStack`, and return the resulting
    /// `VectorStack`.
    fn run_all_on_vectorstack<T, const N: usize>(
        ops: &Vec<StackOp<T>>,
    ) -> VectorStack<T>
    where
        T: Clone,
    {
        let mut stack: VectorStack<T> = VectorStack::new(N);
        for op in ops {
            match op {
                StackOp::Push(value) => {
                    _ = stack.push(value.clone());
                }
                StackOp::PushN(n, value) => {
                    for i in 0..*n {
                        _ = stack.push(value.clone())
                    }
                }
                StackOp::Pop => {
                    _ = stack.pop();
                }
                StackOp::Clear => {
                    stack.clear();
                }
            }
        }
        stack
    }

    /// Run all operations on a new `Stack`, and return the resulting `Stack`.
    fn run_all_on_stack<T, const N: usize>(ops: &Vec<StackOp<T>>) -> Stack<T, N>
    where
        T: Clone,
    {
        let mut stack: Stack<T, N> = Stack::new();
        for op in ops {
            match op {
                StackOp::Push(value) => {
                    _ = stack.push(value.clone());
                }
                StackOp::PushN(n, value) => {
                    for i in 0..*n {
                        _ = stack.push(value.clone())
                    }
                }
                StackOp::Pop => {
                    _ = stack.pop();
                }
                StackOp::Clear => {
                    stack.clear();
                }
            }
        }
        stack
    }

    /// Run all stack operations on both kinds of stack, and compare the
    /// number of counts of owners.
    fn run_on_stacks_test_ownership_counts<T, const N: usize>(
        ops: &Vec<StackOp<T>>,
    ) where
        T: Clone,
    {
        // Create two separate vectors of ops.
        let ops_v: Vec<StackOp<Arc<T>>> = ops
            .iter()
            .map(|x: &StackOp<T>| x.clone().map(Arc::new))
            .collect();
        let ops_s: Vec<StackOp<Arc<T>>> = ops
            .iter()
            .map(|x: &StackOp<T>| x.clone().map(Arc::new))
            .collect();

        // Run the two implementations independently.
        let stack_v = run_all_on_vectorstack::<Arc<T>, N>(&ops_v);
        let stack_a = run_all_on_stack::<Arc<T>, N>(&ops_s);

        // Zip the results together and compare their usage counts.
        let vslice: &[Arc<T>] = &stack_v.data;
        let aslice: &[Arc<T>] = &stack_a;
        for (va, aa) in vslice.iter().zip(aslice.iter()) {
            let cv = Arc::strong_count(va);
            let ca = Arc::strong_count(aa);
            assert_eq!(cv, ca);
        }

        assert_eq!(stack_v.len(), stack_a.len());
    }

    /// Compare a `VectorStack` and a `Stack`.
    fn compare_stacks<T, const N: usize>(
        vstack: &VectorStack<T>,
        astack: &Stack<T, N>,
    ) where
        T: Clone + PartialEq + Debug,
    {
        assert_eq!(vstack.len(), astack.len());

        let vslice: &[T] = &vstack.data;
        let aslice: &[T] = &astack;
        assert_eq!(vslice, aslice);
    }

    proptest! {
        /// Test generated stack operations on various size stacks.
        #[test]
        fn test_generated_stack_operations(
            stack_ops in stack_op_vec(any::<u32>(), 0..200)
        ) {
            run_on_both_stacks_sync::<u32, 1>(&stack_ops);
            run_on_both_stacks_sync::<u32, 5>(&stack_ops);
            run_on_both_stacks_sync::<u32, 20>(&stack_ops);
            run_on_both_stacks_sync::<u32, 200>(&stack_ops);
        }

        /// Test that the stack is properly dropping elements.
        #[test]
        fn test_generated_stack_operations_ownership(
            stack_ops in stack_op_vec(any::<u32>(), 0..200)
        ) {
            run_on_stacks_test_ownership_counts::<u32, 1>(&stack_ops);
            run_on_stacks_test_ownership_counts::<u32, 5>(&stack_ops);
            run_on_stacks_test_ownership_counts::<u32, 20>(&stack_ops);
            run_on_stacks_test_ownership_counts::<u32, 200>(&stack_ops);
        }
    }
}
