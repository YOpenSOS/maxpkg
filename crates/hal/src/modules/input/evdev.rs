//! Ядерный драйвер `evdev` для Яндекс Станции Макс (`/dev/input/event*` + `rotary-volume`).
//! Реализует Zero-Open / Zero-Alloc опрос через `poll(2)` и автоматический откат в `Drop`.

use super::{InputEvent, InputHal};
use crate::modules::arbiter::{ResourceKind, ResourceLease};
use crate::modules::device::DeviceProfile;
use anyhow::{bail, Result};
use std::collections::VecDeque;
use std::fs;

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_SW: u16 = 0x05;

const KEY_VOICECOMMAND: u16 = 582; // 0x246 — центральная кнопка Алисы на Станции Макс
const SW_MUTE_DEVICE: u16 = 14;    // 0x0e  — тумблер выключения микрофонов на Станции Макс
const SW_HEADPHONE_INSERT: u16 = 2;// 0x02  — тумблер разъёма AUX / наушников
const KEY_PAGEUP: u16 = 104;       // 0x68  — поворот шайбы громкости (rotary-volume)
const KEY_PAGEDOWN: u16 = 109;     // 0x6d  — поворот шайбы громкости (rotary-volume)
const KEY_VOLUMEUP: u16 = 115;
const KEY_VOLUMEDOWN: u16 = 114;

const ROTARY_STEP_SYSFS: &str = "/sys/devices/platform/rotary-volume/event_step";

#[cfg(unix)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeKind {
    Keypad,
    Rotary,
    Headphone,
}

#[cfg(unix)]
struct OpenNode {
    file: fs::File,
    kind: NodeKind,
}

pub struct EvdevDriver {
    lease: Option<ResourceLease>,
    #[cfg(unix)]
    nodes: Vec<OpenNode>,
    grabbed: bool,
    saved_wheel_step: Option<u8>,
    action_down_ms: Option<u64>,
    mute_start_ms: Option<u64>,
    pending: VecDeque<InputEvent>,
}

impl EvdevDriver {
    /// Открывает узлы `/dev/input/event*` один раз при инициализации по карте узлов `DeviceProfile`.
    pub fn open_for_device(dev: &DeviceProfile) -> Result<Self> {
        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::os::unix::fs::OpenOptionsExt;

            let mut nodes = Vec::new();
            let targets = [
                ("gpio_keypad", "/dev/input/event0", NodeKind::Keypad),
                ("rotary-volume", "/dev/input/event2", NodeKind::Rotary),
                (
                    "AML-AUGESOUND Headphones",
                    "/dev/input/event3",
                    NodeKind::Headphone,
                ),
            ];

            for (driver_name, fallback_path, kind) in targets {
                let path = dev
                    .kernel_nodes
                    .input_nodes
                    .get(driver_name)
                    .map(String::as_str)
                    .unwrap_or(fallback_path);

                if let Ok(file) = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(path)
                {
                    nodes.push(OpenNode { file, kind });
                }
            }

            Ok(Self {
                lease: None,
                nodes,
                grabbed: false,
                saved_wheel_step: None,
                action_down_ms: None,
                mute_start_ms: None,
                pending: VecDeque::with_capacity(16),
            })
        }

