//! Модуль `system`: локальное время, память (`meminfo`), аптайм (`uptime`), загрузка CPU (`loadavg`),
//! диск (`statvfs`), сеть (`iw dev wlan0 link`), поиск процессов и свойства Android (`getprop`/`setprop`).

pub mod linux;

use anyhow::Result;
pub use linux::LinuxSystemDriver;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SystemTime {
    pub unix_sec: u64,
    pub unix_ms: u64,
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
    pub weekday: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SystemUptime {
    pub uptime_sec: f64,
    pub idle_sec: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SystemMemInfo {
    pub total_kb: u64,
    pub free_kb: u64,
    pub available_kb: u64,
    pub cached_kb: u64,
    pub used_pct: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SystemLoadAvg {
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
    pub running_threads: u32,
    pub total_threads: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SystemDiskInfo {
    pub total_mb: u64,
    pub free_mb: u64,
    pub available_mb: u64,
    pub used_pct: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemWifiInfo {
    pub connected: bool,
    pub iface: String,
    pub ip: String,
    pub ssid: String,
    pub bssid: String,
    pub freq_mhz: u32,
    pub rssi_dbm: i32,
    pub tx_mbps: f32,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemProcInfo {
    pub pid: u32,
    pub name: String,
    pub rss_kb: u64,
    pub threads: u32,
}

pub trait SystemHal: Send + Sync {
    fn time(&self) -> SystemTime;
    fn uptime(&self) -> Result<SystemUptime>;
    fn meminfo(&self) -> Result<SystemMemInfo>;
    fn loadavg(&self) -> Result<SystemLoadAvg>;
    fn disk(&self) -> Result<SystemDiskInfo>;
    fn wifi(&self) -> Result<SystemWifiInfo>;
    fn find_process(&self, query: &str) -> Result<Option<SystemProcInfo>>;
    fn getprop(&self, key: &str) -> Result<Option<String>>;
    fn setprop(&self, key: &str, value: &str) -> Result<()>;
}
