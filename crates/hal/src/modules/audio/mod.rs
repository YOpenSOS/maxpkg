//! Модуль `audio`: системная громкость, аппаратные усилители `SY6045S` (`Woofer`/`Tweeters`/`AUX`/`EQ`/`DRC`),
//! нативное воспроизведение звука через `AAudio` (`libaaudio.so`) и метаданные треков (`media_session`).

pub mod aaudio;
pub mod alsa_ctl;
pub mod media_session;
pub mod sound_override;

use anyhow::Result;
pub use alsa_ctl::StationMaxAudioDriver;
pub use media_session::MediaSessionInfo;
pub use sound_override::SoundOverrideManager;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Текущее состояние двух цифровых усилителей `SY6045S` (сабвуфер и твитеры) и ЦАП наушников.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AmpStatus {
    pub woofer: u8,
    pub tweeters: u8,
    pub aux: u8,
    pub woofer_on: bool,
    pub tweeters_on: bool,
    pub eq: bool,
    pub drc: bool,
}

/// Частичная конфигурация усилителей `SY6045S` (неуказанные поля не меняются).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AmpConfig {
    pub woofer: Option<u8>,
    pub tweeters: Option<u8>,
    pub aux: Option<u8>,
    pub woofer_on: Option<bool>,
    pub tweeters_on: Option<bool>,
    pub eq: Option<bool>,
    pub drc: Option<bool>,
}

/// Трейт-разъём для управления системной громкостью, усилителями, воспроизведением звука и медиа-сессиями.
pub trait AudioHal: Send + Sync {
    fn get_volume(&self) -> Result<u8>;
    fn set_volume(&mut self, percent: u8) -> Result<()>;
    fn is_muted(&self) -> Result<bool>;
    fn set_mute(&mut self, mute: bool) -> Result<()>;
    fn read_amp(&self) -> Result<AmpStatus>;
    fn configure_amp(&mut self, cfg: &AmpConfig) -> Result<()>;
    fn override_sound(&mut self, target_rel: &str, custom_path: &Path) -> Result<()>;
    fn restore_sounds(&mut self) -> Result<()>;
    fn play_tone(&self, freq_hz: f32, duration_ms: u32, volume: f32) -> Result<()>;
    fn play_wav(&mut self, path: &Path) -> Result<()>;
    fn stop(&mut self);
    fn track(&self) -> Result<MediaSessionInfo>;
    fn sessions(&self) -> Result<Vec<MediaSessionInfo>>;
}
