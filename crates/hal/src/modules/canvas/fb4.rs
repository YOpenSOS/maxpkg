//! Драйвер прямого `mmap` ядерного буфера `/dev/graphics/fb4` (`YNDX LED screen`, `25x16`, `24 bpp`, `1200` байт)
//! с межпроцессным арбитражем приоритетов (`Background < Normal < Overlay`) и `flock`-защитой.

use super::quasar_pause::set_quasar_clock_paused;
use super::CanvasHal;
use crate::modules::arbiter::{ResourceKind, ResourceLease};
use crate::modules::device::DeviceProfile;
use anyhow::{bail, Result};
use std::fs;

pub const MATRIX_W: usize = 25;
pub const MATRIX_H: usize = 16;
pub const MATRIX_PIXELS: usize = MATRIX_W * MATRIX_H; // 400 пикселей
pub const MATRIX_BYTES: usize = MATRIX_PIXELS * 3; // 1200 байт (RGB888)
const MMAP_LEN: usize = 4096;

const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const FBIOPAN_DISPLAY: libc::c_ulong = 0x4606;
const FB4_BACKLIGHT_PATH: &str = "/sys/class/backlight/fb4/brightness";

pub struct Fb4CanvasDriver {
    lease: Option<ResourceLease>,
    #[cfg(unix)]
    file: Option<fs::File>,
    #[cfg(unix)]
    mmap_ptr: *mut u8,
    #[cfg(unix)]
    vinfo: [u8; 160],
    #[cfg(unix)]
    vinfo_ok: bool,
    saved_frame: [u8; MATRIX_BYTES],
    saved_backlight: Option<u8>,
    pixels: [u8; MATRIX_PIXELS],
    cur_backlight: u8,
    active: bool,
}

unsafe impl Send for Fb4CanvasDriver {}
unsafe impl Sync for Fb4CanvasDriver {}

impl Fb4CanvasDriver {
    pub fn open_for_device(dev: &DeviceProfile) -> Result<Self> {
        let saved_backlight = fs::read_to_string(FB4_BACKLIGHT_PATH)
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok());
        let cur_backlight = saved_backlight.unwrap_or(170);

        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::os::unix::fs::FileExt;
            use std::os::unix::io::AsRawFd;

            let fb_path = dev
                .kernel_nodes
                .fb_nodes
                .get("YNDX LED screen")
                .map(String::as_str)
                .unwrap_or("/dev/graphics/fb4");

            let mut saved_frame = [0u8; MATRIX_BYTES];
            let mut vinfo = [0u8; 160];
            let mut vinfo_ok = false;
            let mut mmap_ptr = std::ptr::null_mut();

            let file = OpenOptions::new().read(true).write(true).open(fb_path).ok();
            if let Some(ref f) = file {
                let _ = f.read_at(&mut saved_frame, 0);
                let fd = f.as_raw_fd();
                if unsafe { libc::ioctl(fd, FBIOGET_VSCREENINFO as _, vinfo.as_mut_ptr()) } == 0 {
                    vinfo_ok = true;
                }
                let mapped = unsafe {
                    libc::mmap(
                        std::ptr::null_mut(),
                        MMAP_LEN,
                        libc::PROT_READ | libc::PROT_WRITE,
                        libc::MAP_SHARED,
                        fd,
                        0,
                    )
                };
                if mapped != libc::MAP_FAILED && !mapped.is_null() {
                    mmap_ptr = mapped.cast::<u8>();
                }
            }

            Ok(Self {
                lease: None,
                file,
                mmap_ptr,
                vinfo,
                vinfo_ok,
                saved_frame,
                saved_backlight,
                pixels: [0u8; MATRIX_PIXELS],
                cur_backlight,
                active: false,
            })
        }

        #[cfg(not(unix))]
        {
            let _ = dev;
            Ok(Self {
                lease: None,
                saved_frame: [0u8; MATRIX_BYTES],
                saved_backlight,
                pixels: [0u8; MATRIX_PIXELS],
                cur_backlight,
                active: false,
            })
        }
    }

    fn ensure_lease(&mut self) -> Result<()> {
        if self.lease.is_none() {
            let lease = ResourceLease::acquire(ResourceKind::Canvas)?;
            if let Some(shm) = lease.shm_mut() {
                if shm.orig_canvas_saved == 0 {
                    shm.orig_canvas_brightness = u32::from(self.saved_backlight.unwrap_or(170));
                    shm.orig_canvas_saved = 1;
                }
                shm.canvas_brightness[lease.priority as usize] = u32::from(self.cur_backlight);
            }
            self.lease = Some(lease);
        }
        Ok(())
    }

    #[cfg(unix)]
    fn write_raw_frame(&self, raw: &[u8; MATRIX_BYTES]) -> Result<()> {
        use std::os::unix::fs::FileExt;
        use std::os::unix::io::AsRawFd;

        if let Some(ref f) = self.file {
            if !self.mmap_ptr.is_null() {
                unsafe {
                    std::ptr::copy_nonoverlapping(raw.as_ptr(), self.mmap_ptr, MATRIX_BYTES);
                }
            } else {
                f.write_all_at(raw, 0)?;
            }
            if self.vinfo_ok {
                unsafe {
                    let _ = libc::ioctl(f.as_raw_fd(), FBIOPAN_DISPLAY as _, self.vinfo.as_ptr());
                }
            }
        }
        Ok(())
    }
}

