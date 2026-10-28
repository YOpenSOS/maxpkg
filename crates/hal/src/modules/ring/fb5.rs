//! Драйвер прямого `mmap` ядерного буфера `/dev/graphics/fb5` (`YNDX LED ring`, `24x1`, `24 bpp`, `72` байта)
//! с межпроцессным арбитражем приоритетов (`Background < Normal < Overlay`) и `flock`-защитой.

use super::RingHal;
use crate::modules::arbiter::{ResourceKind, ResourceLease};
use crate::modules::device::DeviceProfile;
use anyhow::{bail, Result};

pub const RING_LEDS: usize = 24;
pub const RING_BYTES: usize = RING_LEDS * 3; // 72 байта (RGB888)
const MMAP_LEN: usize = 4096;

const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const FBIOPAN_DISPLAY: libc::c_ulong = 0x4606;

pub struct Fb5RingDriver {
    lease: Option<ResourceLease>,
    #[cfg(unix)]
    file: Option<std::fs::File>,
    #[cfg(unix)]
    mmap_ptr: *mut u8,
    #[cfg(unix)]
    vinfo: [u8; 160],
    #[cfg(unix)]
    vinfo_ok: bool,
    saved_frame: [u8; RING_BYTES],
    buf: [u8; RING_BYTES],
    dirty: bool,
    led_count: usize,
}

unsafe impl Send for Fb5RingDriver {}
unsafe impl Sync for Fb5RingDriver {}

impl Fb5RingDriver {
    pub fn open_for_device(dev: &DeviceProfile) -> Result<Self> {
        let led_count = (dev.hw.led_count as usize).clamp(1, RING_LEDS);

        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::os::unix::fs::FileExt;
            use std::os::unix::io::AsRawFd;

            let fb_path = dev
                .kernel_nodes
                .fb_nodes
                .get("YNDX LED ring")
                .map(String::as_str)
                .unwrap_or("/dev/graphics/fb5");

            let mut saved_frame = [0u8; RING_BYTES];
            let mut vinfo = [0u8; 160];
            let mut vinfo_ok = false;
            let mut mmap_ptr = std::ptr::null_mut();

            let file = OpenOptions::new().read(true).write(true).open(fb_path).ok();
            if let Some(ref f) = file {
                let _ = f.read_at(&mut saved_frame, 0);
                let fd = f.as_raw_fd();
                let rc = unsafe { libc::ioctl(fd, FBIOGET_VSCREENINFO as _, vinfo.as_mut_ptr()) };
                if rc == 0 {
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
                buf: saved_frame,
                dirty: false,
                led_count,
            })
        }

        #[cfg(not(unix))]
        {
            Ok(Self {
                lease: None,
                saved_frame: [0u8; RING_BYTES],
                buf: [0u8; RING_BYTES],
                dirty: false,
                led_count,
            })
        }
    }

    fn ensure_lease(&mut self) -> Result<()> {
        if self.lease.is_none() {
            let lease = ResourceLease::acquire(ResourceKind::Ring)?;
            if let Some(shm) = lease.shm_mut() {
                if shm.orig_ring_saved == 0 {
                    shm.orig_ring_frame = self.saved_frame;
                    shm.orig_ring_saved = 1;
                }
            }
            self.lease = Some(lease);
        }
        Ok(())
    }

    #[cfg(unix)]
    fn write_hardware(&self, frame: &[u8; RING_BYTES]) -> Result<()> {
        use std::os::unix::fs::FileExt;
        use std::os::unix::io::AsRawFd;

        if let Some(ref f) = self.file {
            if !self.mmap_ptr.is_null() {
                unsafe {
                    std::ptr::copy_nonoverlapping(frame.as_ptr(), self.mmap_ptr, RING_BYTES);
                }
            } else {
                f.write_all_at(frame, 0)?;
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

impl RingHal for Fb5RingDriver {
    fn led_count(&self) -> usize {
        self.led_count
    }

    fn clear_buf(&mut self) {
        self.buf = [0u8; RING_BYTES];
    }

    fn set_pixel(&mut self, idx: usize, r: u8, g: u8, b: u8) {
        if idx < self.led_count {
            let base = idx * 3;
            self.buf[base] = r;
            self.buf[base + 1] = g;
            self.buf[base + 2] = b;
        }
    }

    fn fill(&mut self, r: u8, g: u8, b: u8) -> Result<()> {
        for idx in 0..self.led_count {
            self.set_pixel(idx, r, g, b);
        }
        self.flush()
    }

    fn flush(&mut self) -> Result<()> {
        self.ensure_lease()?;
        self.dirty = true;

        if let Some(ref lease) = self.lease {
            if let Some(shm) = lease.shm_mut() {
                let p = lease.priority as usize;
                shm.ring_frames[p] = self.buf;
                shm.ring_has_frame[p] = 1;
            }
            if !lease.is_top_priority() {
                return Ok(());
            }
        }

        #[cfg(unix)]
        {
            self.write_hardware(&self.buf)?;
        }
        Ok(())
    }

    fn off(&mut self) -> Result<()> {
        self.clear_buf();
        self.flush()
    }
}

impl Drop for Fb5RingDriver {
    fn drop(&mut self) {
        #[cfg(unix)]
        if self.dirty {
            let mut restored_other = false;
            if let Some(ref lease) = self.lease {
                if let Some(other_prio) = lease.any_other_priority_active() {
                    if let Some(shm) = lease.shm_mut() {
                        let p = other_prio as usize;
                        if shm.ring_has_frame[p] != 0 {
                            let frame = shm.ring_frames[p];
                            let _ = self.write_hardware(&frame);
                            restored_other = true;
                        }
                    }
                }
            }
            if !restored_other {
                let orig = self
                    .lease
                    .as_ref()
                    .and_then(|l| l.shm_mut())
                    .filter(|s| s.orig_ring_saved != 0)
                    .map(|s| {
                        s.orig_ring_saved = 0;
                        s.orig_ring_frame
                    })
                    .unwrap_or(self.saved_frame);
                let _ = self.write_hardware(&orig);
            }
        }
        self.lease = None;
        #[cfg(unix)]
        if !self.mmap_ptr.is_null() {
            unsafe {
                let _ = libc::munmap(self.mmap_ptr.cast(), MMAP_LEN);
            }
            self.mmap_ptr = std::ptr::null_mut();
        }
    }
}

pub fn parse_hex_color(s: &str) -> Result<(u8, u8, u8)> {
    let hex = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if hex.len() != 6 {
        bail!("цвет должен быть в формате #RRGGBB, получено: {:?}", s);
    }
    let r = u8::from_str_radix(&hex[0..2], 16)?;
    let g = u8::from_str_radix(&hex[2..4], 16)?;
    let b = u8::from_str_radix(&hex[4..6], 16)?;
    Ok((r, g, b))
}

pub fn hsv_to_rgb(h_deg: f32, s: u8, v: u8) -> (u8, u8, u8) {
    if s == 0 {
        return (v, v, v);
    }
    let h = ((h_deg % 360.0) + 360.0) % 360.0;
    let sector = (h / 60.0) as u32;
    let frac = (h / 60.0) - (sector as f32);

    let vf = f32::from(v);
    let sf = f32::from(s) / 255.0;
    let p = (vf * (1.0 - sf)) as u8;
    let q = (vf * (1.0 - sf * frac)) as u8;
    let t = (vf * (1.0 - sf * (1.0 - frac))) as u8;

    match sector {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}
