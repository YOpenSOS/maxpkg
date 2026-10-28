//! Модуль `canvas`: матричный экран `25x16` (`/dev/graphics/fb4` через прямой `mmap` + управление яркостью).

pub mod fb4;
pub mod quasar_pause;

use anyhow::Result;
pub use fb4::{Fb4CanvasDriver, MATRIX_BYTES, MATRIX_H, MATRIX_PIXELS, MATRIX_W};

/// Трейт-разъём для матричного экрана `25x16`.
pub trait CanvasHal: Send + Sync {
    fn dimensions(&self) -> (usize, usize);
    fn clear_buf(&mut self);
    fn set_pixel(&mut self, x: usize, y: usize, br: u8);
    fn set_frame(&mut self, pixels: &[u8], brightness: Option<u8>) -> Result<()>;
    fn get_brightness(&self) -> Result<u8>;
    fn set_brightness(&mut self, br: u8) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
    fn release(&mut self) -> Result<()>;
}
