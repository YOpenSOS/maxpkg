//! FFI-экспорт модулей `canvas` (`25x16` матрица `fb4`) и `ring` (`24` RGB светодиода `fb5`).

use super::set_err;
use crate::modules::canvas::{CanvasHal, Fb4CanvasDriver, MATRIX_PIXELS};
use crate::modules::device::detect_device;
use crate::modules::ring::{Fb5RingDriver, RingHal};

pub struct MaxCanvasHandle(pub Fb4CanvasDriver);
pub struct MaxRingHandle(pub Fb5RingDriver);

#[no_mangle]
pub extern "C" fn max_canvas_open() -> *mut MaxCanvasHandle {
    let dev = detect_device(None);
    match Fb4CanvasDriver::open_for_device(&dev) {
        Ok(drv) => Box::into_raw(Box::new(MaxCanvasHandle(drv))),
        Err(e) => {
            set_err(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_canvas_close(h: *mut MaxCanvasHandle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_canvas_clear(h: *mut MaxCanvasHandle) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("canvas handle is null");
    };
    h.0.clear_buf();
    0
}

#[no_mangle]
pub unsafe extern "C" fn max_canvas_set_pixel(
    h: *mut MaxCanvasHandle,
    x: u32,
    y: u32,
    br: u8,
) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("canvas handle is null");
    };
    h.0.set_pixel(x as usize, y as usize, br);
    0
}

#[no_mangle]
pub unsafe extern "C" fn max_canvas_set_frame(
    h: *mut MaxCanvasHandle,
    pixels400: *const u8,
    len: usize,
) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("canvas handle is null");
    };
    if pixels400.is_null() || len < MATRIX_PIXELS {
        return set_err("pixels400 must contain 400 bytes (25x16)");
    }
    let slice = std::slice::from_raw_parts(pixels400, MATRIX_PIXELS);
    match h.0.set_frame(slice, None) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_canvas_get_brightness(h: *mut MaxCanvasHandle) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("canvas handle is null");
    };
    match h.0.get_brightness() {
        Ok(b) => i32::from(b),
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_canvas_set_brightness(h: *mut MaxCanvasHandle, br: u8) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("canvas handle is null");
    };
    match h.0.set_brightness(br) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_canvas_flush(h: *mut MaxCanvasHandle) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("canvas handle is null");
    };
    match h.0.flush() {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub extern "C" fn max_ring_open() -> *mut MaxRingHandle {
    let dev = detect_device(None);
    match Fb5RingDriver::open_for_device(&dev) {
        Ok(drv) => Box::into_raw(Box::new(MaxRingHandle(drv))),
        Err(e) => {
            set_err(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_ring_close(h: *mut MaxRingHandle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_ring_clear(h: *mut MaxRingHandle) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("ring handle is null");
    };
    h.0.clear_buf();
    0
}

#[no_mangle]
pub unsafe extern "C" fn max_ring_set_pixel(
    h: *mut MaxRingHandle,
    idx: u32,
    r: u8,
    g: u8,
    b: u8,
) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("ring handle is null");
    };
    h.0.set_pixel(idx as usize, r, g, b);
    0
}

#[no_mangle]
pub unsafe extern "C" fn max_ring_fill(h: *mut MaxRingHandle, r: u8, g: u8, b: u8) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("ring handle is null");
    };
    match h.0.fill(r, g, b) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_ring_flush(h: *mut MaxRingHandle) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("ring handle is null");
    };
    match h.0.flush() {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_ring_off(h: *mut MaxRingHandle) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("ring handle is null");
    };
    match h.0.off() {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}
