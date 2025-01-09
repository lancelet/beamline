#[allow(unused)] // TODO: For development.
mod wgpu_context;

use cfg_if::cfg_if;
use log::{trace, warn, LevelFilter};
use std::sync::Arc;
use wgpu::{util::DeviceExt, SurfaceConfiguration};
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
    //event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.set_control_flow(ControlFlow::Wait);

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

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    width: u32,
    height: u32,
    bucket_width: u32,
    bucket_height: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceOffsets {
    tile_x: u32,
    tile_y: u32,
    line_start_index: u32,
    line_count: u32,
}

#[derive(Debug, Default)]
pub struct App {
    /// The Application's winit window.
    window: Option<Arc<Window>>,
    /// WGPU context - has async setup.
    wgpu_context: Option<FutureWgpuContext>,
    /// Flag to indicate whether all WGPU setup has finished.
    extra_wgpu_setup_completed: bool,
    /// Surface configuration for WGPU.
    surface_configuration: Option<wgpu::SurfaceConfiguration>,
    /// Render pipeline for WGPU.
    render_pipeline: Option<wgpu::RenderPipeline>,
    /// Layout for the camera bind group.
    camera_bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Camera uniform buffer.
    camera_buffer: Option<wgpu::Buffer>,
    /// Layout for the instance offsets bind group.
    instance_layout: Option<wgpu::BindGroupLayout>,
    /// Instance offsets buffer.
    instance_offsets_buffer: Option<wgpu::Buffer>,
}
impl App {
    /// Override the application logging level.
    ///
    /// Set this to override the logging level for both **WASM32** and
    /// **Native** applications.
    const LOG_LEVEL_FILTER: Option<LevelFilter> = Some(LevelFilter::Trace);

