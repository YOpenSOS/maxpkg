//! Модуль `input`: физические кнопки (`action`, `mute`), крутилка (`wheel`), штекер `AUX`, перехват `EVIOCGRAB` и статус `EVIOCGSW`.

pub mod evdev;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub use evdev::EvdevDriver;

/// Декодированное событие ввода от кнопок или энкодера колонки.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputEvent {
    pub source: String, // "action" | "mute" | "wheel" | "headphone" | "raw"
    pub action: String, // "down" | "up" | "repeat" | "muted" | "unmuted" | "rotate" | "inserted" | "removed"
    pub delta: i32,     // +1 / -1 для крутилки, 0 для кнопок
    pub held_ms: u64,   // Длительность удержания кнопки или состояния мута в мс
    pub code: u16,      // Сырой код ядра (582, 14, 104, 109, 2)
    pub value: i32,     // Сырое значение (0, 1, 2)
    pub time_ms: u64,   // Метка времени прерывания ядра (мс)
}

impl InputEvent {
    pub fn new(
        source: &str,
        action: &str,
        delta: i32,
        held_ms: u64,
        code: u16,
        value: i32,
        time_ms: u64,
    ) -> Self {
        Self {
            source: source.into(),
            action: action.into(),
            delta,
            held_ms,
            code,
            value,
            time_ms,
        }
    }
}

/// Трейт-разъём для аппаратного драйвера кнопок и крутилки на корпусе колонки.
pub trait InputHal: Send + Sync {
    fn is_mic_muted(&self) -> Result<bool>;
    fn is_aux_inserted(&self) -> Result<bool>;
    fn get_wheel_step(&self) -> Result<u8>;
    fn set_wheel_step(&mut self, step: u8) -> Result<()>;
    fn set_grab(&mut self, grab: bool) -> Result<()>;
    fn next_event(&mut self, timeout_ms: u64) -> Result<Option<InputEvent>>;
}
