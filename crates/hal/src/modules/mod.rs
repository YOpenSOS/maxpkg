//! Реестр всех аппаратных модулей (каждый модуль — в своей папке).

pub mod arbiter;
pub mod audio;
pub mod bluetooth;
pub mod canvas;
pub mod device;
pub mod events;
pub mod input;
pub mod mic;
pub mod remote;
pub mod ring;
pub mod sensors;
pub mod system;

use self::device::DeviceProfile;

/// Полный список аппаратных модулей платформы и их требуемых возможностей (`feature`).
pub const HAL_MODULE_SLOTS: &[(&str, Option<&str>)] = &[
    ("device", None),
    ("events", None),
    ("input", Some("input")),
    ("remote", Some("remote.uinput")),
    ("ring", Some("ring")),
    ("canvas", Some("canvas")),
    ("sensors", Some("sensors")),
    ("audio", Some("audio")),
    ("mic", Some("mic")),
    ("bluetooth", Some("bluetooth")),
    ("system", None),
];

/// Единый интерфейс проверки доступности аппаратных модулей на текущей плате.
pub struct BoardCapabilities {
    pub profile: DeviceProfile,
}

impl BoardCapabilities {
    pub fn detect(forced_model: Option<&str>) -> Self {
        Self {
            profile: device::detect_device(forced_model),
        }
    }

    /// Проверяет, поддерживает ли текущая колонка указанный слот модуля.
    pub fn has_capability(&self, name: &str) -> bool {
        self.profile
            .features
            .iter()
            .any(|f| f == name || f.starts_with(&format!("{name}.")))
    }
}
