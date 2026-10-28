//! Ядерный мультиплексор `EventBus` на базе `poll(2)`, `/dev/input/event*`, `/dev/rfkill`,
//! `inotify` (`IN_OPEN | IN_CLOSE_WRITE`) и `timerfd_create`.

use super::{EventKind, MaxEvent};
use crate::modules::arbiter::lease::is_stop_requested;
use crate::modules::device::DeviceProfile;
use crate::modules::input::{EvdevDriver, InputHal};
use anyhow::Result;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const QUASAR_SOUNDS_COMMON: &str = "/data/data/com.yandex.io.sdk/files/system/sounds/common";
const QUASAR_SOUNDS_RURU: &str = "/data/data/com.yandex.io.sdk/files/system/sounds/ru-RU";
const QUASAR_LEDPATTERNS: &str = "/data/data/com.yandex.io.sdk/files/system/ledpatterns";
const BLUEDROID_DIR: &str = "/data/misc/bluedroid";

const RFKILL_TYPE_BLUETOOTH: u8 = 2;
const RFKILL_OP_CHANGE: u8 = 2;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct RfkillEvent {
    idx: u32,
    type_: u8,
    op: u8,
    soft: u8,
    hard: u8,
}

/// Единая шина событий ядра Linux для нативных пакетов.
pub struct EventBus {
    evdev: EvdevDriver,
    #[cfg(unix)]
    inotify_fd: libc::c_int,
    #[cfg(unix)]
    wd_sounds_common: libc::c_int,
    #[cfg(unix)]
    wd_sounds_ruru: libc::c_int,
    #[cfg(unix)]
    wd_ledpatterns: libc::c_int,
    #[cfg(unix)]
    wd_bluedroid: libc::c_int,
    #[cfg(unix)]
    rfkill_file: Option<File>,
    #[cfg(unix)]
    timer_fd: libc::c_int,
    tick_seq: u64,
    last_bt_soft: Option<u8>,
    last_sound_name: String,
    last_sound_instant: Option<Instant>,
    click_count: u8,
    last_click_instant: Option<Instant>,
    queue: VecDeque<MaxEvent>,
    stop_emitted: bool,
}

impl EventBus {
    /// Открывает шину событий и подписывается на ядерные дескрипторы `evdev`, `rfkill`, `inotify` и `timerfd`.
    pub fn open(dev: &DeviceProfile) -> Result<Self> {
        let evdev = EvdevDriver::open_for_device(dev)?;

        #[cfg(unix)]
        {
            use std::ffi::CString;
            use std::io::Read;
            use std::os::unix::fs::OpenOptionsExt;

            let inotify_fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
            let add_watch = |path: &str, mask: u32| -> libc::c_int {
                if inotify_fd >= 0 && Path::new(path).exists() {
                    if let Ok(c) = CString::new(path) {
                        return unsafe { libc::inotify_add_watch(inotify_fd, c.as_ptr(), mask) };
                    }
                }
                -1
            };

            let sound_mask =
                libc::IN_OPEN | libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_DELETE;
            let wd_sounds_common = add_watch(QUASAR_SOUNDS_COMMON, sound_mask);
            let wd_sounds_ruru = add_watch(QUASAR_SOUNDS_RURU, sound_mask);
            let wd_ledpatterns = add_watch(QUASAR_LEDPATTERNS, libc::IN_OPEN);
            let wd_bluedroid = add_watch(
                BLUEDROID_DIR,
                libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_ATTRIB,
            );

            let mut rfkill_file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open("/dev/rfkill")
                .ok();

            let mut last_bt_soft = None;
            // Считываем стартовый снапшот RFKILL_OP_ADD при открытии /dev/rfkill
            if let Some(ref mut rf) = rfkill_file {
                let mut init_buf = [0u8; 128];
                while let Ok(n) = rf.read(&mut init_buf) {
                    if n < std::mem::size_of::<RfkillEvent>() {
                        break;
                    }
                    for chunk in init_buf[..n].chunks_exact(std::mem::size_of::<RfkillEvent>()) {
                        let ev: RfkillEvent =
                            unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast()) };
                        if ev.type_ == RFKILL_TYPE_BLUETOOTH {
                            last_bt_soft = Some(ev.soft);
                        }
                    }
                }
            }

