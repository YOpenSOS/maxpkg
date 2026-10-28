//! Драйвер `AudioFlingerShmDriver`: один раз отображает (`mmap`) разделяемую память `AudioFlinger::Client`
//! процесса `com.yandex.io.sdk` и читает сырые `i16` сэмплы из 16 каналов (`48 000 Гц`) без копирований в ядре.

use super::{MicHal, MicInfo};
use anyhow::{bail, Result};
use std::fs::{self, File, OpenOptions};

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: usize = 16;
const RING_FRAMES: usize = 9216;
const CBLK_BUF_FRAMES_OFF: usize = 0xa8;
const CBLK_STEP_CNT_OFF: usize = 0x0c;
const CBLK_REAR_OFF: usize = 0xb4;
const CBLK_RING_OFF: usize = 0xe0;

/// Драйвер чтения 16-канального микрофонного буфера `AudioFlinger` через `MAP_SHARED`.
pub struct AudioFlingerShmDriver {
    #[allow(dead_code)]
    shm_file: Option<File>,
    map_ptr: *mut u8,
    map_size: usize,
    cblk_off: usize,
    ring_off: usize,
    pid: u32,
    event0_file: Option<File>,
}

// Безопасно: указатель `map_ptr` ссылается на read-only `MAP_SHARED` страницу разделяемой памяти.
unsafe impl Send for AudioFlingerShmDriver {}
unsafe impl Sync for AudioFlingerShmDriver {}

impl AudioFlingerShmDriver {
    pub fn open() -> Self {
        let event0_file = OpenOptions::new().read(true).open("/dev/input/event0").ok();
        let mut drv = Self {
            shm_file: None,
            map_ptr: std::ptr::null_mut(),
            map_size: 0,
            cblk_off: 0,
            ring_off: 0,
            pid: 0,
            event0_file,
        };
        drv.try_attach();
        drv
    }

    #[cfg(target_os = "android")]
    fn try_attach(&mut self) -> bool {
        use std::os::unix::io::AsRawFd;

        self.unmap();
        let Ok(entries) = fs::read_dir("/proc") else {
            return false;
        };

        for entry in entries.flatten() {
            let fname = entry.file_name();
            let Some(pid_str) = fname.to_str() else {
                continue;
            };
            let Ok(pid) = pid_str.parse::<u32>() else {
                continue;
            };
            if pid <= 1 {
                continue;
            }

            let cmd_path = format!("/proc/{pid}/cmdline");
            let Ok(cmd_bytes) = fs::read(&cmd_path) else {
                continue;
            };
            let cmd = String::from_utf8_lossy(&cmd_bytes);
            if !cmd.contains("com.yandex.io.sdk") {
                continue;
            }

            let maps_path = format!("/proc/{pid}/maps");
            let Ok(maps_text) = fs::read_to_string(&maps_path) else {
                continue;
            };

            for line in maps_text.lines() {
                if !line.contains("AudioFlinger::Client") {
                    continue;
                }
                let Some(range_str) = line.split_whitespace().next() else {
                    continue;
                };
                let Some((start_hex, end_hex)) = range_str.split_once('-') else {
                    continue;
                };
                let (Ok(start), Ok(end)) = (
                    u64::from_str_radix(start_hex, 16),
                    u64::from_str_radix(end_hex, 16),
                ) else {
                    continue;
                };
                if end <= start {
                    continue;
                }
                let sz = (end - start) as usize;
                let mf_path = format!("/proc/{pid}/map_files/{start_hex}-{end_hex}");
                let Ok(file) = OpenOptions::new().read(true).open(&mf_path) else {
                    continue;
                };

                let ptr = unsafe {
                    libc::mmap(
                        std::ptr::null_mut(),
                        sz,
                        libc::PROT_READ,
                        libc::MAP_SHARED,
                        file.as_raw_fd(),
                        0,
                    )
                };
                if ptr == libc::MAP_FAILED || ptr.is_null() {
                    continue;
                }
                let base = ptr as *mut u8;

                // Ищем управляющий заголовок audio_track_cblk_t с mBufferSizeInFrames == 9216 и stepCount == 3072
                let mut off = 0usize;
                while off + 0x60000 <= sz {
                    let buf_frames = unsafe {
                        std::ptr::read_volatile(base.add(off + CBLK_BUF_FRAMES_OFF) as *const u32)
                    };
                    let step_cnt = unsafe {
                        std::ptr::read_volatile(base.add(off + CBLK_STEP_CNT_OFF) as *const u32)
                    };
                    let r1 = unsafe {
                        std::ptr::read_volatile(base.add(off + CBLK_REAR_OFF) as *const u32)
                    };
                    if buf_frames == RING_FRAMES as u32 && step_cnt == 3072 && r1 != 0 {
                        self.shm_file = Some(file);
                        self.map_ptr = base;
                        self.map_size = sz;
                        self.cblk_off = off;
                        self.ring_off = off + CBLK_RING_OFF;
                        self.pid = pid;
                        return true;
                    }
                    off += 0x100;
                }

                unsafe {
                    libc::munmap(ptr, sz);
                }
            }
        }
        false
    }

