use bytemuck::{bytes_of, cast_slice, Pod, Zeroable};

use crate::{style, tiler, Color};

#[derive(Debug)]
pub struct Buffers {
    viewport_buffer: wgpu::Buffer,
    shader_options_buffer: wgpu::Buffer,
    tile_info_capacity: u32,
    tile_info_buffer: wgpu::Buffer,
    lines_buffer_capacity: u32,
    lines_buffer: wgpu::Buffer,
}
impl Buffers {
    pub fn new(device: &wgpu::Device, tile_info_capacity: u32, lines_buffer_capacity: u32) -> Self {
        Buffers {
            viewport_buffer: create_viewport_buffer(device),
            shader_options_buffer: create_shader_options_buffer(device),
            tile_info_capacity,
            tile_info_buffer: create_tile_info_buffer(device, tile_info_capacity),
            lines_buffer_capacity,
            lines_buffer: create_line_buffer(device, lines_buffer_capacity),
        }
    }

    /// Returns a reference to the viewport buffer.
    pub fn viewport_buffer(&self) -> &wgpu::Buffer {
        &self.viewport_buffer
    }

    /// Returns a reference to the shader options buffer.
    pub fn shader_options_buffer(&self) -> &wgpu::Buffer {
        &self.shader_options_buffer
    }

    /// Returns a reference to the tile info buffer.
    pub fn tile_info_buffer(&self) -> &wgpu::Buffer {
        &self.tile_info_buffer
    }

    /// Returns a reference to the lines buffer.
    pub fn lines_buffer(&self) -> &wgpu::Buffer {
        &self.lines_buffer
    }

    /// Write the viewport parameters into the viewport buffer.
    ///
    /// # Parameters
    ///
    /// - `queue`: WGPU queue to enqueue the buffer write.
    /// - `area_width`: Width of the renderable area.
    /// - `area_height`: Height of the renderable area.
    /// - `tile_width`: Width of a single tile.
    /// - `tile_height`: Height of a single tile.
    pub fn write_viewport_buffer(
        &self,
        queue: &wgpu::Queue,
        area_width: u32,
        area_height: u32,
        tile_width: u32,
        tile_height: u32,
    ) {
        let viewport = Viewport {
            area_width,
            area_height,
            tile_width,
            tile_height,
        };
        queue.write_buffer(&self.viewport_buffer, 0, bytes_of(&viewport));
    }

    /// Write the shader options to their buffer.
    ///
    /// # Parameters
    ///
    /// - `queue`: WGPU queue to enqueue the buffer write.
    /// - `antialias_width`: Width of antialiasing smoothstep.
    /// - `draw_tiles`: `true` if individual tiles should be drawn for
    ///   visualization.
    /// - `tile_background`: tile background color.
    /// - `tile_edge`: tile edges color.
    pub fn write_shader_options(
        &self,
        queue: &wgpu::Queue,
        antialias_width: f32,
        draw_tiles: bool,
        tile_background: Color,
        tile_edges: Color,
    ) {
        let shader_options = ShaderOptions {
            antialias_width,
            draw_tiles: if draw_tiles { 1 } else { 0 },
            tile_background: tile_background.as_array(),
            tile_edges: tile_edges.as_array(),
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.shader_options_buffer, 0, bytes_of(&shader_options));
    }

    /// Write tile info to its buffer.
    ///
    /// If the tile info buffer is not large enough, it is re-allocated with
    /// a large enough capacity.
    ///
    /// # Parameters
    ///
    /// - `device`: WGPU Device.
    /// - `queue`: WGPU queue to enqueue the buffer write.
    /// - `tile_info`: Tile info structs from the [`Tiler`].
    pub fn write_tile_info(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tile_info: Vec<tiler::TileInfo>,
    ) {
        if self.tile_info_capacity < tile_info.len() as u32 {
            self.grow_tile_info(device, tile_info.len() as u32);
        }

        let gpu_tile_info: Vec<TileInfo> = tile_info
            .into_iter()
            .map(|tile_info| TileInfo::new_from_tiler_tileinfo(tile_info))
            .collect();
        queue.write_buffer(&self.tile_info_buffer, 0, cast_slice(&gpu_tile_info));
    }

    /// Write line array to its buffer.
    ///
    /// If the line array buffer is not large enough, it is re-allocated with
    /// a large enough capacity.
    ///
    /// # Parameters
    ///
    /// - `device`: WGPU Device.
    /// - `queue`: WGPU queue to enqueue the buffer write.
    /// - `styled_lines`: Styled line structs from the [`Tiler`].
    pub fn write_line_array(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        styled_lines: Vec<style::StyledLine>,
    ) {
        if self.lines_buffer_capacity < styled_lines.len() as u32 {
            self.grow_lines(device, styled_lines.len() as u32);
        }

        let gpu_styled_lines: Vec<StyledLine> = styled_lines
            .into_iter()
            .map(|styled_line| StyledLine::new_from_style_line(styled_line))
            .collect();
        queue.write_buffer(&self.lines_buffer, 0, cast_slice(&gpu_styled_lines));
    }

