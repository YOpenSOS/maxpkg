//! Модуль `sensors`: датчик освещённости (`IIO ltr308als01`), 3 зоны температуры (`soc`, `ddr`, `wifi`),
//! частота CPU и аппаратные часы RTC (`pcf8563`).

pub mod sysfs;

use anyhow::Result;
use serde::{Deserialize, Serialize};
pub use sysfs::SysfsSensorsDriver;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightReading {
    pub lux: f32,
    pub raw: u32,
    pub gain: u8,
    pub integration_ms: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalReading {
    pub soc_c: f32,
    pub ddr_c: f32,
    pub wifi_c: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorItem {
    pub id: String,
    pub kind: String,
    pub unit: String,
    pub value: f32,
}

/// Трейт-разъём для бортовых датчиков колонки.
pub trait SensorsHal: Send + Sync {
    fn read_light(&self) -> Result<LightReading>;
    fn configure_light(&mut self, gain: u8, integration_ms: u16) -> Result<()>;
    fn read_thermal(&self) -> Result<ThermalReading>;
    fn read_cpu_freq_mhz(&self) -> Result<u32>;
    fn read_rtc_epoch(&self) -> Result<u64>;
    fn list_all(&self) -> Result<Vec<SensorItem>>;
}
