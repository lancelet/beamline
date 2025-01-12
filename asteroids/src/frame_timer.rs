#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[derive(Debug)]
pub struct FrameTimer {
    first_tick: Instant,
    last_tick: Instant,
}
impl FrameTimer {
    pub fn new() -> Self {
        let now = Instant::now();
        FrameTimer {
            first_tick: now,
            last_tick: now,
        }
    }

    pub fn tick_millis(&mut self) -> u128 {
        let new_tick = Instant::now();
        let duration = new_tick.duration_since(self.last_tick);
        self.last_tick = new_tick;
        duration.as_millis()
    }

    pub fn total_time_secs_f64(&self) -> f64 {
        Instant::now().duration_since(self.first_tick).as_secs_f64()
    }
}
