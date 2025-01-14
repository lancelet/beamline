#[allow(unused)] // TODO: For development.
mod bucketer;
mod frame_timer;
#[allow(unused)] // TODO: For development.
mod wgpu_context;

use beamline::{Line, Renderer, P2};
use cfg_if::cfg_if;
use frame_timer::FrameTimer;
use log::{trace, warn, LevelFilter};
use std::{cell::RefCell, sync::Arc};
use wgpu::SurfaceConfiguration;
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

/// Run the application.
///
/// This is effectively the main entry point for the whole app, one level
/// above winit. It is also the **WASM32** entry point. It tries to run the
/// app, and panics if unable to do so.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn run() {
    run_app().unwrap(); // Panic on error (intentional).
}

/// Run the application.
///
/// This is effectively the main entry point for winit. It sets up the winit
/// event loop and runs the `App` with it.
fn run_app() -> Result<(), EventLoopError> {
    let event_loop = EventLoop::builder().build()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    // event_loop.set_control_flow(ControlFlow::Wait);

    // The application is launched two different ways for WASM32 and native.
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            // For WASM32, we "spawn" the app and return immediately. This
            // avoids a log message from winit saying that it's using
            // exceptions for control flow (!).
            use winit::platform::web::EventLoopExtWebSys;
            let app = App::default();
            event_loop.spawn_app(app);
            Ok(())
        } else {
            // For native platforms, we call `run_app` instead.
            let mut app = App::default();
            event_loop.run_app(&mut app)
        }
    }
}

#[derive(Debug, Default)]
pub struct App {
    /// Frame timer.
    frame_timer: Option<FrameTimer>,
    /// The Application's winit window.
    window: Option<Arc<Window>>,
    /// WGPU context - has async setup.
    wgpu_context: Option<FutureWgpuContext>,
    /// Flag to indicate whether all WGPU setup has finished.
    extra_wgpu_setup_completed: bool,
    /// Surface configuration for WGPU.
    surface_configuration: Option<wgpu::SurfaceConfiguration>,
    /// Beamline renderer.
    beamline_renderer: Option<RefCell<Renderer>>,
}
impl App {
    /// Override the application logging level.
    ///
    /// Set this to override the logging level for both **WASM32** and
    /// **Native** applications.
    const LOG_LEVEL_FILTER: Option<LevelFilter> = None; // Some(LevelFilter::Debug);

    /// Background color.
    const BACKGROUND_COLOR: wgpu::Color = wgpu::Color {
        r: 0.05,
        g: 0.07,
        b: 0.09,
        a: 1.0,
    };

