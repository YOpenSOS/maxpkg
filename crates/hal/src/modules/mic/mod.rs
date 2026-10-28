//! Модуль `mic`: прямой `Zero-Copy` доступ к 16-канальному кольцевому буферу `AudioFlinger::Client`
//! (`48 000 Гц`, `16-bit PCM`, `9216` фреймов: 7 физических микрофонов + VQE beamformer + стерео loopback).

pub mod shm;

use anyhow::Result;
use serde::{Deserialize, Serialize};
pub use shm::AudioFlingerShmDriver;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MicInfo {
    pub rate: u32,
    pub channels: u8,
    pub buffer_frames: u32,
    pub pid: u32,
    pub rear: u32,
}

/// Низкоуровневый трейт-разъём для чтения 16-канального микрофонного буфера.
pub trait MicHal: Send + Sync {
    fn info(&mut self) -> Result<MicInfo>;
    fn is_muted(&self) -> bool;
    fn read_channel(&mut self, frames: usize, channel: usize, out: &mut Vec<i16>) -> Result<()>;
    fn read_peaks(&mut self, frames: usize) -> Result<[u16; 16]>;
}
