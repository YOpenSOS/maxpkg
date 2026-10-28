//! Прямое управление цифровыми усилителями `SY6045S` через `/dev/snd/controlC0` (`Zero-Open` ioctl)
//! с автоматическим откатом в `Drop`, плюс управление громкостью Android `STREAM_MUSIC`.

use super::aaudio::AAudioPlayer;
use super::sound_override::SoundOverrideManager;
use super::{AmpConfig, AmpStatus, AudioHal};
use crate::modules::arbiter::{ResourceKind, ResourceLease};
use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::process::Command;

// Индексы ALSA numid (numid = ctl_id + 1) на карте 0 (AML-AUGESOUND Станции Макс):
const NUMID_WOOFER_VOL: u32 = 1; // ctl 0: Woofer Master Playback Volume (0..255)
const NUMID_WOOFER_SW: u32 = 4; // ctl 3: Woofer Master Playback Switch (bool)
const NUMID_WOOFER_EQ: u32 = 7; // ctl 6: Woofer EQ Switch (bool)
const NUMID_WOOFER_DRC: u32 = 8; // ctl 7: Woofer DRC Switch (bool)
const NUMID_TWEETERS_VOL: u32 = 10; // ctl 9: Tweeters Master Playback Volume (0..255)
const NUMID_TWEETERS_SW: u32 = 13; // ctl 12: Tweeters Master Playback Switch (bool)
const NUMID_TWEETERS_EQ: u32 = 16; // ctl 15: Tweeters EQ Switch (bool)
const NUMID_TWEETERS_DRC: u32 = 17; // ctl 16: Tweeters DRC Switch (bool)
const NUMID_HEADPHONE_VOL: u32 = 19; // ctl 18: Headphone DAC Playback Volume (0..255)
const NUMID_MASTER_MUTE: u32 = 68; // ctl 67: Master DSP Mute Switch (bool: 1=Muted, 0=Unmuted)

#[cfg(target_os = "android")]
const SNDRV_CTL_IOCTL_ELEM_READ: libc::c_int = 0xC4C8_5512u32 as libc::c_int;
#[cfg(target_os = "android")]
const SNDRV_CTL_IOCTL_ELEM_WRITE: libc::c_int = 0xC4C8_5513u32 as libc::c_int;

/// Структура `snd_ctl_elem_value` ядра Linux (64-бит ABI: 1224 байта).
#[repr(C)]
struct SndCtlElemValue {
    numid: u32,
    iface: i32,
    device: u32,
    subdevice: u32,
    name: [u8; 44],
    index: u32,
    indirect: u32,
    _pad: u32,
    values: [i64; 128],
    reserved: [u8; 128],
}

impl SndCtlElemValue {
    fn new(numid: u32) -> Self {
        Self {
            numid,
            iface: 2, // SNDRV_CTL_ELEM_IFACE_MIXER
            device: 0,
            subdevice: 0,
            name: [0u8; 44],
            index: 0,
            indirect: 0,
            _pad: 0,
            values: [0i64; 128],
            reserved: [0u8; 128],
        }
    }
}

pub struct StationMaxAudioDriver {
    lease: Option<ResourceLease>,
    ctl_file: Option<File>,
    saved_amp: Option<AmpStatus>,
    player: AAudioPlayer,
    sound_overrides: SoundOverrideManager,
}

impl StationMaxAudioDriver {
    pub fn open() -> Self {
        let ctl_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/snd/controlC0")
            .ok();

        Self {
            lease: None,
            ctl_file,
            saved_amp: None,
            player: AAudioPlayer::new(),
            sound_overrides: SoundOverrideManager::new(),
        }
    }

    pub fn ensure_sound_overrides(&self) {
        self.sound_overrides.ensure_all_applied();
    }

    #[cfg(target_os = "android")]
    fn ctl_read(&self, numid: u32) -> Option<i64> {
        use std::os::unix::io::AsRawFd;
        let file = self.ctl_file.as_ref()?;
        let mut elem = SndCtlElemValue::new(numid);
        let rc = unsafe { libc::ioctl(file.as_raw_fd(), SNDRV_CTL_IOCTL_ELEM_READ, &mut elem) };
        if rc == 0 {
            Some(elem.values[0])
        } else {
            None
        }
    }

    #[cfg(not(target_os = "android"))]
    fn ctl_read(&self, _numid: u32) -> Option<i64> {
        None
    }

    #[cfg(target_os = "android")]
    fn ctl_write(&self, numid: u32, val: i64) -> bool {
        use std::os::unix::io::AsRawFd;
        let Some(file) = self.ctl_file.as_ref() else {
            return false;
        };
        let mut elem = SndCtlElemValue::new(numid);
        elem.values[0] = val;
        let rc = unsafe { libc::ioctl(file.as_raw_fd(), SNDRV_CTL_IOCTL_ELEM_WRITE, &mut elem) };
        rc == 0
    }

    #[cfg(not(target_os = "android"))]
    fn ctl_write(&self, _numid: u32, _val: i64) -> bool {
        false
    }

