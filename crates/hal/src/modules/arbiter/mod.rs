//! Модуль `arbiter`: межпроцессный арбитр аппаратных ресурсов на базе ядерных `flock`
//! и разделяемой памяти `MAP_SHARED` (`arbiter.shm`) с поддержкой 3 уровней приоритета
//! (`Background < Normal < Overlay`), теневых буферов и автоматического восстановления после `kill -9`.

pub mod lease;
pub mod shm;

pub use lease::ResourceLease;
use serde::{Deserialize, Serialize};

/// Уровень приоритета пакета при доступе к экрану, кольцу, кнопкам и усилителям.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(usize)]
pub enum Priority {
    Background = 0,
    Normal = 1,
    Overlay = 2,
}

impl Priority {
    pub fn from_str_name(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "background" | "bg" | "low" | "0" => Self::Background,
            "overlay" | "notification" | "high" | "2" => Self::Overlay,
            _ => Self::Normal,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Background,
            2 => Self::Overlay,
            _ => Self::Normal,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Normal => "normal",
            Self::Overlay => "overlay",
        }
    }
}

/// Эксклюзивные аппаратные ресурсы колонки, защищаемые арбитром.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum ResourceKind {
    Canvas = 0,
    Ring = 1,
    Input = 2,
    Amp = 3,
}

impl ResourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Canvas => "canvas",
            Self::Ring => "ring",
            Self::Input => "input",
            Self::Amp => "amp",
        }
    }
}

/// Описание одного слота приоритета внутри `arbiter.shm`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlotEntry {
    pub pid: u32,
    pub active: u8,
    pub _pad: [u8; 3],
    pub pkg_id: [u8; 48],
}

/// Структура разделяемой памяти `arbiter.shm` (`MAP_SHARED`).
#[repr(C)]
pub struct ArbiterShm {
    pub magic: u32,
    pub version: u32,
    /// Слоты владельцев: `[ResourceKind (4)][Priority (3)]`
    pub slots: [[SlotEntry; 3]; 4],
    /// Теневые кадры экрана `25x16` (`1200` байт) для каждого из 3 приоритетов
    pub canvas_frames: [[u8; 1200]; 3],
    pub canvas_brightness: [u32; 3],
    pub canvas_has_frame: [u8; 3],
    pub _pad1: u8,
    /// Теневые кадры кольца `24 LED` (`72` байта) для каждого из 3 приоритетов
    pub ring_frames: [[u8; 72]; 3],
    pub ring_has_frame: [u8; 3],
    pub _pad2: u8,
    /// Журнал исходного состояния железа (для восстановления даже после `kill -9`)
    pub orig_canvas_saved: u8,
    pub orig_ring_saved: u8,
    pub _pad3: [u8; 2],
    pub orig_canvas_brightness: u32,
    pub orig_ring_frame: [u8; 72],
}

pub const ARBITER_MAGIC: u32 = 0x4D41_5841; // "MAXA"
pub const ARBITER_VERSION: u32 = 1;
