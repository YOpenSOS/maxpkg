//! Чтение всех медиа-сессий (`com.yandex.io.sdk`, `com.android.bluetooth`, ТВ-плееры)
//! из системного сервиса Android `media_session` с расчётом текущей позиции `position_ms`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaSessionInfo {
    pub package: String,
    pub active: bool,
    pub playing: bool,
    pub state: u8,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub raw_description: String,
    pub position_ms: u64,
    pub updated_ms: u64,
}

pub fn read_media_sessions() -> Result<Vec<MediaSessionInfo>> {
    let out = Command::new("/system/bin/dumpsys")
        .arg("media_session")
        .output()
        .context("ошибка вызова /system/bin/dumpsys media_session")?;
    let text = String::from_utf8_lossy(&out.stdout);
    let now_uptime_ms = read_uptime_ms();

    let mut sessions = Vec::new();
    let mut in_stack = false;
    let mut cur: Option<MediaSessionInfo> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with("Sessions Stack - have ") {
            in_stack = true;
            continue;
        }
        if !in_stack {
            continue;
        }
        if raw_line.starts_with("  Audio playback") || raw_line.starts_with("User Records:") {
            break;
        }

        // Новая запись сессии начинается с отступа в 4 пробела ("    <Name> <pkg>/<tag> (userId=0)")
        let indent = raw_line.len() - raw_line.trim_start().len();
        if indent == 4 && raw_line.contains("(userId=") {
            if let Some(prev) = cur.take() {
                sessions.push(prev);
            }
            cur = Some(MediaSessionInfo::default());
            continue;
        }

        let Some(s) = cur.as_mut() else {
            continue;
        };

        if let Some(pkg) = line.strip_prefix("package=") {
            s.package = pkg.trim().to_string();
        } else if let Some(act) = line.strip_prefix("active=") {
            s.active = act.trim() == "true";
        } else if let Some(st_rest) = line.strip_prefix("state=PlaybackState {") {
            let state_num = extract_kv_u64(st_rest, "state=").unwrap_or(0) as u8;
            let pos = extract_kv_u64(st_rest, "position=").unwrap_or(0);
            let upd = extract_kv_u64(st_rest, "updated=").unwrap_or(0);
            s.state = state_num;
            s.playing = state_num == 3;
            s.updated_ms = upd;
            if s.playing && upd > 0 && now_uptime_ms > upd {
                s.position_ms = pos.saturating_add(now_uptime_ms - upd);
            } else {
                s.position_ms = pos;
            }
        } else if line.starts_with("metadata:") {
            if let Some(idx) = line.find("description=") {
                let desc = line[idx + "description=".len()..].trim();
                if desc != "null" && desc != "null, null, null" {
                    s.raw_description = desc.to_string();
                    let (title, artist, album) = split_media_description(desc);
                    s.title = title;
                    s.artist = artist;
                    s.album = album;
                }
            }
        }
    }

    if let Some(last) = cur.take() {
        sessions.push(last);
    }
    Ok(sessions)
}

pub fn read_current_track() -> Result<MediaSessionInfo> {
    let sessions = read_media_sessions()?;
    if let Some(playing) = sessions.iter().find(|s| s.playing) {
        return Ok(playing.clone());
    }
    if let Some(with_meta) = sessions.iter().find(|s| !s.title.is_empty()) {
        return Ok(with_meta.clone());
    }
    if let Some(active) = sessions.iter().find(|s| s.active) {
        return Ok(active.clone());
    }
    Ok(sessions.into_iter().next().unwrap_or_default())
}

fn extract_kv_u64(s: &str, prefix: &str) -> Option<u64> {
    let idx = s.find(prefix)?;
    let rest = &s[idx + prefix.len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn split_media_description(desc: &str) -> (String, String, String) {
    let Some(first_comma) = desc.find(", ") else {
        return (desc.to_string(), String::new(), String::new());
    };
    let title = desc[..first_comma].trim().to_string();
    let after_first = &desc[first_comma + 2..];
    if let Some(last_comma) = after_first.rfind(", ") {
        let artist = after_first[..last_comma].trim().to_string();
        let raw_album = after_first[last_comma + 2..].trim();
        let album = if raw_album == "null" {
            String::new()
        } else {
            raw_album.to_string()
        };
        (title, artist, album)
    } else {
        (title, after_first.trim().to_string(), String::new())
    }
}

fn read_uptime_ms() -> u64 {
    if let Ok(s) = std::fs::read_to_string("/proc/uptime") {
        if let Some(first) = s.split_whitespace().next() {
            if let Ok(sec) = first.parse::<f64>() {
                return (sec * 1000.0) as u64;
            }
        }
    }
    0
}
