//! Модуль `bluetooth`: прямое чтение конфигурации `Bluedroid` (`/data/misc/bluedroid/bt_config.conf`),
//! статуса ядра `/sys/class/bluetooth/hci0` и управление питанием адаптера с откатом в `Drop`.

pub mod bluedroid;

use anyhow::Result;
pub use bluedroid::BluedroidDriver;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothStatus {
    pub enabled: bool,
    pub address: String,
    pub name: String,
    pub scan_mode: u8,
    pub connected: bool,
    pub bonded_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothDevice {
    pub mac: String,
    pub name: String,
    pub kind: String,
    pub timestamp: u64,
}

/// Трейт-разъём для работы с Bluetooth-адаптером колонки.
pub trait BluetoothHal: Send + Sync {
    fn status(&self) -> Result<BluetoothStatus>;
    fn devices(&self) -> Result<Vec<BluetoothDevice>>;
    fn set_power(&mut self, on: bool) -> Result<()>;
}
