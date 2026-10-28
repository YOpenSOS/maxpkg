//! Низкоуровневая работа с разделяемой памятью `arbiter.shm` (`MAP_SHARED`) и слотами владельцев.

use super::{ArbiterShm, Priority, ResourceKind, SlotEntry, ARBITER_MAGIC, ARBITER_VERSION};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

pub fn locks_dir() -> PathBuf {
    if Path::new("/data/local/tmp").exists() {
        PathBuf::from("/data/local/tmp/max-platform/store/locks")
    } else {
        std::env::temp_dir().join("max-platform-locks")
    }
}

pub fn lock_file_path(resource: ResourceKind, priority: Priority) -> PathBuf {
    locks_dir().join(format!("{}.{}.lock", resource.as_str(), priority.as_str()))
}

pub fn open_or_create_shm(dir: &Path) -> (Option<File>, *mut ArbiterShm) {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let path = dir.join("arbiter.shm");
        let sz = std::mem::size_of::<ArbiterShm>();
        if let Ok(file) = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
        {
            let _ = file.set_len(sz as u64);
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    sz,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    file.as_raw_fd(),
                    0,
                )
            };
            if ptr != libc::MAP_FAILED && !ptr.is_null() {
                let shm = ptr as *mut ArbiterShm;
                unsafe {
                    if (*shm).magic != ARBITER_MAGIC {
                        std::ptr::write_bytes(shm, 0, 1);
                        (*shm).magic = ARBITER_MAGIC;
                        (*shm).version = ARBITER_VERSION;
                    }
                }
                return (Some(file), shm);
            }
        }
    }
    let _ = dir;
    (None, std::ptr::null_mut())
}

pub fn write_slot_holder(
    shm: *mut ArbiterShm,
    resource: ResourceKind,
    priority: Priority,
    pkg_id: &str,
    pid: u32,
) {
    if shm.is_null() {
        return;
    }
    unsafe {
        let slot = &mut (*shm).slots[resource as usize][priority as usize];
        slot.pid = pid;
        slot.active = 1;
        slot.pkg_id = [0u8; 48];
        let bytes = pkg_id.as_bytes();
        let n = bytes.len().min(47);
        slot.pkg_id[..n].copy_from_slice(&bytes[..n]);
    }
}

pub fn clear_slot_holder(shm: *mut ArbiterShm, resource: ResourceKind, priority: Priority) {
    if shm.is_null() {
        return;
    }
    unsafe {
        let slot = &mut (*shm).slots[resource as usize][priority as usize];
        *slot = SlotEntry {
            pid: 0,
            active: 0,
            _pad: [0; 3],
            pkg_id: [0; 48],
        };
        if resource == ResourceKind::Canvas {
            (*shm).canvas_has_frame[priority as usize] = 0;
        } else if resource == ResourceKind::Ring {
            (*shm).ring_has_frame[priority as usize] = 0;
        }
    }
}

pub fn read_slot_holder(
    shm: *mut ArbiterShm,
    resource: ResourceKind,
    priority: Priority,
) -> (String, u32) {
    if shm.is_null() {
        return ("unknown".into(), 0);
    }
    unsafe {
        let slot = &(*shm).slots[resource as usize][priority as usize];
        let end = slot
            .pkg_id
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(slot.pkg_id.len());
        let name = String::from_utf8_lossy(&slot.pkg_id[..end]).to_string();
        (
            if name.is_empty() {
                "unknown".into()
            } else {
                name
            },
            slot.pid,
        )
    }
}
