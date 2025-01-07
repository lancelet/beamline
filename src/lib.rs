#[allow(unused)] // TODO: For development.
mod wgpu_context;

use cfg_if::cfg_if;
use log::LevelFilter;
use std::sync::Arc;
use wgpu_context::{FutureWgpuContext, WgpuContext};
use winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn run() {
    run_app().unwrap(); // Panic on error (intentional).
}

fn run_app() -> Result<(), EventLoopError> {
    let event_loop = EventLoop::builder().build()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app)
}

#[derive(Default)]
pub struct App {
    window: Option<Arc<Window>>,
    future_wgpu_context: Option<FutureWgpuContext>,
}
impl App {
    const LOG_LEVEL_FILTER: LevelFilter = LevelFilter::Trace;
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            const CANVAS_ID: &str = "linerender-host-canvas";
            const BACKENDS: wgpu::Backends = wgpu::Backends::BROWSER_WEBGPU;
        } else {
            const BACKENDS: wgpu::Backends = wgpu::Backends::PRIMARY;
        }
    }

    /// Fetches the WGPU context if it is available yet.
    ///
    /// # Panics
    ///
    /// - If the WGPU context is requested before the App has been initialized
    ///   (in the [`ApplicationHandler::resumed`]) method).
    /// - If the processing to create the `WgpuContext` was canceled.
    ///
    /// # Returns
    ///
    /// - `Some(wgpu_context)`: if the WgpuContext has been created.
    /// - `None`: if the WgpuContext creation is still pending.
    #[allow(unused)] // TODO: Development
    fn wgpu_context(&mut self) -> Option<&WgpuContext> {
        self.future_wgpu_context
            .as_ref()
            .map(FutureWgpuContext::retrieve_option)
            .expect(
                "Attempted to fetch the WGPU context before it was \
                 initialized in ApplicationHandler::resumed.",
            )
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Configure the logger.
        init_logger(Self::LOG_LEVEL_FILTER);

        // Set up window attributes.
        let mut attributes = Window::default_attributes();
        #[cfg(not(target_arch = "wasm32"))]
        {
            attributes = attributes.with_title("Line Render Prototype");
        }
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            attributes = attributes.with_canvas(get_canvas(App::CANVAS_ID));
        }

        // Create the window.
        match event_loop.create_window(attributes) {
            Err(os_error) => {
                panic!("Could not creating window: {:?}", os_error)
            }
            Ok(window) => {
                let window = Arc::new(window);
                self.window = Some(window.clone());

                // Set up requirements for WGPU context.
                let instance_descriptor = wgpu::InstanceDescriptor {
                    backends: App::BACKENDS,
                    ..Default::default()
                };
                let request_adapter_options = wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: None, // filled in by `FutureWgpuContext`
                    force_fallback_adapter: false,
                };

                // Launch WGPU context setup.
                //
                // Here, we are handing off further configuration of WGPU to
                // the window event handler, [`window_event`].
                self.future_wgpu_context = Some(FutureWgpuContext::new(
                    window.clone(),
                    instance_descriptor,
                    request_adapter_options,
                ));
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Fetch WGPU context.
        /*
        if let Some(wgpu_context) = self.wgpu_context() {
            info!("WGPU context was created");
        } else {
            info!("Waiting for WGPU context to be created");
        }
        */
        use WindowEvent::CloseRequested;
        match event {
            CloseRequested => event_loop.exit(),
            _ => (),
        }
    }
}

/// Get the HTML canvas element named `canvas_id` on the **WASM32** platform.
///
/// On the **WASM32** platform, rendering will happen in a canvas. This
/// function fetches the canvas element using its string ID.
///
/// # Parameters
///
/// - `canvas_id`: id of the canvas to return
///
/// # Returns
///
/// Canvas as an `Option<HtmlCanvasElement>`. `None` indicates that the canvas
/// could not be found.
#[cfg(target_arch = "wasm32")]
fn get_canvas(canvas_id: &str) -> Option<wgpu::web_sys::HtmlCanvasElement> {
    use wgpu::web_sys;
    let window: web_sys::Window = web_sys::window()?;
    let document: web_sys::Document = window.document()?;
    let element: web_sys::Element = document.get_element_by_id(canvas_id)?;
    let canvas: web_sys::HtmlCanvasElement =
        element.dyn_into::<web_sys::HtmlCanvasElement>().ok()?;
    Some(canvas)
}

/// Initializes the logger in a platform-dependent way.
///
/// This function sets up a logger suitable for the current platform.
///
/// - **WASM32 (WebAssembly:** Uses `console_log`.
/// - **Native Platforms:** Uses `env_logger`.
///
/// # Parameters
///
/// - `level_filter`: The logging level to be applied globally.
///
/// # Panics
///
/// - On **WASM32**, the function will panic if the `console_log` fails to
///   initialize.
fn init_logger(level_filter: LevelFilter) {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let log_level = level_filter.to_level().unwrap_or(log::Level::Warn);
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log_level).expect("Could not initialize WASM32 logger");
        } else {
            env_logger::Builder::from_default_env()
                .filter_level(level_filter)
                .init();
        }
    }
}
