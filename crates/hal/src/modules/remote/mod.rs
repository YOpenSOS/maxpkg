//! Модуль `remote`: виртуальный пульт управления медиа и ТВ через `/dev/uinput`.

pub mod uinput;

use anyhow::Result;
pub use uinput::UinputRemote;

/// Трейт-разъём для виртуального пульта (`/dev/uinput`).
pub trait RemoteHal: Send + Sync {
    /// Отправляет одиночное нажатие и отпускание клавиши по её строковому имени
    /// (`"next"`, `"prev"`, `"play_pause"`, `"vol_up"`, `"vol_down"`, `"mute"`, `"up"`, `"down"`, `"left"`, `"right"`, `"ok"`, `"back"`, `"home"`, `"menu"`, `"power"`, `"action"`).
    fn send_key(&mut self, key_name: &str) -> Result<()>;
}
