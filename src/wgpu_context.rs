//! Creates WGPU context that must run async.
//!
//! Three important parts of the WGPU API are async functions:
//!   - Creating a new WGPU Instance.
//!   - Requesting a new WGPU Adapter.
//!   - Creating a WGPU Device and Queue.
//!   
//! In non-web applications, we would like to block on these functions. However,
//! for WASM (web) usage, we cannot block.
//!
//! The solution here is something called [`FutureWgpuContext`]. The idea is
//! that you pass the structures necessary to construct the WGPU components that
//! are async. Then [`FutureWgpuContext`] can be queried using
//! [`FutureWgpuContext::retrieve`], until it returns a completed value. This
//! querying should be done in the application's event loop, to avoid blocking
//! anything else.

use futures::channel::oneshot::{Canceled, Receiver, Sender};
use pollster::block_on;
use std::{
    cell::{OnceCell, RefCell},
    fmt::Debug,
    future::Future,
};

/// Encapsulates parts of WGPU that need async construction.
///
/// To create a [`WgpuContext`] in a way that handles the async problems for
/// both WASM and other platforms, see [`FutureWgpuContext::new`].
#[derive(Debug)]
pub struct WgpuContext {
    /// WGPU Surface.
    pub surface: wgpu::Surface<'static>,
    /// Selected WGPU Adapter.
    pub adapter: wgpu::Adapter,
    /// Selected WGPU Device.
    pub device: wgpu::Device,
    /// Selected WGPU Queue.
    pub queue: wgpu::Queue,
}

impl WgpuContext {
    /// Create a new `WgpuContext` in an async function.
    ///
    /// When the WGPU Surface is created, `request_adapter_options` will be
    /// modified so that the `compatible_surface` contains a pointer to the
    /// created surface.
    ///
    /// You may want to use [`FutureWgpuContext::new`] instead, for an approach
    /// that allows you to poll for completion instead of using async.
    async fn new_async(
        window: impl Into<wgpu::SurfaceTarget<'static>> + 'static,
        instance_descriptor: wgpu::InstanceDescriptor,
        request_adapter_options: wgpu::RequestAdapterOptions<'static, 'static>,
        device_descriptor: wgpu::DeviceDescriptor<'static>,
    ) -> Self {
        let instance = wgpu::Instance::new(instance_descriptor);
        let surface = instance
            .create_surface(window)
            .expect("Could not create WGPU Surface.");

        let adapter_options_for_surface = wgpu::RequestAdapterOptions {
            power_preference: request_adapter_options.power_preference,
            force_fallback_adapter: request_adapter_options.force_fallback_adapter,
            compatible_surface: Some(&surface),
        };

        let adapter = instance
            .request_adapter(&adapter_options_for_surface)
            .await
            .expect("Could not create WGPU Adapter.");

        let (device, queue) = adapter
            .request_device(&device_descriptor, None)
            .await
            .expect("Could not create WGPU Device and Queue.");

        WgpuContext {
            surface,
            adapter,
            device,
            queue,
        }
    }

    /// Return a reference to the WGPU Surface.
    pub fn surface(&self) -> &wgpu::Surface {
        &self.surface
    }

    /// Return a reference to the WGPU Adapter.
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    /// Return a reference to the WGPU Device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }
}

/// Result of an async computation to create a [`WgpuContext`].
#[derive(Debug)]
pub enum AsyncWgpuContextResult {
    /// The [`WgpuContext`] has been created.
    Done(WgpuContext),
    /// The async computation has not yet been completed.
    NotReady,
    /// The async computation was canceled.
    Canceled,
}
impl AsyncWgpuContextResult {
    /// Convert an `AsyncWgpuContextResult` to an option.
    ///
    /// # Panics
    ///
    /// - If the `AsyncWgpuContextResult` was `Canceled`.
    pub fn to_option(&self) -> Option<&WgpuContext> {
        match self {
            Self::Done(wgpu_context) => Some(wgpu_context),
            Self::NotReady => None,
            Self::Canceled => {
                panic!("FutureWgpuContext creation was canceled!");
            }
        }
    }
}

