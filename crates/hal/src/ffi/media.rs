//! FFI-экспорт модулей `audio` (`SY6045S` / `AAudio` / `media_session`) и `mic` (`AudioFlinger SHM`).

use super::{copy_str_to_buf, cstr_to_str, set_err, MaxAmpStatus, MaxMicInfo, MaxTrackInfo};
use crate::modules::audio::{AmpConfig, AudioHal, StationMaxAudioDriver};
use crate::modules::mic::{AudioFlingerShmDriver, MicHal};
use libc::c_char;
use std::path::Path;

pub struct MaxAudioHandle(pub StationMaxAudioDriver);
pub struct MaxMicHandle(pub AudioFlingerShmDriver);

#[no_mangle]
pub extern "C" fn max_audio_open() -> *mut MaxAudioHandle {
    Box::into_raw(Box::new(MaxAudioHandle(StationMaxAudioDriver::open())))
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_close(h: *mut MaxAudioHandle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_get_volume(h: *mut MaxAudioHandle) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("audio handle is null");
    };
    match h.0.get_volume() {
        Ok(v) => i32::from(v),
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_set_volume(h: *mut MaxAudioHandle, percent: u8) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("audio handle is null");
    };
    match h.0.set_volume(percent) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_is_muted(h: *mut MaxAudioHandle) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("audio handle is null");
    };
    match h.0.is_muted() {
        Ok(v) => i32::from(v),
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_set_mute(h: *mut MaxAudioHandle, mute: u8) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("audio handle is null");
    };
    match h.0.set_mute(mute != 0) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_override_sound(
    h: *mut MaxAudioHandle,
    target_rel: *const c_char,
    custom_path: *const c_char,
) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("audio handle is null");
    };
    let Some(target) = cstr_to_str(target_rel) else {
        return set_err("target_rel is null");
    };
    let Some(custom) = cstr_to_str(custom_path) else {
        return set_err("custom_path is null");
    };
    match h.0.override_sound(target, Path::new(custom)) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_restore_sounds(h: *mut MaxAudioHandle) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("audio handle is null");
    };
    match h.0.restore_sounds() {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_read_amp(
    h: *mut MaxAudioHandle,
    out: *mut MaxAmpStatus,
) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("audio handle is null");
    };
    if out.is_null() {
        return set_err("out pointer is null");
    }
    match h.0.read_amp() {
        Ok(st) => {
            *out = MaxAmpStatus {
                woofer: st.woofer,
                tweeters: st.tweeters,
                aux: st.aux,
                woofer_on: u8::from(st.woofer_on),
                tweeters_on: u8::from(st.tweeters_on),
                eq: u8::from(st.eq),
                drc: u8::from(st.drc),
            };
            0
        }
        Err(e) => set_err(e),
    }
}

/// Устанавливает усиление `SY6045S` (`-1` оставляет текущее значение параметра без изменений).
#[no_mangle]
pub unsafe extern "C" fn max_audio_set_amp(
    h: *mut MaxAudioHandle,
    woofer: i32,
    tweeters: i32,
    aux: i32,
    eq: i32,
    drc: i32,
) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("audio handle is null");
    };
    let cfg = AmpConfig {
        woofer: (0..=255).contains(&woofer).then_some(woofer as u8),
        tweeters: (0..=255).contains(&tweeters).then_some(tweeters as u8),
        aux: (0..=255).contains(&aux).then_some(aux as u8),
        woofer_on: None,
        tweeters_on: None,
        eq: (eq >= 0).then_some(eq != 0),
        drc: (drc >= 0).then_some(drc != 0),
    };
    match h.0.configure_amp(&cfg) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_play_tone(
    h: *mut MaxAudioHandle,
    freq_hz: f32,
    duration_ms: u32,
    volume: f32,
) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("audio handle is null");
    };
    match h.0.play_tone(freq_hz, duration_ms, volume) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_play_wav(h: *mut MaxAudioHandle, path: *const c_char) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("audio handle is null");
    };
    let Some(p) = cstr_to_str(path) else {
        return set_err("wav path is null");
    };
    match h.0.play_wav(Path::new(p)) {
        Ok(()) => 0,
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_stop(h: *mut MaxAudioHandle) {
    if let Some(h) = h.as_mut() {
        h.0.stop();
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_audio_read_track(
    h: *mut MaxAudioHandle,
    out: *mut MaxTrackInfo,
) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("audio handle is null");
    };
    if out.is_null() {
        return set_err("out pointer is null");
    }
    match h.0.track() {
        Ok(tr) => {
            let mut info: MaxTrackInfo = std::mem::zeroed();
            info.active = u8::from(tr.active);
            info.playing = u8::from(tr.playing);
            info.volume = tr.state;
            info.position_ms = tr.position_ms as i64;
            info.duration_ms = tr.updated_ms as i64;
            copy_str_to_buf(&tr.package, &mut info.package);
            copy_str_to_buf(&tr.title, &mut info.title);
            copy_str_to_buf(&tr.artist, &mut info.artist);
            copy_str_to_buf(&tr.album, &mut info.album);
            *out = info;
            0
        }
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub extern "C" fn max_mic_open() -> *mut MaxMicHandle {
    Box::into_raw(Box::new(MaxMicHandle(AudioFlingerShmDriver::open())))
}

#[no_mangle]
pub unsafe extern "C" fn max_mic_close(h: *mut MaxMicHandle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_mic_info(h: *mut MaxMicHandle, out: *mut MaxMicInfo) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("mic handle is null");
    };
    if out.is_null() {
        return set_err("out pointer is null");
    }
    match h.0.info() {
        Ok(info) => {
            *out = MaxMicInfo {
                rate: info.rate,
                channels: info.channels,
                _pad: [0; 3],
                buffer_frames: info.buffer_frames,
                pid: info.pid,
                rear: info.rear,
            };
            0
        }
        Err(e) => set_err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn max_mic_is_muted(h: *mut MaxMicHandle) -> i32 {
    let Some(h) = h.as_ref() else {
        return set_err("mic handle is null");
    };
    i32::from(h.0.is_muted())
}

#[no_mangle]
pub unsafe extern "C" fn max_mic_read_peaks(
    h: *mut MaxMicHandle,
    frames: u32,
    out16: *mut u16,
) -> i32 {
    let Some(h) = h.as_mut() else {
        return set_err("mic handle is null");
    };
    if out16.is_null() {
        return set_err("out16 pointer is null");
    }
    match h.0.read_peaks(frames as usize) {
        Ok(peaks) => {
            std::ptr::copy_nonoverlapping(peaks.as_ptr(), out16, 16);
            0
        }
        Err(e) => set_err(e),
    }
}