    #[cfg(not(target_os = "android"))]
    fn try_attach(&mut self) -> bool {
        false
    }

    fn read_rear(&self) -> u32 {
        if !self.map_ptr.is_null() {
            unsafe {
                std::ptr::read_volatile(self.map_ptr.add(self.cblk_off + CBLK_REAR_OFF) as *const u32)
            }
        } else {
            0
        }
    }

    fn unmap(&mut self) {
        #[cfg(target_os = "android")]
        if !self.map_ptr.is_null() && self.map_size > 0 {
            unsafe {
                libc::munmap(self.map_ptr as *mut libc::c_void, self.map_size);
            }
        }
        self.map_ptr = std::ptr::null_mut();
        self.map_size = 0;
        self.shm_file = None;
        self.pid = 0;
    }
}

impl MicHal for AudioFlingerShmDriver {
    fn info(&mut self) -> Result<MicInfo> {
        if self.map_ptr.is_null() && !self.try_attach() {
            bail!("не найден сегмент разделяемой памяти AudioFlinger::Client");
        }
        Ok(MicInfo {
            rate: SAMPLE_RATE,
            channels: CHANNELS as u8,
            buffer_frames: RING_FRAMES as u32,
            pid: self.pid,
            rear: self.read_rear(),
        })
    }

    fn is_muted(&self) -> bool {
        #[cfg(target_os = "android")]
        if let Some(file) = self.event0_file.as_ref() {
            use std::os::unix::io::AsRawFd;
            const EVIOCGSW_8: libc::c_int = 0x8008_451bu32 as libc::c_int;
            const SW_MUTE_DEVICE: usize = 14;
            let mut bits = [0u8; 8];
            let rc = unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGSW_8, bits.as_mut_ptr()) };
            if rc >= 0 {
                return (bits[SW_MUTE_DEVICE / 8] & (1 << (SW_MUTE_DEVICE % 8))) != 0;
            }
        }
        false
    }

    fn read_channel(&mut self, frames: usize, channel: usize, out: &mut Vec<i16>) -> Result<()> {
        let count = frames.clamp(1, 4096);
        let ch = channel.min(CHANNELS - 1);
        out.clear();
        out.reserve(count);

        if self.map_ptr.is_null() && !self.try_attach() {
            bail!("не найден сегмент разделяемой памяти AudioFlinger::Client");
        }

        let rear = self.read_rear() as usize;
        let ring_base = unsafe { self.map_ptr.add(self.ring_off) as *const i16 };
        let start = rear.wrapping_sub(count);
        for i in 0..count {
            let frame_idx = start.wrapping_add(i) % RING_FRAMES;
            let sample = unsafe {
                std::ptr::read_volatile(ring_base.add(frame_idx * CHANNELS + ch))
            };
            out.push(sample);
        }
        Ok(())
    }

    fn read_peaks(&mut self, frames: usize) -> Result<[u16; 16]> {
        let count = frames.clamp(1, 4096);
        let mut peaks = [0u16; 16];

        if self.map_ptr.is_null() && !self.try_attach() {
            bail!("не найден сегмент разделяемой памяти AudioFlinger::Client");
        }

        let rear = self.read_rear() as usize;
        let ring_base = unsafe { self.map_ptr.add(self.ring_off) as *const i16 };
        let start = rear.wrapping_sub(count);
        for i in 0..count {
            let frame_idx = start.wrapping_add(i) % RING_FRAMES;
            let base_idx = frame_idx * CHANNELS;
            for (ch, pk) in peaks.iter_mut().enumerate() {
                let s = unsafe { std::ptr::read_volatile(ring_base.add(base_idx + ch)) };
                let abs_s = s.unsigned_abs();
                if abs_s > *pk {
                    *pk = abs_s;
                }
            }
        }
        Ok(peaks)
    }
}

impl Drop for AudioFlingerShmDriver {
    fn drop(&mut self) {
        self.unmap();
    }
}
