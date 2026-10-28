//! Модуль `events`: единая ядерная Event-Driven шина событий (`EventBus`), объединяющая:
//! - физические кнопки, энкодер, тумблер Mute и разъём AUX (`/dev/input/event*`);
//! - питание Bluetooth-чипа `WCN3990` (`/dev/rfkill`, бинарный `struct rfkill_event`);
//! - обновление конфигурации Bluedroid (`/data/misc/bluedroid/bt_config.conf` через `inotify`);
//! - состояния ассистента Алисы (`ledpatterns/*.led` через `inotify IN_OPEN`);
//! - воспроизведение системных звуков и джинглов (`sounds/{common,ru-RU}/*` через `inotify IN_OPEN`);
//! - высокоточный периодический таймер ядра (`timerfd_create`);
//! - сигнал штатной остановки пакета `SIGTERM` / `SIGINT` (`EventKind::Stop`).

pub mod bus;

pub use bus::EventBus;

/// Тип события в единой шине `EventBus` (совместим с C-ABI `MaxEventKind`).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    None = 0,
    Stop = 1,
    Tick = 2,
    Input = 3,
    BtPower = 4,
    BtConfig = 5,
    AliceState = 6,
    SoundPlay = 7,
}

impl EventKind {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Stop,
            2 => Self::Tick,
            3 => Self::Input,
            4 => Self::BtPower,
            5 => Self::BtConfig,
            6 => Self::AliceState,
            7 => Self::SoundPlay,
            _ => Self::None,
        }
    }
}

/// Универсальная C-ABI структура события `MaxEvent` (`128` байт, фиксированный размер без аллокаций).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MaxEvent {
    /// Тип события (`EventKind as u32`).
    pub kind: u32,
    /// Числовой код события (код кнопки, `1`/`0` для питания/тумблера, код состояния Алисы).
    pub code: i32,
    /// Дельта вращения энкодера (`-1`/`+1`) или счётчик мульти-клика (`1`, `2`, `4`).
    pub delta: i32,
    /// Длительность удержания кнопки (мс) или порядковый номер тика `timerfd`.
    pub held_ms: u64,
    /// Монотонная метка времени события (мс).
    pub timestamp_ms: u64,
    /// Источник события (`"action"`, `"wheel"`, `"mute"`, `"headphone"`, `"rfkill"`, `"quasar"`, `"timer"`).
    pub source: [u8; 32],
    /// Имя действия или файла (`"down"`, `"up"`, `"quadro_click"`, `"bluetooth_turning_on_jingle.opus"`, `"listening"`).
    pub name: [u8; 64],
}

impl Default for MaxEvent {
    fn default() -> Self {
        Self {
            kind: EventKind::None as u32,
            code: 0,
            delta: 0,
            held_ms: 0,
            timestamp_ms: 0,
            source: [0u8; 32],
            name: [0u8; 64],
        }
    }
}

impl MaxEvent {
    pub fn new(
        kind: EventKind,
        source: &str,
        name: &str,
        code: i32,
        delta: i32,
        held_ms: u64,
        timestamp_ms: u64,
    ) -> Self {
        let mut ev = Self {
            kind: kind as u32,
            code,
            delta,
            held_ms,
            timestamp_ms,
            source: [0u8; 32],
            name: [0u8; 64],
        };
        copy_cstr(&mut ev.source, source);
        copy_cstr(&mut ev.name, name);
        ev
    }

    pub fn kind_enum(&self) -> EventKind {
        EventKind::from_u32(self.kind)
    }

    pub fn source_str(&self) -> &str {
        cstr_to_str(&self.source)
    }

    pub fn name_str(&self) -> &str {
        cstr_to_str(&self.name)
    }
}

fn copy_cstr(dst: &mut [u8], s: &str) {
    dst.fill(0);
    let bytes = s.as_bytes();
    let n = bytes.len().min(dst.len().saturating_sub(1));
    dst[..n].copy_from_slice(&bytes[..n]);
}

fn cstr_to_str(buf: &[u8]) -> &str {
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    std::str::from_utf8(&buf[..len]).unwrap_or("")
}
