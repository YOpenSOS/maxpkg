//! Hardware Abstraction Layer (HAL) и C-ABI ядро (`libmax_hal`) для `max-platform`.
//! Поддерживает разработку нативных пакетов на **Rust**, **C**, **C++**, **Go** и **Zig**.

#![cfg_attr(not(unix), allow(dead_code, unused_imports, unused_mut))]

pub mod ffi;
pub mod modules;

pub use modules::arbiter::lease::{
    current_process_context, install_signal_handlers, is_stop_requested,
    set_current_process_context,
};
pub use modules::arbiter::{Priority, ResourceKind, ResourceLease};
pub use modules::audio::{AmpConfig, AmpStatus, AudioHal, MediaSessionInfo, StationMaxAudioDriver};
pub use modules::bluetooth::{BluedroidDriver, BluetoothDevice, BluetoothHal, BluetoothStatus};
pub use modules::canvas::{CanvasHal, Fb4CanvasDriver, MATRIX_BYTES, MATRIX_H, MATRIX_PIXELS, MATRIX_W};
pub use modules::device::{detect_device, DeviceProfile, HardwareSpecs, KernelNodeMap};
pub use modules::events::{EventBus, EventKind, MaxEvent};
pub use modules::input::{EvdevDriver, InputEvent, InputHal};
pub use modules::mic::{AudioFlingerShmDriver, MicHal, MicInfo};
pub use modules::remote::{RemoteHal, UinputRemote};
pub use modules::ring::{Fb5RingDriver, RingHal, RING_BYTES, RING_LEDS};
pub use modules::sensors::{LightReading, SensorItem, SensorsHal, SysfsSensorsDriver, ThermalReading};
pub use modules::system::{
    LinuxSystemDriver, SystemDiskInfo, SystemHal, SystemLoadAvg, SystemMemInfo, SystemProcInfo,
    SystemTime, SystemUptime, SystemWifiInfo,
};
pub use modules::{BoardCapabilities, HAL_MODULE_SLOTS};

/// Единая точка инициализации нативного Rust-пакета:
/// регистрирует обработчики `SIGINT`/`SIGTERM`, подхватывает `MAX_PKG_ID`/`MAX_PKG_PRIORITY`/`MAX_PKG_REPLACE`
/// и возвращает паспорт устройства `DeviceProfile`.
pub fn init() -> DeviceProfile {
    install_signal_handlers();
    let _ = current_process_context();
    detect_device(None)
}

/// Проверяет, получен ли сигнал остановки (`SIGTERM`/`SIGINT` от `maxctl stop` или `--replace`).
pub fn should_stop() -> bool {
    is_stop_requested()
}

/// Спит `ms` мс с проверкой `SIGTERM` каждые 10 мс. Возвращает `false`, если пакет должен завершиться.
pub fn sleep_ms(ms: u64) -> bool {
    ffi::max_hal_sleep_ms(ms as u32) != 0
}

/// Записывает файл в приватное хранилище пакета (`MAX_PKG_DATA/<name>`).
pub fn write_data(name: &str, content: &str) -> anyhow::Result<()> {
    let Some(path) = ffi::system::pkg_data_file(name) else {
        anyhow::bail!("некорректное имя файла данных: {name}");
    };
    std::fs::write(path, content)?;
    Ok(())
}

/// Читает файл из приватного хранилища пакета (`MAX_PKG_DATA/<name>`).
pub fn read_data(name: &str) -> anyhow::Result<Option<String>> {
    let Some(path) = ffi::system::pkg_data_file(name) else {
        anyhow::bail!("некорректное имя файла данных: {name}");
    };
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(std::fs::read_to_string(path)?))
}

/// Удобный реэкспорт всех типов и трейтов для нативных Rust-пакетов (`use max_hal::prelude::*;`).
pub mod prelude {
    pub use crate::{
        init, read_data, should_stop, sleep_ms, write_data, AmpConfig, AmpStatus,
        AudioFlingerShmDriver, AudioHal, BluedroidDriver, BluetoothDevice, BluetoothHal,
        BluetoothStatus, CanvasHal, DeviceProfile, EvdevDriver, EventBus, EventKind,
        Fb4CanvasDriver, Fb5RingDriver, InputEvent, InputHal, LightReading, LinuxSystemDriver,
        MaxEvent, MediaSessionInfo, MicHal, MicInfo, Priority, RemoteHal, RingHal, SensorsHal, StationMaxAudioDriver, SysfsSensorsDriver,
        SystemHal, ThermalReading, UinputRemote, MATRIX_H, MATRIX_PIXELS, MATRIX_W, RING_LEDS,
    };
}
