use crate::{
    buffers::Buffers,
    style::{LineStyle, StyledLine},
    tiler::Tiler,
    Color, Line,
};

#[derive(Debug)]
pub struct Renderer {
    area_width: u32,
    area_height: u32,
    tile_width: u32,
    tile_height: u32,
    tiler: Tiler,
    draw_tiles: bool,
    tile_background: Color,
    tile_edges: Color,
    render_pipeline: wgpu::RenderPipeline,
    viewport_layout: wgpu::BindGroupLayout,
    tile_layout: wgpu::BindGroupLayout,
    buffers: Buffers,
}

impl Renderer {
    /// Creates a new `Renderer`.
    ///
    /// # Parameters
    ///
    /// - `device`: WGPU Device for rendering.
    /// - `area_width`: Width of the renderable area.
    /// - `area_height`: Height of the renderable area.
    /// - `tile_width`: Width of a single bucketing tile.
    /// - `tile_height`: Height of a single bucketing tile.
    pub fn new(
        device: &wgpu::Device,
        area_width: u32,
        area_height: u32,
        tile_width: u32,
        tile_height: u32,
    ) -> Self {
        assert!(area_width > 0);
        assert!(area_height > 0);
        assert!(tile_width > 0);
        assert!(tile_height > 0);

        const DEFAULT_TILE_INFO_CAPACITY: u32 = 1024;
        const DEFAULT_LINES_BUFFER_CAPACITY: u32 = 1024;

        let tiler = Tiler::new(area_width, area_height, tile_width, tile_height);
        let viewport_layout = create_viewport_layout(device);
        let tile_layout = create_tile_layout(device);
        let render_pipeline = create_render_pipeline(device, &viewport_layout, &tile_layout);
        let buffers = Buffers::new(
            device,
            DEFAULT_TILE_INFO_CAPACITY,
            DEFAULT_LINES_BUFFER_CAPACITY,
        );

        Renderer {
            area_width,
            area_height,
            tile_width,
            tile_height,
            tiler,
            draw_tiles: false,
            tile_background: Color::new(0.0, 0.0, 0.0, 0.0),
            tile_edges: Color::new(0.0, 0.0, 0.0, 0.0),
            render_pipeline,
            viewport_layout,
            tile_layout,
            buffers,
        }
    }

    /// Adds a line to be rendered.
    ///
    /// This queues a line to be rendered. The actual rendering does not happen
    /// until [`Renderer::render`] is called.
    ///
    /// # Parameters
    ///
    /// - `line`: Line to render.
    /// - `style`: Style of the line to render.
    pub fn line(&mut self, line: Line, style: &LineStyle) {
        self.tiler.add(StyledLine {
            line,
            style: style.clone(),
        })
    }

    /// Resizes the renderer.
    ///
    /// When the screen is re-sized, this method must be called. This resets
    /// the renderer, removing any lines that might have been queued for
    /// rendering.
    ///
    /// # Parameters
    ///
    /// - `area_width`: Width of the rendering area.
    /// - `area_height`: Height of the rendering area.
    pub fn resize(&mut self, area_width: u32, area_height: u32) {
        assert!(area_width > 0);
        assert!(area_height > 0);

        self.tiler.resize(area_width, area_height);
        self.area_height = area_height;
        self.area_width = area_width;
    }

    /// Render the current set of lines, by adding them to the render queue.
    ///
    /// # Parameters
    ///
    /// - `device`: WGPU Device to use.
    /// - `encoder`: Command encoder to which commands should be submitted.
    /// - `queue`: WGPU Queue to use.
    /// - `output_texture`: Texture view to write the output.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        output_texture: &wgpu::TextureView,
    ) {
        // Set up the current viewport.
        self.buffers.write_viewport_buffer(
            queue,
            self.area_width,
            self.area_height,
            self.tile_width,
            self.tile_height,
        );
        // Set up the shader options.
        self.buffers.write_shader_options(
            queue,
            self.draw_tiles,
            self.tile_background,
            self.tile_edges,
        );

        // Set up viewport bind group.
        let viewport_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Beamline: Viewport bind group."),
            layout: &self.viewport_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.buffers.viewport_buffer().as_entire_binding(),
            }],
        });

        // Fetch tile info and styled lines from the tiler.
        let (tile_infos, styled_lines) = self.tiler.drain();
        let n_instances = tile_infos.len() as u32;
        self.buffers.write_tile_info(device, queue, tile_infos);
        self.buffers.write_line_array(device, queue, styled_lines);

        // Set up the tile bind group.
        let tile_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Beamline: Tile bind group."),
            layout: &self.tile_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.buffers.shader_options_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.buffers.tile_info_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.buffers.lines_buffer().as_entire_binding(),
                },
            ],
        });

        // Create the render pass.
        {
            let color_attachment = wgpu::RenderPassColorAttachment {
                view: &output_texture,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            };

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Beamline: Line render pass"),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &viewport_bind_group, &[]);
            render_pass.set_bind_group(1, &tile_bind_group, &[]);
            render_pass.draw(0..6, 0..n_instances);
        }
    }
}

/// Create the render pipeline.
fn create_render_pipeline(
    device: &wgpu::Device,
    viewport_layout: &wgpu::BindGroupLayout,
    tile_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader_module_descriptor = wgpu::include_wgsl!("line_sdf.wgsl");
    let shader = device.create_shader_module(shader_module_descriptor);

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Beamline: Line render pipeline layout."),
        bind_group_layouts: &[viewport_layout, tile_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Beamline: Line render pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Bgra8Unorm, // TODO
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    })
}

/// Create the bind group layout for the viewport.
///
/// At render time, this contains the:
///   - viewport size
///   - bucket size
fn create_viewport_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    use wgpu::{BufferBindingType::Uniform, ShaderStages};
    let vis = ShaderStages::VERTEX | ShaderStages::FRAGMENT;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Beamline: Viewport bind group layout."),
        entries: &[bind_group_layout_entry(0, vis, Uniform)],
    })
}

/// Create the bind group layout for tile information.
///
/// At render time, this contains:
///   - shader parameters
///   - tile instance information
///   - array of lines
fn create_tile_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    use wgpu::{
        BufferBindingType::{Storage, Uniform},
        ShaderStages,
    };
    let vis_vf = ShaderStages::VERTEX | ShaderStages::FRAGMENT;
    let vis_f = ShaderStages::FRAGMENT;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Beamline: Tile bind group layout."),
        entries: &[
            // Binding 0: Shader parameters.
            bind_group_layout_entry(0, vis_vf, Uniform),
            // Binding 1: Tile instance information.
            bind_group_layout_entry(1, vis_vf, Storage { read_only: true }),
            // Binding 2: Line array.
            bind_group_layout_entry(2, vis_f, Storage { read_only: true }),
        ],
    })
}

/// Create a bind group layout entry.
fn bind_group_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    binding_type: wgpu::BufferBindingType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        count: None,
        ty: wgpu::BindingType::Buffer {
            ty: binding_type,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
    }
}