    /// Size of a rendering bucket.
    const TILE_SIZE: u32 = 32;

    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            const CANVAS_ID: &str = "linerender-host-canvas";
            const BACKENDS: wgpu::Backends = wgpu::Backends::BROWSER_WEBGPU;
        } else {
            const BACKENDS: wgpu::Backends = wgpu::Backends::PRIMARY;
        }
    }

    /// Return a mutable reference to the frame timer.
    fn frame_timer(&mut self) -> &mut FrameTimer {
        self.frame_timer.as_mut().unwrap()
    }

    /// Return a reference to the application window, incrementing its
    /// reference count.
    fn window(&self) -> Arc<Window> {
        self.window.clone().unwrap()
    }

    /// Fetches the WGPU context if it is available.
    ///
    /// The setup of the WGPU context is started when the application launches,
    /// in the [`App::resumed`] method. The initialization then runs async and
    /// posts when it is finished. If the WGPU context is available,
    ///
    /// # Panics
    ///
    /// - If this method is called before `App::resumed`.
    /// - If creating the `WgpuContext` was canceled.
    ///
    /// # Returns
    ///
    /// - `Some(wgpu_context)`: if the `WgpuContext` was created.
    /// - `None`: if the `WgpuContext` is still pending.
    fn optional_wgpu_context(&self) -> Option<&WgpuContext> {
        self.wgpu_context
            .as_ref()
            .map(|ctx| ctx.retrieve_option())
            .expect(
                "App::resumed must have been called before you call this \
                 method.",
            )
    }

    /// Fetches the WGPU context, making the assumption it is available.
    ///
    /// The setup of the WGPU context is started when the application launches,
    /// in the [`App::resumed`] method. The initialization then runs async and
    /// posts when it is finished. If the WGPU context is available,
    ///
    ///
    /// # Panics
    ///
    /// - If this method is called before `App::resumed`.
    /// - If creating the `WgpuContext` was canceled.
    /// - If the `WgpuContext` is not available yet.
    ///
    /// # Returns
    ///
    /// A reference to the `WgpuContext`.
    fn wgpu_context(&self) -> &WgpuContext {
        self.optional_wgpu_context()
            .expect("WgpuContext was not (yet) available.")
    }

    /// Choose the surface configuration for rendering.
    ///
    /// We want an SRGB surface. This must be called after the WGPU context is
    /// available.
    fn choose_surface_configuration(&mut self) {
        let ctx = self.wgpu_context();
        let surface_caps = ctx.surface().get_capabilities(ctx.adapter());

        // find an sRGB surface
        /*
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or({
                warn!(
                    "Could not select sRGB surface format. Falling back to \
                     first format available."
                );
                surface_caps.formats[0]
            });
            */
        let surface_format = wgpu::TextureFormat::Bgra8Unorm;
        trace!("Surface format: {:?}", surface_format);

        let size = self.window().inner_size();
        let surface_configuration = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        self.surface_configuration = Some(surface_configuration);

        trace!("Chose surface configuration.");
    }

    /// Return a reference to the WGPU SurfaceConfiguration.
    fn surface_configuration(&self) -> &SurfaceConfiguration {
        self.surface_configuration.as_ref().unwrap()
    }

    /// Return a mutable reference to the WGPU SurfaceConfiguration.
    fn surface_configuration_mut(&mut self) -> &mut SurfaceConfiguration {
        self.surface_configuration.as_mut().unwrap()
    }

    /// Create the beamline line renderer.
    fn create_beamline_renderer(&mut self) {
        let device = self.wgpu_context().device();
        let size = self.window().inner_size();
        let texture_format = self.surface_configuration().format;
        let renderer = Renderer::new(
            device,
            texture_format,
            size.width,
            size.height,
            App::TILE_SIZE,
            App::TILE_SIZE,
        );
        self.beamline_renderer = Some(RefCell::new(renderer));
    }

    /// Return a reference to the beamline renderer.
    fn beamline_renderer(&self) -> &RefCell<Renderer> {
        self.beamline_renderer.as_ref().unwrap()
    }

    /// Configure the surface post-resize. This sets the size of the surface.
    fn resize(&mut self) {
        // The window might be resized before WGPU setup has finished. If so,
        // just bail.
        if !self.extra_wgpu_setup_completed {
            return;
        }
        let size = self.window().inner_size();
        if size.width > 0 && size.height > 0 {
            // Resize the surface
            {
                let cfg = self.surface_configuration_mut();
                cfg.width = size.width;
                cfg.height = size.height;
            }
            let ctx = self.wgpu_context();
            ctx.surface()
                .configure(ctx.device(), self.surface_configuration());

            // Resize the beamline renderer
            self.beamline_renderer()
                .borrow_mut()
                .resize(size.width, size.height);
        }
        trace!("Configured surface size: {:?}", size);
    }

    /// Finish the WGPU static setup.
    ///
    /// This should be called from the event loop once the `WgpuContext` has
    /// finished its async setup and is available.
    fn finish_wgpu_static_setup(&mut self) {
        if !self.extra_wgpu_setup_completed {
            if self.optional_wgpu_context().is_some() {
                // Perform extra WGPU setup.
                self.choose_surface_configuration();
                self.frame_timer = Some(FrameTimer::new());
                self.create_beamline_renderer();
                self.extra_wgpu_setup_completed = true;
                self.resize();
            } else {
                // If we reach here, the extra WGPU setup wasn't yet completed,
                // and we're waiting for the WgpuContext to finish its async
                // creation. So, request a redraw so that we can try again on
                // the next frame.
                self.window().request_redraw();
            }
        }
    }

    /// Render a single frame.
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Bail if setup has not completed.
        if !self.extra_wgpu_setup_completed {
            return Ok(());
        }

        // Frame timer
        let tsec = self.frame_timer().total_time_secs_f64();
        let millis = self.frame_timer().tick_millis();
        println!("Frame time: {} ms", millis);

        let ctx = self.wgpu_context();
        let device = ctx.device();
        let queue = ctx.queue();
        let surface = ctx.surface();

        // get_current_texture will block when in FIFO present mode.
        let output_texture = surface.get_current_texture()?;
        let view = output_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Command Encoder"),
        });

        // TODO: Remove.
        let width = 100.0 * (0.95 * (tsec * 1.5).sin() as f32 + 1.0);
        let alpha = 1.0; // 0.5 * (0.3 * (tsec * 7.0).sin() as f32 + 1.0);
                         // Add example lines.
        self.beamline_renderer().borrow_mut().line(
            Line::new(P2::new(100.0, 100.0), P2::new(800.0, 100.0)),
            &beamline::LineStyle {
                width,
                cap: beamline::LineCap::Round,
                color: beamline::Color::new(0.9, 0.4, 0.4, alpha),
            },
        );
        self.beamline_renderer().borrow_mut().line(
            Line::new(P2::new(100.0, 160.0), P2::new(800.0, 160.0)),
            &beamline::LineStyle {
                width,
                cap: beamline::LineCap::Square,
                color: beamline::Color::new(0.4, 0.9, 0.4, alpha),
            },
        );
        self.beamline_renderer().borrow_mut().line(
            Line::new(P2::new(100.0, 220.0), P2::new(800.0, 220.0)),
            &beamline::LineStyle {
                width,
                cap: beamline::LineCap::Butt,
                color: beamline::Color::new(0.4, 0.4, 0.9, alpha),
            },
        );

        // Render to the surface from the beamline renderer.
        self.beamline_renderer()
            .borrow_mut()
            .render(device, &mut encoder, queue, &view);

        queue.submit(std::iter::once(encoder.finish()));
        output_texture.present();

        Ok(())
    }

    /// Redraw the window: render a frame and handle any errors.
    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        // Bail if setup has not completed.
        if !self.extra_wgpu_setup_completed {
            return;
        }

        // Handle any errors from the render call.
        use wgpu::SurfaceError::{Lost, OutOfMemory, Outdated, Timeout};
        match self.render() {
            Ok(()) => {}
            Err(Lost) | Err(Outdated) => self.resize(),
            Err(Timeout) => warn!("Surface timeout"),
            Err(OutOfMemory) => {
                log::error!("OutOfMemory");
                event_loop.exit();
            }
        }

        // Request a new redraw after this one.
        self.window().request_redraw();
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

        // Create the window, and launch async WGPU setup.
        match event_loop.create_window(attributes) {
            Err(os_error) => {
                panic!("Could not creating window: {:?}", os_error)
            }
            Ok(window) => {
                let window = Arc::new(window);
                self.window = Some(window.clone());
                self.wgpu_context = Some(create_wgpu_context(window));
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Finish any WGPU static setup. This just bails immediately if the
        // setup has been completed.
        if !self.extra_wgpu_setup_completed {
            self.finish_wgpu_static_setup();
        }

        use WindowEvent::{CloseRequested, RedrawRequested, Resized};
        match event {
            CloseRequested => event_loop.exit(),
            Resized(_) => self.resize(),
            RedrawRequested => self.redraw(event_loop),
            _ => (),
        }
    }
}