        #[cfg(not(unix))]
        {
            let _ = dev;
            Ok(Self {
                lease: None,
                grabbed: false,
                saved_wheel_step: None,
                action_down_ms: None,
                mute_start_ms: None,
                pending: VecDeque::new(),
            })
        }
    }

    #[cfg(unix)]
    fn query_switch_bit(&self, target_kind: NodeKind, sw_code: u16) -> Result<bool> {
        use std::os::unix::io::AsRawFd;
        const EVIOCGSW_8: libc::c_ulong = (2 << 30) | (8 << 16) | (0x45 << 8) | 0x1b;
        for node in &self.nodes {
            if node.kind == target_kind {
                let mut sw_bits = [0u8; 8];
                let rc = unsafe {
                    libc::ioctl(
                        node.file.as_raw_fd(),
                        EVIOCGSW_8 as _,
                        sw_bits.as_mut_ptr(),
                    )
                };
                if rc >= 0 {
                    let byte_idx = (sw_code / 8) as usize;
                    let bit_idx = sw_code % 8;
                    return Ok((sw_bits[byte_idx] & (1 << bit_idx)) != 0);
                }
            }
        }
        Ok(false)
    }

    pub fn decode_raw(
        &mut self,
        kind: u16,
        code: u16,
        value: i32,
        time_ms: u64,
    ) -> Option<InputEvent> {
        if kind == EV_SYN {
            return None;
        }
        match (kind, code) {
            (EV_KEY, KEY_VOICECOMMAND) => {
                let (action, held_ms) = match value {
                    1 => {
                        self.action_down_ms = Some(time_ms);
                        ("down", 0)
                    }
                    0 => {
                        let held = self
                            .action_down_ms
                            .take()
                            .map(|t0| time_ms.saturating_sub(t0))
                            .unwrap_or(0);
                        ("up", held)
                    }
                    _ => ("repeat", 0),
                };
                Some(InputEvent::new("action", action, 0, held_ms, code, value, time_ms))
            }
            (EV_SW, SW_MUTE_DEVICE) => {
                let (action, held_ms) = if value != 0 {
                    self.mute_start_ms = Some(time_ms);
                    ("muted", 0)
                } else {
                    let held = self
                        .mute_start_ms
                        .take()
                        .map(|t0| time_ms.saturating_sub(t0))
                        .unwrap_or(0);
                    ("unmuted", held)
                };
                Some(InputEvent::new("mute", action, 0, held_ms, code, value, time_ms))
            }
            (EV_KEY, KEY_PAGEUP | KEY_VOLUMEUP) if value == 1 => {
                Some(InputEvent::new("wheel", "rotate", 1, 0, code, value, time_ms))
            }
            (EV_KEY, KEY_PAGEDOWN | KEY_VOLUMEDOWN) if value == 1 => {
                Some(InputEvent::new("wheel", "rotate", -1, 0, code, value, time_ms))
            }
            (EV_SW, SW_HEADPHONE_INSERT) => {
                let act = if value != 0 { "inserted" } else { "removed" };
                Some(InputEvent::new("headphone", act, 0, 0, code, value, time_ms))
            }
            _ => None,
        }
    }
}

impl InputHal for EvdevDriver {
    fn is_mic_muted(&self) -> Result<bool> {
        #[cfg(unix)]
        {
            self.query_switch_bit(NodeKind::Keypad, SW_MUTE_DEVICE)
        }
        #[cfg(not(unix))]
        {
            Ok(false)
        }
    }

    fn is_aux_inserted(&self) -> Result<bool> {
        #[cfg(unix)]
        {
            self.query_switch_bit(NodeKind::Headphone, SW_HEADPHONE_INSERT)
        }
        #[cfg(not(unix))]
        {
            Ok(false)
        }
    }

    fn get_wheel_step(&self) -> Result<u8> {
        if let Ok(raw) = fs::read_to_string(ROTARY_STEP_SYSFS) {
            if let Ok(v) = raw.trim().parse::<u8>() {
                return Ok(v);
            }
        }
        Ok(1)
    }

    fn set_wheel_step(&mut self, step: u8) -> Result<()> {
        if !(1..=10).contains(&step) {
            bail!("шаг чувствительности крутилки должен быть от 1 до 10");
        }
        if self.lease.is_none() {
            self.lease = Some(ResourceLease::acquire(ResourceKind::Input)?);
        }
        if self.saved_wheel_step.is_none() {
            self.saved_wheel_step = self.get_wheel_step().ok();
        }
        if std::path::Path::new(ROTARY_STEP_SYSFS).exists() {
            fs::write(ROTARY_STEP_SYSFS, step.to_string())?;
        }
        Ok(())
    }

