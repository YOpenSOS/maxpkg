//! Драйвер ядерного виртуального пульта `/dev/uinput` (`max-virtual-remote`).
//! При закрытии дескриптора автоматически вызывает `UI_DEV_DESTROY` в `Drop`.

use super::RemoteHal;
use anyhow::{bail, Result};

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const SYN_REPORT: u16 = 0x00;
const BUS_VIRTUAL: u16 = 0x06;

const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564;
const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565;
const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;

/// Таблица поддерживаемых клавиш пульта и их ядерных кодов Linux (`Generic.kl` / `AVRCP.kl`).
const REMOTE_KEYS: &[(&str, u16)] = &[
    ("up", 103),         // KEY_UP
    ("down", 108),       // KEY_DOWN
    ("left", 105),       // KEY_LEFT
    ("right", 106),      // KEY_RIGHT
    ("ok", 353),         // KEY_SELECT (DPAD_CENTER)
    ("enter", 28),       // KEY_ENTER
    ("back", 158),       // KEY_BACK
    ("home", 172),       // KEY_HOMEPAGE
    ("menu", 139),       // KEY_MENU
    ("vol_up", 115),     // KEY_VOLUMEUP
    ("vol_down", 114),   // KEY_VOLUMEDOWN
    ("mute", 113),       // KEY_MUTE
    ("play_pause", 164), // KEY_PLAYPAUSE
    ("prev", 165),       // KEY_PREVIOUSSONG
    ("next", 163),       // KEY_NEXTSONG
    ("power", 116),      // KEY_POWER
    ("wakeup", 143),     // KEY_WAKEUP
    ("search", 217),     // KEY_SEARCH
    ("action", 582),     // KEY_VOICECOMMAND (центральная кнопка Алисы)
];

pub fn resolve_key_code(name: &str) -> Result<u16> {
    let norm = name.trim().to_ascii_lowercase();
    for &(k, code) in REMOTE_KEYS {
        if k == norm {
            return Ok(code);
        }
    }
    bail!(
        "неизвестная клавиша пульта {:?} (доступны: next, prev, play_pause, vol_up, vol_down, mute, up, down, left, right, ok, enter, back, home, menu, power, wakeup, search, action)",
        name
    )
}

#[repr(C)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
struct UinputUserDev {
    name: [u8; 80],
    id: InputId,
    ff_effects_max: u32,
    absmax: [i32; 64],
    absmin: [i32; 64],
    absfuzz: [i32; 64],
    absflat: [i32; 64],
}

/// Драйвер виртуального устройства ввода `/dev/uinput`.
pub struct UinputRemote {
    #[cfg(unix)]
    file: Option<std::fs::File>,
}

impl UinputRemote {
    pub fn new() -> Self {
        Self {
            #[cfg(unix)]
            file: None,
        }
    }

    pub fn open() -> Result<Self> {
        let mut s = Self::new();
        #[cfg(unix)]
        {
            let _ = s.ensure_open()?;
        }
        Ok(s)
    }

    #[cfg(unix)]
    fn ensure_open(&mut self) -> Result<&mut std::fs::File> {
        if self.file.is_none() {
            use std::fs::OpenOptions;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            use std::os::unix::io::AsRawFd;

            let mut file = OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open("/dev/uinput")?;
            let fd = file.as_raw_fd();

            unsafe {
                libc::ioctl(fd, UI_SET_EVBIT as _, libc::c_int::from(EV_KEY));
                libc::ioctl(fd, UI_SET_EVBIT as _, libc::c_int::from(EV_SYN));
                for &(_, code) in REMOTE_KEYS {
                    libc::ioctl(fd, UI_SET_KEYBIT as _, libc::c_int::from(code));
                }
            }

            let mut uidev: UinputUserDev = unsafe { std::mem::zeroed() };
            let dev_name = b"max-virtual-remote";
            uidev.name[..dev_name.len()].copy_from_slice(dev_name);
            uidev.id.bustype = BUS_VIRTUAL;
            uidev.id.vendor = 0x1b8e;
            uidev.id.product = 0x0cec;
            uidev.id.version = 1;

            let bytes: &[u8] = unsafe {
                std::slice::from_raw_parts(
                    (&uidev as *const UinputUserDev).cast::<u8>(),
                    std::mem::size_of::<UinputUserDev>(),
                )
            };
            file.write_all(bytes)?;
            let rc = unsafe { libc::ioctl(fd, UI_DEV_CREATE as _) };
            if rc < 0 {
                bail!("не удалось создать устройство /dev/uinput (UI_DEV_CREATE)");
            }
            // Пауза 40 мс для регистрации нового виртуального пульта в EventHub Android
            std::thread::sleep(std::time::Duration::from_millis(40));
            self.file = Some(file);
        }
        Ok(self.file.as_mut().unwrap())
    }
}

impl Default for UinputRemote {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteHal for UinputRemote {
    fn send_key(&mut self, key_name: &str) -> Result<()> {
        let code = resolve_key_code(key_name)?;
        #[cfg(unix)]
        {
            use std::io::Write;
            let file = self.ensure_open()?;
            // 4 пакета struct input_event по 24 байта (на 64-бит): key down -> syn -> key up -> syn
            let word = std::mem::size_of::<libc::c_long>();
            let ev_size = word * 2 + 8;
            let mut packet = [0u8; 24 * 4];

            let events: [(u16, u16, i32); 4] = [
                (EV_KEY, code, 1),
                (EV_SYN, SYN_REPORT, 0),
                (EV_KEY, code, 0),
                (EV_SYN, SYN_REPORT, 0),
            ];
            for (i, (kind, c, val)) in events.iter().enumerate() {
                let base = i * ev_size + word * 2;
                packet[base..base + 2].copy_from_slice(&kind.to_ne_bytes());
                packet[base + 2..base + 4].copy_from_slice(&c.to_ne_bytes());
                packet[base + 4..base + 8].copy_from_slice(&val.to_ne_bytes());
            }
            file.write_all(&packet[..ev_size * 4])?;
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let _ = code;
            bail!("устройство /dev/uinput доступно только на Linux/Android")
        }
    }
}

impl Drop for UinputRemote {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(ref file) = self.file {
            use std::os::unix::io::AsRawFd;
            unsafe {
                let _ = libc::ioctl(file.as_raw_fd(), UI_DEV_DESTROY as _);
            }
        }
    }
}
