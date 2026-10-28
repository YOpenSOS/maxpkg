//! Модуль `device`: идентификация колонки, ревизии, ОС и аппаратных параметров.

pub mod detect;
pub mod types;

pub use detect::detect_device;
pub use types::{DeviceProfile, HardwareSpecs, KernelNodeMap};