/// A possibly-ongoing async computation to create a [`WgpuContext`].
#[derive(Debug)]
pub struct FutureWgpuContext {
    value_cell: OnceCell<AsyncWgpuContextResult>,
    receiver: RefCell<Receiver<WgpuContext>>,
}

impl FutureWgpuContext {
    /// Create a new `FutureWgpuContext`, which will perform async
    /// construction of a `WgpuContext`.
    ///
    /// After creating a `FutureWgpuContext`, in the application event loop,
    /// use the [`FutureWgpuContext::retrieve`] method to query the result.
    ///
    /// When the WGPU Surface is created, `request_adapter_options` will be
    /// modified so that the `compatible_surface` contains a pointer to the
    /// created surface.
    pub fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>> + 'static,
        instance_descriptor: wgpu::InstanceDescriptor,
        request_adapter_options: wgpu::RequestAdapterOptions<'static, 'static>,
        device_descriptor: wgpu::DeviceDescriptor<'static>,
    ) -> Self {
        FutureWgpuContext {
            value_cell: OnceCell::new(),
            receiver: RefCell::new(FutureWgpuContext::spawn_receiver(|| {
                WgpuContext::new_async(
                    window,
                    instance_descriptor,
                    request_adapter_options,
                    device_descriptor,
                )
            })),
        }
    }

    /// Retrieve an optional [`WgpuContext`].
    ///
    /// # Panics
    ///
    /// - If the `AsyncWgpuContextResult` was `Canceled`.
    pub fn retrieve_option(&self) -> Option<&WgpuContext> {
        self.retrieve().to_option()
    }

    /// Retrieve an [`AsyncWgpuContextValue`].
    pub fn retrieve(&self) -> &AsyncWgpuContextResult {
        match self.value_cell.get() {
            Some(value) => value,
            None => {
                let mut receiver = self.receiver.borrow_mut();
                match receiver.try_recv() {
                    Ok(Some(value)) => {
                        self.value_cell
                            .set(AsyncWgpuContextResult::Done(value))
                            .unwrap();
                        receiver.close();
                        self.retrieve()
                    }
                    Ok(None) => &AsyncWgpuContextResult::NotReady,
                    Err(Canceled) => {
                        self.value_cell
                            .set(AsyncWgpuContextResult::Canceled)
                            .unwrap();
                        receiver.close();
                        self.retrieve()
                    }
                }
            }
        }
    }

    /// Run async function `f`, possibly blocking on it, and return a
    /// `Receiver` for its returned value.
    ///
    /// The purpose of `spawn_receiver` is to abstract over async handling for
    /// WASM and other platforms. WASM cannot block, so a channel arrangement
    /// is used. The `Receiver` will receive the result of the async function
    /// once it has completed.
    fn spawn_receiver<Fn, Fut, T>(f: Fn) -> Receiver<T>
    where
        T: Debug + 'static,
        Fn: FnOnce() -> Fut + 'static,
        Fut: Future<Output = T> + 'static,
    {
        let (sender, receiver) = futures::channel::oneshot::channel::<T>();
        FutureWgpuContext::spawn(sender, f);
        receiver
    }

    /// Run async function `f`, possibly blocking on it, and send the resulting
    /// value to `sender`.
    ///
    /// The purpose of `spawn` is to abstract over async handling for WASM and
    /// other platforms. WASM cannot block, so a channel arrangement is used.
    fn spawn<Fn, Fut, T>(sender: Sender<T>, f: Fn)
    where
        T: Debug + 'static,
        Fn: FnOnce() -> Fut + 'static,
        Fut: Future<Output = T> + 'static,
    {
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = f().await;
                sender.send(result).unwrap();
            })
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let result = block_on(f());
            sender.send(result).unwrap();
        }
    }
}