    fn set_grab(&mut self, grab: bool) -> Result<()> {
        if self.grabbed == grab {
            return Ok(());
        }
        if grab && self.lease.is_none() {
            self.lease = Some(ResourceLease::acquire(ResourceKind::Input)?);
        }
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            const EVIOCGRAB: libc::c_ulong = 0x4004_4590;
            let flag: libc::c_int = if grab { 1 } else { 0 };
            for node in &self.nodes {
                unsafe {
                    let _ = libc::ioctl(node.file.as_raw_fd(), EVIOCGRAB as _, flag);
                }
            }
        }
        self.grabbed = grab;
        Ok(())
    }

    fn next_event(&mut self, timeout_ms: u64) -> Result<Option<InputEvent>> {
        if let Some(ev) = self.pending.pop_front() {
            return Ok(Some(ev));
        }
        #[cfg(unix)]
        {
            use std::io::Read;
            use std::os::unix::io::AsRawFd;
            use std::time::{Duration, Instant};

            if self.nodes.is_empty() {
                return Ok(None);
            }

            let word = std::mem::size_of::<libc::c_long>();
            let ev_size = word * 2 + 8;
            let mut raw_buf = [0u8; 24 * 16];
            let deadline = Instant::now() + Duration::from_millis(timeout_ms);

            loop {
                if let Some(ev) = self.pending.pop_front() {
                    return Ok(Some(ev));
                }
                let rem = deadline.saturating_duration_since(Instant::now());
                if rem.is_zero() {
                    return Ok(None);
                }

                let mut poll_fds = [libc::pollfd {
                    fd: -1,
                    events: libc::POLLIN,
                    revents: 0,
                }; 4];
                let count = self.nodes.len().min(4);
                for (i, node) in self.nodes.iter().take(count).enumerate() {
                    poll_fds[i].fd = node.file.as_raw_fd();
                }

                let wait_ms = (rem.as_millis() as i32).min(200);
                let ready = unsafe { libc::poll(poll_fds.as_mut_ptr(), count as _, wait_ms) };
                if ready > 0 {
                    for i in 0..count {
                        if (poll_fds[i].revents & libc::POLLIN) != 0 {
                            if let Ok(n) = self.nodes[i].file.read(&mut raw_buf) {
                                for chunk in raw_buf[..n - (n % ev_size)].chunks_exact(ev_size) {
                                    let sec =
                                        u64::from_ne_bytes(chunk[0..8].try_into().unwrap_or([0; 8]));
                                    let usec = u64::from_ne_bytes(
                                        chunk[8..16].try_into().unwrap_or([0; 8]),
                                    );
                                    let kind =
                                        u16::from_ne_bytes([chunk[word * 2], chunk[word * 2 + 1]]);
                                    let code = u16::from_ne_bytes([
                                        chunk[word * 2 + 2],
                                        chunk[word * 2 + 3],
                                    ]);
                                    let value = i32::from_ne_bytes([
                                        chunk[word * 2 + 4],
                                        chunk[word * 2 + 5],
                                        chunk[word * 2 + 6],
                                        chunk[word * 2 + 7],
                                    ]);
                                    let time_ms =
                                        sec.saturating_mul(1000).saturating_add(usec / 1000);
                                    if let Some(decoded) =
                                        self.decode_raw(kind, code, value, time_ms)
                                    {
                                        self.pending.push_back(decoded);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = timeout_ms;
            Ok(None)
        }
    }
}

impl Drop for EvdevDriver {
    fn drop(&mut self) {
        let _ = self.set_grab(false);
        if let Some(orig) = self.saved_wheel_step.take() {
            if std::path::Path::new(ROTARY_STEP_SYSFS).exists() {
                let _ = fs::write(ROTARY_STEP_SYSFS, orig.to_string());
            }
        }
        self.lease = None;
    }
}
