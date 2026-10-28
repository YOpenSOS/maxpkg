//! Нативное воспроизведение звука через `/system/lib64/libaaudio.so` (Android NDK AAudio)
//! без порождения внешних процессов и без конфликта с `audioserver` / Алисой.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub struct AAudioPlayer {
    stop_flag: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl AAudioPlayer {
    pub fn new() -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    /// Синтезирует синусоидальный тон с плавной огибающей 5 мс (чтобы не было щелчков)
    /// и синхронно проигрывает через `AAudio`.
    pub fn play_tone(&self, freq_hz: f32, duration_ms: u32, volume: f32) -> Result<()> {
        if duration_ms == 0 || freq_hz <= 0.0 {
            return Ok(());
        }
        let sample_rate = 48_000u32;
        let total_frames = ((sample_rate as u64 * duration_ms as u64) / 1000) as usize;
        let fade_frames = ((sample_rate as usize * 5) / 1000).min(total_frames / 2).max(1);
        let amp = (volume.clamp(0.0, 1.0) * 32000.0) as f32;

        let mut pcm = Vec::with_capacity(total_frames);
        for i in 0..total_frames {
            let env = if i < fade_frames {
                i as f32 / fade_frames as f32
            } else if i + fade_frames > total_frames {
                (total_frames - i) as f32 / fade_frames as f32
            } else {
                1.0
            };
            let phase = 2.0 * std::f32::consts::PI * freq_hz * (i as f32 / sample_rate as f32);
            pcm.push((phase.sin() * amp * env) as i16);
        }

        write_pcm_aaudio(&pcm, sample_rate as i32, 1, &self.stop_flag)
    }

    /// Запускает фоновое воспроизведение `.wav` файла (8/16-bit PCM, mono/stereo) через `AAudio`.
    pub fn play_wav(&mut self, path: &Path) -> Result<()> {
        self.stop();
        let bytes = fs::read(path)
            .with_context(|| format!("не удалось прочитать WAV файл {}", path.display()))?;
        let (pcm, sample_rate, channels) = parse_wav_pcm16(&bytes)?;

        self.stop_flag.store(false, Ordering::SeqCst);
        let flag = self.stop_flag.clone();
        self.worker = Some(thread::spawn(move || {
            let _ = write_pcm_aaudio(&pcm, sample_rate as i32, channels as i32, &flag);
        }));
        Ok(())
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        self.stop_flag.store(false, Ordering::SeqCst);
    }
}

/// Разбирает стандартный RIFF WAVE (8-bit unsigned или 16-bit signed PCM) в `Vec<i16>`.
fn parse_wav_pcm16(data: &[u8]) -> Result<(Vec<i16>, u32, u16)> {
    if data.len() < 44 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        bail!("некорректный заголовок RIFF/WAVE");
    }
    let mut pos = 12usize;
    let mut channels = 1u16;
    let mut sample_rate = 48_000u32;
    let mut bits_per_sample = 16u16;

    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        pos += 8;
        if pos + chunk_len > data.len() {
            break;
        }
        if chunk_id == b"fmt " && chunk_len >= 16 {
            let audio_fmt = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap());
            if audio_fmt != 1 {
                bail!("поддерживается только несжатый PCM WAV (format=1)");
            }
            channels = u16::from_le_bytes(data[pos + 2..pos + 4].try_into().unwrap());
            sample_rate = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap());
            bits_per_sample = u16::from_le_bytes(data[pos + 14..pos + 16].try_into().unwrap());
        } else if chunk_id == b"data" {
            let raw = &data[pos..pos + chunk_len];
            let pcm = match bits_per_sample {
                16 => raw
                    .chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]]))
                    .collect(),
                8 => raw
                    .iter()
                    .map(|&b| ((b as i16) - 128) * 256)
                    .collect(),
                other => bail!("неподдерживаемая разрядность WAV: {} бит", other),
            };
            return Ok((pcm, sample_rate, channels.max(1)));
        }
        pos += chunk_len + (chunk_len & 1);
    }
    bail!("в WAV файле не найден чанк data")
}