            let timer_fd = unsafe {
                libc::timerfd_create(
                    libc::CLOCK_MONOTONIC,
                    libc::TFD_NONBLOCK | libc::TFD_CLOEXEC,
                )
            };

            Ok(Self {
                evdev,
                inotify_fd,
                wd_sounds_common,
                wd_sounds_ruru,
                wd_ledpatterns,
                wd_bluedroid,
                rfkill_file,
                timer_fd,
                tick_seq: 0,
                last_bt_soft,
                last_sound_name: String::new(),
                last_sound_instant: None,
                click_count: 0,
                last_click_instant: None,
                queue: VecDeque::with_capacity(32),
                stop_emitted: false,
            })
        }

        #[cfg(not(unix))]
        {
            Ok(Self {
                evdev,
                tick_seq: 0,
                last_bt_soft: None,
                last_sound_name: String::new(),
                last_sound_instant: None,
                click_count: 0,
                last_click_instant: None,
                queue: VecDeque::new(),
                stop_emitted: false,
            })
        }
    }

    /// Настраивает период ядерного таймера `timerfd` в миллисекундах (`0` — выключить таймер).
    pub fn set_tick_ms(&mut self, period_ms: u32) -> Result<()> {
        #[cfg(unix)]
        if self.timer_fd >= 0 {
            let sec = (period_ms / 1000) as libc::time_t;
            let nsec = ((period_ms % 1000) as libc::c_long) * 1_000_000;
            let spec = libc::itimerspec {
                it_interval: libc::timespec {
                    tv_sec: sec,
                    tv_nsec: nsec,
                },
                it_value: libc::timespec {
                    tv_sec: sec,
                    tv_nsec: nsec,
                },
            };
            unsafe {
                libc::timerfd_settime(self.timer_fd, 0, &spec, std::ptr::null_mut());
            }
        }
        #[cfg(not(unix))]
        let _ = period_ms;
        Ok(())
    }

    /// Включает или выключает эксклюзивный перехват физических кнопок (`EVIOCGRAB`).
    pub fn set_input_grab(&mut self, grab: bool) -> Result<()> {
        self.evdev.set_grab(grab)
    }

    /// Ожидает следующее событие до `timeout_ms` мс (`< 0` — бесконечное ожидание до события или `SIGTERM`).
    pub fn next_event(&mut self, timeout_ms: i32) -> Result<Option<MaxEvent>> {
        if is_stop_requested() && !self.stop_emitted {
            self.stop_emitted = true;
            return Ok(Some(MaxEvent::new(
                EventKind::Stop,
                "signal",
                "stop",
                0,
                0,
                0,
                now_ms(),
            )));
        }
        if let Some(ev) = self.queue.pop_front() {
            return Ok(Some(ev));
        }

        let start = Instant::now();
        loop {
            if is_stop_requested() && !self.stop_emitted {
                self.stop_emitted = true;
                return Ok(Some(MaxEvent::new(
                    EventKind::Stop,
                    "signal",
                    "stop",
                    0,
                    0,
                    0,
                    now_ms(),
                )));
            }

            // 1. Проверяем неблокирующую очередь evdev
            self.poll_evdev_nonblocking()?;
            if let Some(ev) = self.queue.pop_front() {
                return Ok(Some(ev));
            }

            // 2. Опрашиваем ядерные дескрипторы inotify, rfkill и timerfd
            let slice_ms = if timeout_ms < 0 {
                25
            } else {
                let elapsed = start.elapsed().as_millis() as i32;
                let rem = timeout_ms - elapsed;
                if rem <= 0 {
                    return Ok(None);
                }
                rem.min(25)
            };

            #[cfg(unix)]
            self.poll_kernel_fds(slice_ms);
            #[cfg(not(unix))]
            std::thread::sleep(std::time::Duration::from_millis(slice_ms as u64));

            if let Some(ev) = self.queue.pop_front() {
                return Ok(Some(ev));
            }
            if timeout_ms == 0 {
                return Ok(None);
            }
        }
    }

    fn poll_evdev_nonblocking(&mut self) -> Result<()> {
        while let Some(inp) = self.evdev.next_event(0)? {
            let src = inp.source.as_str();
            let act = inp.action.as_str();
            self.queue.push_back(MaxEvent::new(
                EventKind::Input,
                src,
                act,
                inp.code as i32,
                inp.delta,
                inp.held_ms,
                inp.time_ms,
            ));

            // Распознавание мульти-кликов (DoubleClick / QuadroClick) и LongPress на кнопке Алисы
            if src == "action" && act == "up" {
                if inp.held_ms >= 800 {
                    self.click_count = 0;
                    self.queue.push_back(MaxEvent::new(
                        EventKind::Input,
                        "action",
                        "long_press",
                        inp.code as i32,
                        1,
                        inp.held_ms,
                        inp.time_ms,
                    ));
                } else {
                    let within_window = self
                        .last_click_instant
                        .map(|t| t.elapsed().as_millis() <= 450)
                        .unwrap_or(false);
                    self.click_count = if within_window {
                        self.click_count.saturating_add(1)
                    } else {
                        1
                    };
                    self.last_click_instant = Some(Instant::now());

                    if self.click_count == 2 {
                        self.queue.push_back(MaxEvent::new(
                            EventKind::Input,
                            "action",
                            "double_click",
                            inp.code as i32,
                            2,
                            0,
                            inp.time_ms,
                        ));
                    } else if self.click_count == 4 {
                        self.click_count = 0;
                        self.queue.push_back(MaxEvent::new(
                            EventKind::Input,
                            "action",
                            "quadro_click",
                            inp.code as i32,
                            4,
                            0,
                            inp.time_ms,
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    fn poll_kernel_fds(&mut self, wait_ms: i32) {
        use std::io::Read;
        use std::os::unix::io::AsRawFd;

        let mut fds = [
            libc::pollfd {
                fd: self.inotify_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: self.rfkill_file.as_ref().map(|f| f.as_raw_fd()).unwrap_or(-1),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: self.timer_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, wait_ms) };
        if ready <= 0 {
            return;
        }

        // 1. Обработка timerfd
        if (fds[2].revents & libc::POLLIN) != 0 && self.timer_fd >= 0 {
            let mut expirations = 0u64;
            let rc = unsafe {
                libc::read(
                    self.timer_fd,
                    (&mut expirations as *mut u64).cast(),
                    std::mem::size_of::<u64>(),
                )
            };
            if rc == std::mem::size_of::<u64>() as isize && expirations > 0 {
                self.tick_seq = self.tick_seq.wrapping_add(expirations);
                self.queue.push_back(MaxEvent::new(
                    EventKind::Tick,
                    "timer",
                    "tick",
                    1,
                    expirations as i32,
                    self.tick_seq,
                    now_ms(),
                ));
            }
        }

        // 2. Обработка /dev/rfkill (питание Bluetooth WCN3990)
        if (fds[1].revents & libc::POLLIN) != 0 {
            if let Some(ref mut rf) = self.rfkill_file {
                let mut buf = [0u8; 64];
                while let Ok(n) = rf.read(&mut buf) {
                    if n < std::mem::size_of::<RfkillEvent>() {
                        break;
                    }
                    for chunk in buf[..n].chunks_exact(std::mem::size_of::<RfkillEvent>()) {
                        let ev: RfkillEvent =
                            unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast()) };
                        if ev.type_ == RFKILL_TYPE_BLUETOOTH
                            && (ev.op == RFKILL_OP_CHANGE || self.last_bt_soft != Some(ev.soft))
                        {
                            self.last_bt_soft = Some(ev.soft);
                            let enabled = ev.soft == 0 && ev.hard == 0;
                            self.queue.push_back(MaxEvent::new(
                                EventKind::BtPower,
                                "rfkill",
                                if enabled { "enabled" } else { "disabled" },
                                i32::from(enabled),
                                0,
                                0,
                                now_ms(),
                            ));
                        }
                    }
                }
            }
        }

        // 3. Обработка inotify (звуки Quasar, анимации состояния Алисы, конфиг Bluedroid)
        if (fds[0].revents & libc::POLLIN) != 0 && self.inotify_fd >= 0 {
            let mut buf = [0u8; 2048];
            loop {
                let n = unsafe {
                    libc::read(
                        self.inotify_fd,
                        buf.as_mut_ptr().cast(),
                        buf.len(),
                    )
                };
                if n <= 0 {
                    break;
                }
                let n_usize = n as usize;
                let hdr_size = std::mem::size_of::<libc::inotify_event>();
                let mut offset = 0usize;

                while offset + hdr_size <= n_usize {
                    let ev: libc::inotify_event =
                        unsafe { std::ptr::read_unaligned(buf[offset..].as_ptr().cast()) };
                    let name_start = offset + hdr_size;
                    let name_end = (name_start + ev.len as usize).min(n_usize);
                    let fname = extract_inotify_name(&buf[name_start..name_end]);
                    self.handle_inotify_event(ev.wd, ev.mask, fname);
                    offset = name_end;
                }
            }
        }
    }

    #[cfg(unix)]
    fn handle_inotify_event(&mut self, wd: libc::c_int, mask: u32, fname: &str) {
        if fname.is_empty() {
            return;
        }
        let ts = now_ms();

        if wd == self.wd_bluedroid && fname == "bt_config.conf" {
            self.queue.push_back(MaxEvent::new(
                EventKind::BtConfig,
                "bluedroid",
                "bt_config.conf",
                1,
                0,
                0,
                ts,
            ));
            return;
        }

        if wd == self.wd_ledpatterns && (mask & libc::IN_OPEN) != 0 {
            let (state_name, state_code) = match fname {
                f if f.contains("listening") => ("listening", 1),
                f if f.contains("thinking") => ("thinking", 2),
                f if f.contains("speaking") => ("speaking", 3),
                f if f.contains("mute_mics") => ("muted", 4),
                f if f.contains("bluetooth") => ("bluetooth", 5),
                other => (other, 0),
            };
            self.queue.push_back(MaxEvent::new(
                EventKind::AliceState,
                "quasar",
                state_name,
                state_code,
                0,
                0,
                ts,
            ));
            return;
        }

        if (wd == self.wd_sounds_common || wd == self.wd_sounds_ruru)
            && (mask & libc::IN_OPEN) != 0
        {
            // Дедупликация двойного open() со стороны GStreamer в пределах 60 мс
            let is_dup = self.last_sound_name == fname
                && self
                    .last_sound_instant
                    .map(|t| t.elapsed().as_millis() < 60)
                    .unwrap_or(false);
            if !is_dup {
                self.last_sound_name = fname.to_string();
                self.last_sound_instant = Some(Instant::now());
                let folder = if wd == self.wd_sounds_common {
                    "common"
                } else {
                    "ru-RU"
                };
                self.queue.push_back(MaxEvent::new(
                    EventKind::SoundPlay,
                    folder,
                    fname,
                    1,
                    0,
                    0,
                    ts,
                ));
            }
        }
    }
}

#[cfg(unix)]
impl Drop for EventBus {
    fn drop(&mut self) {
        unsafe {
            if self.timer_fd >= 0 {
                libc::close(self.timer_fd);
            }
            if self.inotify_fd >= 0 {
                libc::close(self.inotify_fd);
            }
        }
    }
}

#[cfg(unix)]
fn extract_inotify_name(slice: &[u8]) -> &str {
    let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    std::str::from_utf8(&slice[..len]).unwrap_or("")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
