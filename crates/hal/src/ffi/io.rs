//! FFI-экспорт модулей `events` (`EventBus`), `input` (`evdev`), `remote` (`uinput`) и `sensors` (`IIO` / `thermal` / `RTC`).

use super::{copy_str_to_buf, cstr_to_str, set_err, MaxInputEvent, MaxLightReading, MaxThermalReading};
use crate::modules::device::detect_device;
use crate::modules::events::{EventBus, MaxEvent};
use crate::modules::input::{EvdevDriver, InputHal};
use crate::modules::remote::{RemoteHal, UinputRemote};
use crate::modules::sensors::{SensorsHal, SysfsSensorsDriver};
use libc::c_char;

pub struct MaxEventsHandle(pub EventBus);
pub struct MaxInputHandle(pub EvdevDriver);
pub struct MaxRemoteHandle(pub UinputRemote);
pub struct MaxSensorsHandle(pub SysfsSensorsDriver);

#[no_mangle]
pub extern "C" fn max_events_open() -> *mut MaxEventsHandle {
    let dev = detect_device(None);
    match EventBus::open(&dev) {
        Ok(bus) => Box::into_raw(Box::new(MaxEventsHandle(bus))),
        Err(e) => {
            set_err(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_events_close(h: *mut MaxEventsHandle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_events_set_tick_ms(h: *mut MaxEventsHandle, period_ms: u32) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("events handle is null");
    };
    match h.0.set_tick_ms(period_ms) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_events_set_input_grab(h: *mut MaxEventsHandle, grab: u8) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("events handle is null");
    };
    match h.0.set_input_grab(grab != 0) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

/// Ожидает следующее событие из единой шины `EventBus` до `timeout_ms` мс (`< 0` — бесконечно).
/// Возвращает `1` при получении события, `0` по таймауту, `-1` при ошибке.
#[no_mangle]
pub unsafe extern "C" fn max_events_next(
    h: *mut MaxEventsHandle,
    timeout_ms: i32,
    out: *mut MaxEvent,
) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("events handle is null");
    };
    if out.is_null() {
        return set_err("out pointer is null");
    }
    match h.0.next_event(timeout_ms) {
        Ok(Some(ev)) => {
            *out = ev;
            1
        }
        Ok(None) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub extern "C" fn max_input_open() -> *mut MaxInputHandle {
    let dev = detect_device(None);
    match EvdevDriver::open_for_device(&dev) {
        Ok(drv) => Box::into_raw(Box::new(MaxInputHandle(drv))),
        Err(e) => {
            set_err(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_input_close(h: *mut MaxInputHandle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_input_is_mic_muted(h: *mut MaxInputHandle) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("input handle is null");
    };
    match h.0.is_mic_muted() {
        Ok(v) => i32::from(v),
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_input_is_aux_inserted(h: *mut MaxInputHandle) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("input handle is null");
    };
    match h.0.is_aux_inserted() {
        Ok(v) => i32::from(v),
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_input_get_wheel_step(h: *mut MaxInputHandle) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("input handle is null");
    };
    match h.0.get_wheel_step() {
        Ok(v) => i32::from(v),
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_input_set_wheel_step(h: *mut MaxInputHandle, step: u8) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("input handle is null");
    };
    match h.0.set_wheel_step(step) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_input_set_grab(h: *mut MaxInputHandle, grab: u8) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("input handle is null");
    };
    match h.0.set_grab(grab != 0) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

/// Ждёт событие ввода до `timeout_ms`.
/// Возвращает `1` при получении события, `0` по таймауту, `-1` при ошибке.
#[no_mangle]
pub unsafe extern "C" fn max_input_next_event(
    h: *mut MaxInputHandle,
    timeout_ms: u32,
    out: *mut MaxInputEvent,
) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("input handle is null");
    };
    if out.is_null() {
        return set_err("out pointer is null");
    }
    match h.0.next_event(u64::from(timeout_ms)) {
        Ok(Some(ev)) => {
            let mut c_ev: MaxInputEvent = std::mem::zeroed();
            copy_str_to_buf(&ev.source, &mut c_ev.source);
            copy_str_to_buf(&ev.action, &mut c_ev.action);
            c_ev.delta = ev.delta;
            c_ev.held_ms = ev.held_ms;
            c_ev.code = ev.code;
            c_ev.value = ev.value;
            c_ev.time_ms = ev.time_ms;
            *out = c_ev;
            1
        }
        Ok(None) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub extern "C" fn max_remote_open() -> *mut MaxRemoteHandle {
    match UinputRemote::open() {
        Ok(r) => Box::into_raw(Box::new(MaxRemoteHandle(r))),
        Err(e) => {
            set_err(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_remote_close(h: *mut MaxRemoteHandle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_remote_send_key(h: *mut MaxRemoteHandle, key: *const c_char) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("remote handle is null");
    };
    let Some(k) = cstr_to_str(key) else {
        return set_err("key string is null");
    };
    match h.0.send_key(k) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub extern "C" fn max_sensors_open() -> *mut MaxSensorsHandle {
    Box::into_raw(Box::new(MaxSensorsHandle(SysfsSensorsDriver::open())))
}

#[no_mangle]
pub unsafe extern "C" fn max_sensors_close(h: *mut MaxSensorsHandle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_sensors_read_light(
    h: *mut MaxSensorsHandle,
    out: *mut MaxLightReading,
) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("sensors handle is null");
    };
    if out.is_null() {
        return set_err("out pointer is null");
    }
    match h.0.read_light() {
        Ok(r) => {
            *out = MaxLightReading {
                lux: r.lux,
                raw: r.raw,
                gain: r.gain,
                integration_ms: r.integration_ms,
            };
            0
        }
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_sensors_configure_light(
    h: *mut MaxSensorsHandle,
    gain: u8,
    integration_ms: u16,
) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("sensors handle is null");
    };
    match h.0.configure_light(gain, integration_ms) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_sensors_read_thermal(
    h: *mut MaxSensorsHandle,
    out: *mut MaxThermalReading,
) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("sensors handle is null");
    };
    if out.is_null() {
        return set_err("out pointer is null");
    }
    match h.0.read_thermal() {
        Ok(t) => {
            *out = MaxThermalReading {
                soc_c: t.soc_c,
                ddr_c: t.ddr_c,
                wifi_c: t.wifi_c,
            };
            0
        }
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_sensors_cpu_freq_mhz(h: *mut MaxSensorsHandle) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("sensors handle is null");
    };
    match h.0.read_cpu_freq_mhz() {
        Ok(v) => v as i32,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_sensors_rtc_epoch(h: *mut MaxSensorsHandle) -> i64 {
    let Some(h) = h.as_ref() else {
        return set_err("sensors handle is null") as i64;
    };
    match h.0.read_rtc_epoch() {
        Ok(v) => v as i64,
        Err(e) => set_err(e) as i64,
    }
}
