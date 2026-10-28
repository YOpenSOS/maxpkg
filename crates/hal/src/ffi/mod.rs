//! C-ABI интерфейс (`libmax_hal.so` / `libmax_hal.a`) для SDK на C, C++ и Go.

pub mod display;
pub mod io;
pub mod media;
pub mod system;

use crate::modules::arbiter::lease::{
    current_process_context, install_signal_handlers, is_stop_requested,
    set_current_process_context,
};
use crate::modules::arbiter::Priority;
use crate::modules::device::detect_device;
use libc::c_char;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::thread;
use std::time::Duration;

thread_local! {
    static LAST_ERR: RefCell<CString> = RefCell::new(CString::new("").unwrap());
}

pub(crate) fn set_err(msg: impl ToString) -> i32 {
    let s = msg.to_string().replace('\0', " ");
    LAST_ERR.with(|cell| {
        if let Ok(c) = CString::new(s) {
            *cell.borrow_mut() = c;
        }
    });
    -1
}

pub(crate) fn copy_str_to_buf(src: &str, dst: &mut [u8]) {
    dst.fill(0);
    let bytes = src.as_bytes();
    let n = bytes.len().min(dst.len().saturating_sub(1));
    dst[..n].copy_from_slice(&bytes[..n]);
}

pub(crate) unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MaxDeviceInfo {
    pub model: [u8; 32],
    pub name: [u8; 64],
    pub sku: [u8; 32],
    pub serial: [u8; 32],
    pub os: [u8; 16],
    pub arch: [u8; 16],
    pub kernel: [u8; 32],
    pub firmware: [u8; 32],
    pub led_count: u8,
    pub screen_w: u16,
    pub screen_h: u16,
    pub mic_count: u8,
    pub has_encoder: u8,
    pub has_hdmi: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MaxInputEvent {
    pub source: [u8; 16],
    pub action: [u8; 16],
    pub delta: i32,
    pub held_ms: u64,
    pub code: u16,
    pub value: i32,
    pub time_ms: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MaxLightReading {
    pub lux: f32,
    pub raw: u32,
    pub gain: u8,
    pub integration_ms: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MaxThermalReading {
    pub soc_c: f32,
    pub ddr_c: f32,
    pub wifi_c: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MaxAmpStatus {
    pub woofer: u8,
    pub tweeters: u8,
    pub aux: u8,
    pub woofer_on: u8,
    pub tweeters_on: u8,
    pub eq: u8,
    pub drc: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MaxTrackInfo {
    pub active: u8,
    pub playing: u8,
    pub volume: u8,
    pub _pad: u8,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub package: [u8; 64],
    pub title: [u8; 128],
    pub artist: [u8; 128],
    pub album: [u8; 128],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MaxMicInfo {
    pub rate: u32,
    pub channels: u8,
    pub _pad: [u8; 3],
    pub buffer_frames: u32,
    pub pid: u32,
    pub rear: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MaxBtStatus {
    pub enabled: u8,
    pub connected: u8,
    pub scan_mode: u8,
    pub _pad: u8,
    pub bonded_count: u32,
    pub address: [u8; 24],
    pub name: [u8; 64],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MaxSysInfo {
    pub unix_sec: u64,
    pub uptime_sec: f64,
    pub mem_total_kb: u64,
    pub mem_avail_kb: u64,
    pub load1: f32,
    pub wifi_rssi_dbm: i32,
    pub disk_free_mb: u64,
    pub wifi_connected: u8,
    pub _pad: [u8; 7],
    pub wifi_ip: [u8; 32],
    pub wifi_ssid: [u8; 64],
}

/// Возвращает последнюю ошибку HAL в текущем потоке.
#[no_mangle]
pub extern "C" fn max_hal_last_error() -> *const c_char {
    LAST_ERR.with(|cell| cell.borrow().as_ptr())
}

/// Инициализирует сигналы `SIGINT`/`SIGTERM` и контекст арбитра ресурсов.
/// Если `pkg_id == NULL`, автоматически берёт `MAX_PKG_ID`, `MAX_PKG_PRIORITY` и `MAX_PKG_REPLACE` из окружения.
#[no_mangle]
pub unsafe extern "C" fn max_hal_init(
    pkg_id: *const c_char,
    priority: u8,
    replace: u8,
) -> i32 {
    install_signal_handlers();
    if let Some(id) = cstr_to_str(pkg_id) {
        if !id.is_empty() {
            set_current_process_context(id, Priority::from_u8(priority), replace != 0);
            return 0;
        }
    }
    let _ = current_process_context();
    0
}

/// Возвращает `1`, если процессу пришёл `SIGINT` или `SIGTERM` (`maxctl stop` или вытеснение `--replace`).
#[no_mangle]
pub extern "C" fn max_hal_should_stop() -> i32 {
    if is_stop_requested() {
        1
    } else {
        0
    }
}

/// Спит `ms` миллисекунд с проверкой `SIGTERM` каждые 10 мс.
/// Возвращает `1`, если время вышло штатно, или `0`, если получен сигнал остановки.
#[no_mangle]
pub extern "C" fn max_hal_sleep_ms(ms: u32) -> i32 {
    let mut rem = ms as u64;
    while rem > 0 {
        if is_stop_requested() {
            return 0;
        }
        let step = rem.min(10);
        thread::sleep(Duration::from_millis(step));
        rem -= step;
    }
    if is_stop_requested() {
        0
    } else {
        1
    }
}

/// Заполняет паспорт устройства `MaxDeviceInfo`.
#[no_mangle]
pub unsafe extern "C" fn max_device_info(out: *mut MaxDeviceInfo) -> i32 {
    if out.is_null() {
        return set_err("out pointer is null");
    }
    let p = detect_device(None);
    let mut info: MaxDeviceInfo = std::mem::zeroed();
    copy_str_to_buf(&p.model, &mut info.model);
    copy_str_to_buf(&p.name, &mut info.name);
    copy_str_to_buf(&p.sku, &mut info.sku);
    copy_str_to_buf(&p.serial, &mut info.serial);
    copy_str_to_buf(&p.os, &mut info.os);
    copy_str_to_buf(&p.arch, &mut info.arch);
    copy_str_to_buf(&p.kernel, &mut info.kernel);
    copy_str_to_buf(&p.firmware, &mut info.firmware);
    info.led_count = p.hw.led_count;
    info.screen_w = p.hw.screen_w;
    info.screen_h = p.hw.screen_h;
    info.mic_count = p.hw.mic_count;
    info.has_encoder = u8::from(p.hw.has_encoder);
    info.has_hdmi = u8::from(p.hw.has_hdmi);
    *out = info;
    0
}