    fn apply_amp_status(&mut self, st: &AmpStatus) {
        if self.ctl_file.is_some() {
            let master_mute = i64::from(!st.woofer_on && !st.tweeters_on);
            self.ctl_write(NUMID_MASTER_MUTE, master_mute);
            self.ctl_write(NUMID_WOOFER_VOL, st.woofer as i64);
            self.ctl_write(NUMID_TWEETERS_VOL, st.tweeters as i64);
            self.ctl_write(NUMID_HEADPHONE_VOL, st.aux as i64);
            self.ctl_write(NUMID_WOOFER_SW, i64::from(st.woofer_on));
            self.ctl_write(NUMID_TWEETERS_SW, i64::from(st.tweeters_on));
            self.ctl_write(NUMID_WOOFER_EQ, i64::from(st.eq));
            self.ctl_write(NUMID_TWEETERS_EQ, i64::from(st.eq));
            self.ctl_write(NUMID_WOOFER_DRC, i64::from(st.drc));
            self.ctl_write(NUMID_TWEETERS_DRC, i64::from(st.drc));
        }
    }
}

impl AudioHal for StationMaxAudioDriver {
    fn get_volume(&self) -> Result<u8> {
        let out = Command::new("/system/bin/media")
            .args(["volume", "--stream", "3", "--get"])
            .output()
            .context("ошибка вызова /system/bin/media volume --get")?;
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(idx) = line.find("volume is ") {
                let rest = &line[idx + "volume is ".len()..];
                if let Some(num_str) = rest.split_whitespace().next() {
                    if let Ok(v) = num_str.parse::<u8>() {
                        return Ok(v);
                    }
                }
            }
        }
        anyhow::bail!("не удалось разобрать текущую громкость из вывода /system/bin/media")
    }

    fn set_volume(&mut self, percent: u8) -> Result<()> {
        let clamped = percent.min(100);
        Command::new("/system/bin/media")
            .args(["volume", "--stream", "3", "--set", &clamped.to_string()])
            .output()
            .context("ошибка вызова /system/bin/media volume --set")?;
        Ok(())
    }

    fn is_muted(&self) -> Result<bool> {
        let st = self.read_amp()?;
        let master_muted = self.ctl_read(NUMID_MASTER_MUTE).unwrap_or(0) != 0;
        Ok(master_muted || (!st.woofer_on && !st.tweeters_on))
    }

    fn set_mute(&mut self, mute: bool) -> Result<()> {
        self.configure_amp(&AmpConfig {
            woofer_on: Some(!mute),
            tweeters_on: Some(!mute),
            ..Default::default()
        })
    }

    fn read_amp(&self) -> Result<AmpStatus> {
        if self.ctl_file.is_some() {
            if let Some(woofer) = self.ctl_read(NUMID_WOOFER_VOL) {
                return Ok(AmpStatus {
                    woofer: woofer.clamp(0, 255) as u8,
                    tweeters: self.ctl_read(NUMID_TWEETERS_VOL).unwrap_or(174).clamp(0, 255) as u8,
                    aux: self.ctl_read(NUMID_HEADPHONE_VOL).unwrap_or(113).clamp(0, 255) as u8,
                    woofer_on: self.ctl_read(NUMID_WOOFER_SW).unwrap_or(1) != 0,
                    tweeters_on: self.ctl_read(NUMID_TWEETERS_SW).unwrap_or(1) != 0,
                    eq: self.ctl_read(NUMID_WOOFER_EQ).unwrap_or(1) != 0,
                    drc: self.ctl_read(NUMID_WOOFER_DRC).unwrap_or(1) != 0,
                });
            }
        }
        anyhow::bail!("интерфейс /dev/snd/controlC0 недоступен")
    }

    fn configure_amp(&mut self, cfg: &AmpConfig) -> Result<()> {
        if self.lease.is_none() {
            self.lease = Some(ResourceLease::acquire(ResourceKind::Amp)?);
        }
        let mut current = self.read_amp()?;
        if self.saved_amp.is_none() {
            self.saved_amp = Some(current);
        }
        if let Some(v) = cfg.woofer {
            current.woofer = v;
        }
        if let Some(v) = cfg.tweeters {
            current.tweeters = v;
        }
        if let Some(v) = cfg.aux {
            current.aux = v;
        }
        if let Some(v) = cfg.woofer_on {
            current.woofer_on = v;
        }
        if let Some(v) = cfg.tweeters_on {
            current.tweeters_on = v;
        }
        if let Some(v) = cfg.eq {
            current.eq = v;
        }
        if let Some(v) = cfg.drc {
            current.drc = v;
        }
        self.apply_amp_status(&current);
        Ok(())
    }

    fn override_sound(&mut self, target_rel: &str, custom_path: &Path) -> Result<()> {
        self.sound_overrides.override_sound(target_rel, custom_path)
    }

    fn restore_sounds(&mut self) -> Result<()> {
        self.sound_overrides.restore_all()
    }

    fn play_tone(&self, freq_hz: f32, duration_ms: u32, volume: f32) -> Result<()> {
        self.player.play_tone(freq_hz, duration_ms, volume)
    }

    fn play_wav(&mut self, path: &Path) -> Result<()> {
        self.player.play_wav(path)
    }

    fn stop(&mut self) {
        self.player.stop();
    }

    fn track(&self) -> Result<super::MediaSessionInfo> {
        super::media_session::read_current_track()
    }

    fn sessions(&self) -> Result<Vec<super::MediaSessionInfo>> {
        super::media_session::read_media_sessions()
    }
}

impl Drop for StationMaxAudioDriver {
    fn drop(&mut self) {
        self.player.stop();
        let _ = self.sound_overrides.restore_all();
        if let Some(orig) = self.saved_amp.take() {
            self.apply_amp_status(&orig);
        }
        self.lease = None;
    }
}
