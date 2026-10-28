//! Пауза и возобновление штатных часов `com.yandex.io.sdk` (`YandexStationFrontalController::isInitialized_`),
//! чтобы штатный демон не затирал буфер `/dev/graphics/fb4` во время работы плагина.

use std::fs::{self, OpenOptions};
use std::io::BufRead;

#[cfg(unix)]
pub fn set_quasar_clock_paused(pause: bool) {
    if let Some((pid, addr)) = find_quasar_controller_addr() {
        let mem_path = format!("/proc/{pid}/mem");
        if let Ok(file) = OpenOptions::new().read(true).write(true).open(mem_path) {
            use std::os::unix::fs::FileExt;
            let val: [u8; 1] = [if pause { 0 } else { 1 }];
            let _ = file.write_all_at(&val, addr);
        }
    }
}

#[cfg(not(unix))]
pub fn set_quasar_clock_paused(_pause: bool) {}

#[cfg(unix)]
fn find_quasar_pid() -> Option<u32> {
    let entries = fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if let Ok(pid) = s.parse::<u32>() {
            if let Ok(cmdline) = fs::read(entry.path().join("cmdline")) {
                if cmdline.starts_with(b"com.yandex.io.sdk\0")
                    || cmdline.starts_with(b"com.yandex.io.sdk")
                {
                    return Some(pid);
                }
            }
        }
    }
    None
}

#[cfg(unix)]
fn find_quasar_controller_addr() -> Option<(u32, u64)> {
    use std::os::unix::fs::FileExt;

    let pid = find_quasar_pid()?;
    let maps_file = fs::File::open(format!("/proc/{pid}/maps")).ok()?;
    let lines: Vec<String> = std::io::BufReader::new(maps_file)
        .lines()
        .map_while(Result::ok)
        .collect();

    let mut fb4_mmap_addr: u64 = 0;
    for line in &lines {
        if line.contains("/dev/graphics/fb4") || line.contains("/dev/fb4") {
            let range = line.split_whitespace().next()?;
            let start_hex = range.split('-').next()?;
            fb4_mmap_addr = u64::from_str_radix(start_hex, 16).ok()?;
            break;
        }
    }
    if fb4_mmap_addr == 0 {
        return None;
    }

    let mem_file = fs::File::open(format!("/proc/{pid}/mem")).ok()?;
    let mut chunk = vec![0u8; 256 * 1024];

    for line in &lines {
        let mut parts = line.split_whitespace();
        let Some(range) = parts.next() else { continue };
        let Some(perms) = parts.next() else { continue };
        if !perms.starts_with("rw") || !perms.ends_with('p') {
            continue;
        }
        if line.contains("/dev/ashmem/") {
            continue;
        }
        let mut bounds = range.split('-');
        let start = u64::from_str_radix(bounds.next()?, 16).ok()?;
        let end = u64::from_str_radix(bounds.next()?, 16).ok()?;
        if start < 0x50_0000_0000 || (end - start) < 4096 || (end - start) > 64 * 1024 * 1024 {
            continue;
        }

        let mut cur = start;
        while cur < end {
            let to_read = ((end - cur) as usize).min(chunk.len());
            let Ok(n) = mem_file.read_at(&mut chunk[..to_read], cur) else {
                break;
            };
            if n < 24 {
                break;
            }
            let mut off = 0;
            while off <= n - 24 {
                let cand_ptr = u64::from_ne_bytes(chunk[off..off + 8].try_into().ok()?);
                if cand_ptr == fb4_mmap_addr {
                    let cand_h = i32::from_ne_bytes(chunk[off + 16..off + 20].try_into().ok()?);
                    let cand_w = i32::from_ne_bytes(chunk[off + 20..off + 24].try_into().ok()?);
                    if cand_h == 16 && cand_w == 25 {
                        let fb_ptr_va = cur + (off as u64);
                        let mut hdr = [0u8; 288];
                        if mem_file.read_at(&mut hdr, fb_ptr_va - 256).is_ok() {
                            let mut best_rel: i64 = -224;
                            for rel in (-232..=-192).step_by(8) {
                                let i1 = (256 + rel - 24) as usize;
                                let i2 = (256 + rel - 8) as usize;
                                let vp1 = u64::from_ne_bytes(hdr[i1..i1 + 8].try_into().ok()?);
                                let vp2 = u64::from_ne_bytes(hdr[i2..i2 + 8].try_into().ok()?);
                                if vp2 > vp1 && (vp2 - vp1) >= 0x30 && (vp2 - vp1) <= 0x100 {
                                    best_rel = rel;
                                    break;
                                }
                            }
                            let is_init_addr = (fb_ptr_va as i64 + best_rel) as u64;
                            return Some((pid, is_init_addr));
                        }
                    }
                }
                off += 8;
            }
            if to_read < chunk.len() {
                break;
            }
            cur += (chunk.len() - 32) as u64;
        }
    }
    None
}
