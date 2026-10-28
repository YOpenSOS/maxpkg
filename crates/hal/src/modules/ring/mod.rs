//! Модуль `ring`: верхнее светодиодное кольцо (`/dev/graphics/fb5` через прямой `mmap` + `FBIOPAN_DISPLAY`).

pub mod fb5;

use anyhow::Result;
pub use fb5::{hsv_to_rgb, parse_hex_color, Fb5RingDriver, RING_BYTES, RING_LEDS};

/// Трейт-разъём для управления светодиодным кольцом.
pub trait RingHal: Send + Sync {
    fn led_count(&self) -> usize;
    fn clear_buf(&mut self);
    fn set_pixel(&mut self, idx: usize, r: u8, g: u8, b: u8);
    fn fill(&mut self, r: u8, g: u8, b: u8) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
    fn off(&mut self) -> Result<()>;
}