#[cfg(target_os = "android")]
fn write_pcm_aaudio(
    pcm: &[i16],
    sample_rate: i32,
    channels: i32,
    stop_flag: &AtomicBool,
) -> Result<()> {
    use libc::{c_char, c_int, c_void, dlopen, dlsym, RTLD_NOW};

    type CreateBuilderFn = unsafe extern "C" fn(*mut *mut c_void) -> c_int;
    type SetIntFn = unsafe extern "C" fn(*mut c_void, c_int);
    type OpenStreamFn = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> c_int;
    type DeleteBuilderFn = unsafe extern "C" fn(*mut c_void) -> c_int;
    type StreamActionFn = unsafe extern "C" fn(*mut c_void) -> c_int;
    type StreamWriteFn =
        unsafe extern "C" fn(*mut c_void, *const c_void, i32, i64) -> c_int;

    unsafe {
        let lib = dlopen(b"libaaudio.so\0".as_ptr() as *const c_char, RTLD_NOW);
        if lib.is_null() {
            bail!("не удалось загрузить /system/lib64/libaaudio.so");
        }
        macro_rules! sym {
            ($name:literal, $t:ty) => {{
                let p = dlsym(lib, concat!($name, "\0").as_ptr() as *const c_char);
                if p.is_null() {
                    bail!("символ {} не найден в libaaudio.so", $name);
                }
                std::mem::transmute::<*mut c_void, $t>(p)
            }};
        }

        let create_builder = sym!("AAudio_createStreamBuilder", CreateBuilderFn);
        let set_format = sym!("AAudioStreamBuilder_setFormat", SetIntFn);
        let set_channels = sym!("AAudioStreamBuilder_setChannelCount", SetIntFn);
        let set_rate = sym!("AAudioStreamBuilder_setSampleRate", SetIntFn);
        let set_usage = sym!("AAudioStreamBuilder_setUsage", SetIntFn);
        let open_stream = sym!("AAudioStreamBuilder_openStream", OpenStreamFn);
        let delete_builder = sym!("AAudioStreamBuilder_delete", DeleteBuilderFn);
        let request_start = sym!("AAudioStream_requestStart", StreamActionFn);
        let write_stream = sym!("AAudioStream_write", StreamWriteFn);
        let request_stop = sym!("AAudioStream_requestStop", StreamActionFn);
        let close_stream = sym!("AAudioStream_close", StreamActionFn);

        let mut builder: *mut c_void = std::ptr::null_mut();
        if create_builder(&mut builder) != 0 || builder.is_null() {
            bail!("AAudio_createStreamBuilder вернул ошибку");
        }
        set_format(builder, 1); // AAUDIO_FORMAT_PCM_I16 = 1
        set_channels(builder, channels);
        set_rate(builder, sample_rate);
        set_usage(builder, 1); // AAUDIO_USAGE_MEDIA = 1

        let mut stream: *mut c_void = std::ptr::null_mut();
        let rc = open_stream(builder, &mut stream);
        delete_builder(builder);
        if rc != 0 || stream.is_null() {
            bail!("AAudioStreamBuilder_openStream вернул код {}", rc);
        }

        request_start(stream);
        let ch = channels.max(1) as usize;
        let total_frames = pcm.len() / ch;
        let chunk_frames = 1024usize;
        let mut frame_idx = 0usize;

        while frame_idx < total_frames && !stop_flag.load(Ordering::Relaxed) {
            let n = (total_frames - frame_idx).min(chunk_frames);
            let ptr = pcm.as_ptr().add(frame_idx * ch) as *const c_void;
            let written = write_stream(stream, ptr, n as i32, 500_000_000);
            if written <= 0 {
                break;
            }
            frame_idx += written as usize;
        }

        request_stop(stream);
        close_stream(stream);
    }
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn write_pcm_aaudio(
    pcm: &[i16],
    sample_rate: i32,
    channels: i32,
    stop_flag: &AtomicBool,
) -> Result<()> {
    let ch = channels.max(1) as usize;
    let total_frames = pcm.len() / ch;
    let duration_ms = (total_frames as u64 * 1000) / (sample_rate.max(1) as u64);
    let step_ms = 25u64;
    let mut elapsed = 0u64;
    while elapsed < duration_ms && !stop_flag.load(Ordering::Relaxed) {
        let wait = (duration_ms - elapsed).min(step_ms);
        std::thread::sleep(std::time::Duration::from_millis(wait));
        elapsed += wait;
    }
    Ok(())
}