    /// Grow the tile info buffer to a new size.
    ///
    /// # Parameters
    ///
    /// - `device`: WGPU Device.
    /// - `new_capacity`: New size of the buffer.
    fn grow_tile_info(&mut self, device: &wgpu::Device, new_capacity: u32) {
        assert!(new_capacity > self.tile_info_capacity);
        self.tile_info_buffer = create_tile_info_buffer(device, new_capacity);
        self.tile_info_capacity = new_capacity;
    }

    /// Grow the line array buffer to a new size.
    ///
    /// # Parameters
    ///
    /// - `device`: WGPU Device.
    /// - `new_capacity`: New size of the buffer.
    fn grow_lines(&mut self, device: &wgpu::Device, new_capacity: u32) {
        assert!(new_capacity > self.lines_buffer_capacity);
        self.lines_buffer = create_line_buffer(device, new_capacity);
        self.lines_buffer_capacity = new_capacity;
    }
}

/// Create the viewport uniform buffer.
fn create_viewport_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    let viewport: Viewport = Default::default();
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Beamline: Viewport unifom"),
        contents: bytes_of(&viewport),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

/// Create the shader options uniform buffer.
fn create_shader_options_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    let shader_options: ShaderOptions = Default::default();
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Beamline: Shader options unifom"),
        contents: bytes_of(&shader_options),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

/// Create the tile info buffer.
///
/// # Parameters
///
/// - `device`: WGPU Device.
/// - `capacity`: Number of `TileInfo` structs that the buffer can store.
fn create_tile_info_buffer(device: &wgpu::Device, capacity: u32) -> wgpu::Buffer {
    use wgpu::BufferAddress;
    let struct_sz = std::mem::size_of::<TileInfo>() as BufferAddress;
    let buf_sz_bytes = struct_sz * capacity as BufferAddress;

    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Beamline: Tile info buffer"),
        size: buf_sz_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Create the line array buffer.
///
/// # Parameters
///
/// - `device`: WGPU Device.
/// - `capacity`: Number of `StyledLine` structs that the buffer can store.
fn create_line_buffer(device: &wgpu::Device, capacity: u32) -> wgpu::Buffer {
    use wgpu::BufferAddress;
    let struct_sz = std::mem::size_of::<StyledLine>() as BufferAddress;
    let buf_sz_bytes = struct_sz * capacity as BufferAddress;

    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Beamline: Line array buffer"),
        size: buf_sz_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// GPU version of the viewport information, for the viewport uniform buffer.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Pod, Zeroable)]
struct Viewport {
    area_width: u32,
    area_height: u32,
    tile_width: u32,
    tile_height: u32,
}

/// GPU version of shader options.
#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Pod, Zeroable)]
struct ShaderOptions {
    tile_background: [f32; 4], // 16 bytes
    tile_edges: [f32; 4],      // 16 bytes
    antialias_width: f32,      // 4 bytes
    draw_tiles: u32,           // 4 bytes
    _padding: [f32; 2],
}

/// GPU version of the tile info.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct TileInfo {
    tile_x: u32,
    tile_y: u32,
    start_index: u32,
    n_lines: u32,
}
impl TileInfo {
    pub fn new_from_tiler_tileinfo(tile_info: tiler::TileInfo) -> Self {
        TileInfo {
            tile_x: tile_info.tile_x,
            tile_y: tile_info.tile_y,
            start_index: tile_info.start_index,
            n_lines: tile_info.n_lines,
        }
    }
}

/// GPU version of styled line.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct StyledLine {
    start: [f32; 2],     // 8 bytes
    end: [f32; 2],       // 8 bytes
    width: f32,          // 4 bytes
    cap: u32,            // 4 bytes
    _padding0: [f32; 2], // 4 bytes
    color: [f32; 4],     // 16 bytes
}
impl StyledLine {
    pub fn new_from_style_line(styled_line: style::StyledLine) -> Self {
        StyledLine {
            start: [styled_line.line.start().x, styled_line.line.start().y],
            end: [styled_line.line.end().x, styled_line.line.end().y],
            width: styled_line.style.width,
            cap: styled_line.style.cap as u32,
            _padding0: [0.0, 0.0],
            color: styled_line.style.color.as_array(),
        }
    }
}