/// Create the WGPU context.
///
/// This launches the creation of the async parts of the WGPU context. The
/// rest of the application continues while we're waiting for them. The
/// [`FutureWgpuContext`] can be queried from the event loop of the application
/// to see when WGPU is ready.
fn create_wgpu_context(window: Arc<Window>) -> FutureWgpuContext {
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
    let device_descriptor = wgpu::DeviceDescriptor {
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        label: Some("Device Descriptor"),
        memory_hints: Default::default(),
    };

    // Launch WGPU context setup.
    //
    // Here, we are handing off further configuration of WGPU to
    // the window event handler, [`window_event`].
    FutureWgpuContext::new(
        window.clone(),
        instance_descriptor,
        request_adapter_options,
        device_descriptor,
    )
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
/// - `level_filter`: The logging level to be applied globally. If this is
///   not set, then default logging levels are used.
///
/// # Panics
///
/// - On **WASM32**, the function will panic if the `console_log` fails to
///   initialize.
fn init_logger(level_filter: Option<LevelFilter>) {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            let opt_logger = match level_filter {
                None => console_log::init(),
                Some(level_filt) => {
                    let level = level_filt.to_level().unwrap_or(log::Level::Warn);
                    console_log::init_with_level(level)
                }
            };
            opt_logger.expect("Could not initialize WASM32 logger.")
        } else {
            let mut builder = env_logger::Builder::from_default_env();
            level_filter.map(|level| builder.filter_level(level));
            builder.init()
        }
    }
}