impl CanvasHal for Fb4CanvasDriver {
    fn dimensions(&self) -> (usize, usize) {
        (MATRIX_W, MATRIX_H)
    }

    fn clear_buf(&mut self) {
        self.pixels = [0u8; MATRIX_PIXELS];
    }

    fn set_pixel(&mut self, x: usize, y: usize, br: u8) {
        if x < MATRIX_W && y < MATRIX_H {
            self.pixels[y * MATRIX_W + x] = br;
        }
    }

    fn set_frame(&mut self, values: &[u8], brightness: Option<u8>) -> Result<()> {
        if values.len() != MATRIX_PIXELS {
            bail!(
                "frame должен содержать ровно {} значений (25x16), получено: {}",
                MATRIX_PIXELS,
                values.len()
            );
        }
        let is_binary_mask = values.iter().all(|&v| v <= 1);
        let scale = brightness.unwrap_or(255);

        for (i, &val) in values.iter().enumerate() {
            self.pixels[i] = if is_binary_mask {
                if val != 0 { scale } else { 0 }
            } else if let Some(br) = brightness {
                ((u16::from(val) * u16::from(br)) / 255) as u8
            } else {
                val
            };
        }
        Ok(())
    }

    fn get_brightness(&self) -> Result<u8> {
        let s = fs::read_to_string(FB4_BACKLIGHT_PATH).unwrap_or_else(|_| "170".into());
        Ok(s.trim().parse::<u8>().unwrap_or(170))
    }

    fn set_brightness(&mut self, br: u8) -> Result<()> {
        self.ensure_lease()?;
        let clamped = br.min(200);
        self.cur_backlight = clamped;
        if let Some(ref lease) = self.lease {
            if let Some(shm) = lease.shm_mut() {
                shm.canvas_brightness[lease.priority as usize] = u32::from(clamped);
            }
            if lease.is_top_priority() && std::path::Path::new(FB4_BACKLIGHT_PATH).exists() {
                fs::write(FB4_BACKLIGHT_PATH, clamped.to_string())?;
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.ensure_lease()?;
        if !self.active {
            set_quasar_clock_paused(true);
            self.active = true;
        }

        let mut raw = [0u8; MATRIX_BYTES];
        for (i, &br) in self.pixels.iter().enumerate() {
            let base = i * 3;
            raw[base] = br;
            raw[base + 1] = br;
            raw[base + 2] = br;
        }

        if let Some(ref lease) = self.lease {
            if let Some(shm) = lease.shm_mut() {
                let p = lease.priority as usize;
                shm.canvas_frames[p] = raw;
                shm.canvas_has_frame[p] = 1;
            }
            // Если сейчас активен пакет с более высоким приоритетом (например, Overlay поверх Background),
            // мы сохранили кадр в теневой буфер SHM и НЕ перезаписываем физический fb4!
            if !lease.is_top_priority() {
                return Ok(());
            }
        }

        #[cfg(unix)]
        {
            self.write_raw_frame(&raw)?;
        }
        Ok(())
    }

    fn release(&mut self) -> Result<()> {
        if self.active {
            if let Some(ref lease) = self.lease {
                if let Some(other_prio) = lease.any_other_priority_active() {
                    // Передаём экран оставшемуся пакету (например, возврат с Overlay на Background)
                    if let Some(shm) = lease.shm_mut() {
                        let p = other_prio as usize;
                        if shm.canvas_has_frame[p] != 0 {
                            #[cfg(unix)]
                            {
                                let restored = shm.canvas_frames[p];
                                let _ = self.write_raw_frame(&restored);
                            }
                            let br = shm.canvas_brightness[p].min(200);
                            if std::path::Path::new(FB4_BACKLIGHT_PATH).exists() {
                                let _ = fs::write(FB4_BACKLIGHT_PATH, br.to_string());
                            }
                        }
                    }
                    self.active = false;
                    self.lease = None;
                    return Ok(());
                }
            }

            // Ни одного другого пакета на экране не осталось — возвращаем штатные часы Яндекса!
            #[cfg(unix)]
            {
                let _ = self.write_raw_frame(&self.saved_frame);
            }
            set_quasar_clock_paused(false);
            self.active = false;

            let orig_br = self
                .lease
                .as_ref()
                .and_then(|l| l.shm_mut())
                .filter(|s| s.orig_canvas_saved != 0)
                .map(|s| {
                    s.orig_canvas_saved = 0;
                    s.orig_canvas_brightness as u8
                })
                .or(self.saved_backlight);

            if let Some(br) = orig_br {
                if std::path::Path::new(FB4_BACKLIGHT_PATH).exists() {
                    let _ = fs::write(FB4_BACKLIGHT_PATH, br.to_string());
                }
            }
        }
        self.lease = None;
        Ok(())
    }
}

impl Drop for Fb4CanvasDriver {
    fn drop(&mut self) {
        let _ = self.release();
        #[cfg(unix)]
        if !self.mmap_ptr.is_null() {
            unsafe {
                let _ = libc::munmap(self.mmap_ptr.cast(), MMAP_LEN);
            }
            self.mmap_ptr = std::ptr::null_mut();
        }
    }
}
