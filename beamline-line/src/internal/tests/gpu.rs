use core::default::Default;
use futures::executor::block_on;
use std::sync::Arc;
use wgpu::{Device, Instance, Queue};

/// Encapsulates GPU (WGPU) basic classes for testing.
///
/// This is probably not suitable for use in non-test code, because we block
/// while waiting for the GPU resources to be created.
pub struct Gpu {
    pub device: Arc<Device>,
    pub queue: Queue,
}

impl Gpu {
    pub fn new() -> Self {
        block_on(Self::new_async())
    }

    async fn new_async() -> Self {
        let instance = Instance::new(Default::default());
        let adapter = instance
            .request_adapter(&Default::default())
            .await
            .expect("Could not create WGPU Adapter.");
        let (device, queue) = adapter
            .request_device(&Default::default(), None)
            .await
            .expect("Could not create WGPU Device and Queue.");

        Self {
            device: Arc::new(device),
            queue,
        }
    }
}
