use super::{pushbuf, pushbuf::PushBuf, types::StyledLine};
use std::sync::Arc;
use wgpu::{BufferUsages, CommandBuffer, Device};

pub struct Tiler {
    area_width: u32,
    area_height: u32,
    tile_width: u32,
    tile_height: u32,
    pushbuf: PushBuf<StyledLine>,
}
impl Tiler {
    const CHUNK_SIZE: usize = 16;

    pub fn new(
        device: Arc<Device>,
        area_width: u32,
        area_height: u32,
        tile_width: u32,
        tile_height: u32,
        line_capacity: usize,
    ) -> Tiler {
        Tiler {
            area_width,
            area_height,
            tile_width,
            tile_height,
            pushbuf: PushBuf::new(
                device,
                Some("Tiler Line Buffer"),
                BufferUsages::STORAGE,
                line_capacity,
                Tiler::CHUNK_SIZE,
            ),
        }
    }

    pub fn begin_frame(&mut self) {
        self.pushbuf.begin_frame();
    }

    pub fn push(
        &mut self,
        styled_line: StyledLine,
    ) -> Result<(), pushbuf::Error> {
        self.pushbuf.push(styled_line)
    }

    pub fn end_frame(&mut self) -> Vec<CommandBuffer> {
        vec![self.pushbuf.end_frame()]
    }

    pub fn recall(&mut self) {
        self.pushbuf.recall();
    }
}