    /// Background color.
    const BACKGROUND_COLOR: wgpu::Color = wgpu::Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    };

    /// Number of instance offsets (ie. number of drawn buckets).
    const N_INSTANCE_OFFSETS: u64 = (3640 / 16) * (2160 / 16);

    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            const CANVAS_ID: &str = "linerender-host-canvas";
            const BACKENDS: wgpu::Backends = wgpu::Backends::BROWSER_WEBGPU;
        } else {
            const BACKENDS: wgpu::Backends = wgpu::Backends::PRIMARY;
        }
    }

    /// TEMPORARY: Provide some example instance offsets.
    fn example_instance_offsets() -> Vec<InstanceOffsets> {
        vec![
            InstanceOffsets {
                tile_x: 1,
                tile_y: 1,
                line_start_index: 0,
                line_count: 0,
            },
            InstanceOffsets {
                tile_x: 2,
                tile_y: 0,
                line_start_index: 0,
                line_count: 0,
            },
            InstanceOffsets {
                tile_x: 0,
                tile_y: 0,
                line_start_index: 0,
                line_count: 0,
            },
        ]
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
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or({
                warn!(
                    "Could not select sRGB surface format. Falling back to \
                     first format available."
                );
                surface_caps.formats[0]
            });
        trace!("Surface format: {:?}", surface_format);

        let size = self.window().inner_size();
        let surface_configuration = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
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

    /// Set up the render pipeline.
    fn create_render_pipeline(&mut self) {
        let ctx = self.wgpu_context();
        let device = ctx.device();

        let shader_module_descriptor = wgpu::include_wgsl!("shader.wgsl");
        let shader = device.create_shader_module(shader_module_descriptor);

        // Camera bind group
        let camera_layout_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let camera_bind_group_layout_descriptor = wgpu::BindGroupLayoutDescriptor {
            entries: &[camera_layout_entry],
            label: Some("Camera Bind Group Layout"),
        };
        let camera_bind_group_layout =
            device.create_bind_group_layout(&camera_bind_group_layout_descriptor);

        // Instance offsets bind group
        let instance_offsets_layout_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let instance_layout_descriptor = wgpu::BindGroupLayoutDescriptor {
            entries: &[instance_offsets_layout_entry],
            label: Some("Bind Group Layout for Instances"),
        };
        let instance_layout = device.create_bind_group_layout(&instance_layout_descriptor);

        let pipeline_layout_descriptor = wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &instance_layout],
            push_constant_ranges: &[],
        };
        let render_pipeline_layout = device.create_pipeline_layout(&pipeline_layout_descriptor);
        let vertex_state = wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        };
        let color_target_state = wgpu::ColorTargetState {
            format: self.surface_configuration().format,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        };
        let fragment_state = wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(color_target_state)],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        };
        let primitive_state = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        };
        let multisample_state = wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        };
        let render_pipeline_descriptor = wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: vertex_state,
            fragment: Some(fragment_state),
            primitive: primitive_state,
            depth_stencil: None,
            multisample: multisample_state,
            multiview: None,
            cache: None,
        };
        let render_pipeline = device.create_render_pipeline(&render_pipeline_descriptor);

        self.camera_bind_group_layout = Some(camera_bind_group_layout);
        self.instance_layout = Some(instance_layout);
        self.render_pipeline = Some(render_pipeline);
    }

    /// Return a reference to the WGPU RenderPipeline.
    fn render_pipeline(&self) -> &wgpu::RenderPipeline {
        self.render_pipeline.as_ref().unwrap()
    }

    /// Create the camera buffer; large enough to contain one CameraUniform.
    fn create_camera_buffer(&mut self) {
        let camera_uniform: [CameraUniform; 1] = [Default::default()];
        let device = self.wgpu_context().device();
        let buffer_init_descriptor = wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer: View Parameters"),
            contents: bytemuck::cast_slice(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        };
        let camera_buffer = device.create_buffer_init(&buffer_init_descriptor);
        self.camera_buffer = Some(camera_buffer);
    }

    /// Return the camera buffer.
    fn camera_buffer(&self) -> &wgpu::Buffer {
        self.camera_buffer.as_ref().unwrap()
    }

    /// Create the instance offsets buffer.
    fn create_instance_offsets_buffer(&mut self) {
        let buffer_size_bytes = (App::N_INSTANCE_OFFSETS as wgpu::BufferAddress)
            * (std::mem::size_of::<InstanceOffsets>() as wgpu::BufferAddress);
        let device = self.wgpu_context().device();
        let buffer_descriptor = wgpu::BufferDescriptor {
            label: Some("Instance Offsets Buffer"),
            size: buffer_size_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        };
        let instance_offsets_buffer = device.create_buffer(&buffer_descriptor);
        self.instance_offsets_buffer = Some(instance_offsets_buffer);
    }

    /// Return the instance offsets buffer.
    fn instance_offsets_buffer(&self) -> &wgpu::Buffer {
        self.instance_offsets_buffer.as_ref().unwrap()
    }

    /// Return the layout of the camera bind group.
    fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.camera_bind_group_layout.as_ref().unwrap()
    }

    /// Return the layout of the instance bind group.
    fn instance_layout(&self) -> &wgpu::BindGroupLayout {
        self.instance_layout.as_ref().unwrap()
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
            {
                let cfg = self.surface_configuration_mut();
                cfg.width = size.width;
                cfg.height = size.height;
            }
            let ctx = self.wgpu_context();
            ctx.surface()
                .configure(ctx.device(), self.surface_configuration())
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
                self.create_render_pipeline();
                self.create_camera_buffer();
                self.create_instance_offsets_buffer();
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
    fn render(&self) -> Result<(), wgpu::SurfaceError> {
        // Bail if setup has not completed.
        if !self.extra_wgpu_setup_completed {
            return Ok(());
        }

        let ctx = self.wgpu_context();
        let device = ctx.device();

        let output_texture = ctx.surface().get_current_texture()?;
        let view = output_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.wgpu_context()
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Command Encoder"),
                });

        // Set up the camera buffer.
        let size = self.window().inner_size();
        let camera_uniform = CameraUniform {
            width: size.width,
            height: size.height,
            bucket_width: 32,
            bucket_height: 32,
        };
        ctx.queue()
            .write_buffer(self.camera_buffer(), 0, bytemuck::bytes_of(&camera_uniform));

        // Set up the instance offsets buffer.
        let instance_offsets = App::example_instance_offsets();
        ctx.queue().write_buffer(
            self.instance_offsets_buffer(),
            0,
            bytemuck::cast_slice(&instance_offsets),
        );

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: self.camera_bind_group_layout(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.camera_buffer().as_entire_binding(),
            }],
            label: Some("Camera Bind Group"),
        });

        let instance_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: self.instance_layout(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.instance_offsets_buffer().as_entire_binding(),
            }],
            label: Some("Instance Bind Group"),
        });

        {
            let rpca = wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(App::BACKGROUND_COLOR),
                    store: wgpu::StoreOp::Store,
                },
            };

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(rpca)],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(self.render_pipeline());
            render_pass.set_bind_group(0, &camera_bind_group, &[]);
            render_pass.set_bind_group(1, &instance_bind_group, &[]);

            let n_instances = instance_offsets.len() as u32;
            render_pass.draw(0..6, 0..n_instances); // 6 vertices
        }

        self.wgpu_context()
            .queue()
            .submit(std::iter::once(encoder.finish()));
        output_texture.present();

        Ok(())
    }

    /// Redraw the window: render a frame and handle any errors.
    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        // Request a new redraw after this one.
        self.window().request_redraw();
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
        self.finish_wgpu_static_setup();

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
