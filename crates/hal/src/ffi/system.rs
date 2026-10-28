//! FFI-экспорт модулей `bluetooth` (`Bluedroid`) и `system` (`LinuxSystemDriver` + приватное хранилище `data/`).

use super::{copy_str_to_buf, cstr_to_str, set_err, MaxBtStatus, MaxSysInfo};
use crate::modules::bluetooth::{BluedroidDriver, BluetoothHal};
use crate::modules::system::{LinuxSystemDriver, SystemHal};
use libc::c_char;
use std::fs;
use std::path::PathBuf;

pub struct MaxBtHandle(pub BluedroidDriver);

#[no_mangle]
pub extern "C" fn max_bt_open() -> *mut MaxBtHandle {
    Box::into_raw(Box::new(MaxBtHandle(BluedroidDriver::open())))
}

#[no_mangle]
pub unsafe extern "C" fn max_bt_close(h: *mut MaxBtHandle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_bt_status(h: *mut MaxBtHandle, out: *mut MaxBtStatus) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("bt handle is null");
    };
    if out.is_null() {
        return set_err("out pointer is null");
    }
    match h.0.status() {
        Ok(st) => {
            let mut info: MaxBtStatus = std::mem::zeroed();
            info.enabled = u8::from(st.enabled);
            info.connected = u8::from(st.connected);
            info.scan_mode = st.scan_mode;
            info.bonded_count = st.bonded_count as u32;
            copy_str_to_buf(&st.address, &mut info.address);
            copy_str_to_buf(&st.name, &mut info.name);
            *out = info;
            0
        }
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_bt_set_power(h: *mut MaxBtHandle, on: u8) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("bt handle is null");
    };
    match h.0.set_power(on != 0) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_sys_info(out: *mut MaxSysInfo) -> i32 {
    if out.is_null() {
        return set_err("out pointer is null");
    }
    let drv = LinuxSystemDriver::open();
    let t = drv.time();
    let up = drv.uptime().map(|u| u.uptime_sec).unwrap_or(0.0);
    let mem = drv.meminfo().unwrap_or(crate::modules::system::SystemMemInfo {
        total_kb: 0,
        free_kb: 0,
        available_kb: 0,
        cached_kb: 0,
        used_pct: 0.0,
    });
    let load1 = drv.loadavg().map(|l| l.load1).unwrap_or(0.0);
    let disk_free = drv.disk().map(|d| d.available_mb).unwrap_or(0);
    let wifi = drv.wifi().unwrap_or_default();

    let mut info: MaxSysInfo = std::mem::zeroed();
    info.unix_sec = t.unix_sec;
    info.uptime_sec = up;
    info.mem_total_kb = mem.total_kb;
    info.mem_avail_kb = mem.available_kb;
    info.load1 = load1;
    info.disk_free_mb = disk_free;
    info.wifi_connected = u8::from(wifi.connected);
    info.wifi_rssi_dbm = wifi.rssi_dbm;
    copy_str_to_buf(&wifi.ip, &mut info.wifi_ip);
    copy_str_to_buf(&wifi.ssid, &mut info.wifi_ssid);
    *out = info;
    0
}

#[no_mangle]
pub unsafe extern "C" fn max_sys_getprop(
    key: *const c_char,
    out_buf: *mut u8,
    out_len: usize,
) -> i32 {
    let Some(k) = cstr_to_str(key) else {
        return set_err("key is null");
    };
    if out_buf.is_null() || out_len == 0 {
        return set_err("out_buf is null");
    }
    let drv = LinuxSystemDriver::open();
    match drv.getprop(k) {
        Ok(Some(v)) => {
            let slice = std::slice::from_raw_parts_mut(out_buf, out_len);
            copy_str_to_buf(&v, slice);
            v.len() as i32
        }
        Ok(None) => {
            *out_buf = 0;
            0
        }
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_sys_setprop(key: *const c_char, val: *const c_char) -> i32 {
    let (Some(k), Some(v)) = (cstr_to_str(key), cstr_to_str(val)) else {
        return set_err("key or val is null");
    };
    let drv = LinuxSystemDriver::open();
    match drv.setprop(k, v) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

pub fn pkg_data_file(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
        return None;
    }
    let dir = std::env::var("MAX_PKG_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("max-pkg-data"));
    let _ = fs::create_dir_all(&dir);
    Some(dir.join(name))
}

#[no_mangle]
pub unsafe extern "C" fn max_sys_write_data(name: *const c_char, content: *const c_char) -> i32 {
    let (Some(n), Some(c)) = (cstr_to_str(name), cstr_to_str(content)) else {
        return set_err("name or content is null");
    };
    let Some(path) = pkg_data_file(n) else {
        return set_err("invalid data filename");
    };
    match fs::write(path, c) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_sys_read_data(
    name: *const c_char,
    out_buf: *mut u8,
    out_len: usize,
) -> i32 {
    let Some(n) = cstr_to_str(name) else {
        return set_err("name is null");
    };
    if out_buf.is_null() || out_len == 0 {
        return set_err("out_buf is null");
    }
    let Some(path) = pkg_data_file(n) else {
        return set_err("invalid data filename");
    };
    match fs::read_to_string(path) {
        Ok(s) => {
            let slice = std::slice::from_raw_parts_mut(out_buf, out_len);
            copy_str_to_buf(&s, slice);
            s.len() as i32
        }
        Err(_) => {
            *out_buf = 0;
            0
        }
    }
}
