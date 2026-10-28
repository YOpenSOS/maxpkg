//! Транзакционная подмена системных звуковых файлов `com.yandex.io.sdk`
//! (`/data/data/com.yandex.io.sdk/files/system/sounds/`) с автоматическим сохранением
//! заводских оригиналов в `MAX_PKG_DATA/sound_backups/` и возвратом в `Drop`.

use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const QUASAR_SOUNDS_BASE: &str = "/data/data/com.yandex.io.sdk/files/system/sounds";

#[derive(Clone)]
struct OverrideRecord {
    target_path: PathBuf,
    backup_path: PathBuf,
    payload: Vec<u8>,
}

/// Менеджер подмены системных звуков с гарантированным откатом при закрытии драйвера `Audio`.
#[derive(Default)]
pub struct SoundOverrideManager {
    records: HashMap<PathBuf, OverrideRecord>,
}

impl SoundOverrideManager {
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
        }
    }

    /// Подменяет системный звук `target_rel` (например, `"common/bluetooth_turning_on_jingle.opus"`
    /// или `"ru-RU/bluetooth_turning_on.mp3"`) файлом `custom_path`, сохраняя заводской оригинал.
    pub fn override_sound(&mut self, target_rel: &str, custom_path: &Path) -> Result<()> {
        let target_path = resolve_target_sound_path(target_rel)?;
        let resolved_custom = resolve_custom_sound_path(custom_path)?;
        let payload = fs::read(&resolved_custom).with_context(|| {
            format!(
                "не удалось прочитать кастомный звуковой файл {}",
                resolved_custom.display()
            )
        })?;
        if payload.is_empty() {
            bail!("кастомный звуковой файл пуст: {}", resolved_custom.display());
        }

        let backup_dir = resolve_backup_dir()?;
        let safe_name = target_rel
            .trim_start_matches('/')
            .replace(['/', '\\'], "__");
        let backup_path = backup_dir.join(safe_name);

        let rec = OverrideRecord {
            target_path: target_path.clone(),
            backup_path,
            payload,
        };
        apply_record(&rec)?;
        self.records.insert(target_path, rec);
        Ok(())
    }

    /// Проверяет и при необходимости повторно накатывает все активные подмены (вызывается по событию `inotify`).
    pub fn ensure_all_applied(&self) {
        for rec in self.records.values() {
            let _ = apply_record(rec);
        }
    }

    /// Восстанавливает все заводские звуковые файлы из бэкапа и очищает список активных подмен.
    pub fn restore_all(&mut self) -> Result<()> {
        for rec in self.records.values() {
            if rec.backup_path.exists() {
                if let Some(parent) = rec.target_path.parent() {
                    if fs::copy(&rec.backup_path, &rec.target_path).is_ok() {
                        fix_permissions_like_dir(&rec.target_path, parent);
                    }
                }
            }
        }
        self.records.clear();
        Ok(())
    }
}

impl Drop for SoundOverrideManager {
    fn drop(&mut self) {
        let _ = self.restore_all();
    }
}

fn apply_record(rec: &OverrideRecord) -> Result<()> {
    let Some(parent) = rec.target_path.parent() else {
        bail!("некорректный путь целевого звука: {}", rec.target_path.display());
    };
    if !parent.exists() {
        bail!("системная директория звуков не найдена: {}", parent.display());
    }

    let cur_len = fs::metadata(&rec.target_path)
        .map(|m| m.len() as usize)
        .unwrap_or(0);
    if cur_len == rec.payload.len() && rec.backup_path.exists() {
        return Ok(());
    }

    if rec.target_path.exists() && (!rec.backup_path.exists() || cur_len != rec.payload.len()) {
        fs::copy(&rec.target_path, &rec.backup_path).with_context(|| {
            format!(
                "не удалось сохранить бэкап {} -> {}",
                rec.target_path.display(),
                rec.backup_path.display()
            )
        })?;
    }

    fs::write(&rec.target_path, &rec.payload).with_context(|| {
        format!(
            "не удалось записать кастомный звук в {}",
            rec.target_path.display()
        )
    })?;
    fix_permissions_like_dir(&rec.target_path, parent);
    Ok(())
}

fn resolve_target_sound_path(target_rel: &str) -> Result<PathBuf> {
    let trimmed = target_rel.trim();
    if trimmed.is_empty() || trimmed.contains("..") {
        bail!("недопустимое имя системного звука: {target_rel:?}");
    }
    let base = Path::new(QUASAR_SOUNDS_BASE);
    if trimmed.starts_with('/') {
        return Ok(PathBuf::from(trimmed));
    }
    let direct = base.join(trimmed);
    if direct.exists() || trimmed.contains('/') {
        return Ok(direct);
    }
    let in_common = base.join("common").join(trimmed);
    if in_common.exists() {
        return Ok(in_common);
    }
    let in_ruru = base.join("ru-RU").join(trimmed);
    if in_ruru.exists() {
        return Ok(in_ruru);
    }
    Ok(in_common)
}

fn resolve_custom_sound_path(custom_path: &Path) -> Result<PathBuf> {
    if custom_path.is_absolute() && custom_path.exists() {
        return Ok(custom_path.to_path_buf());
    }
    if let Ok(pkg_root) = std::env::var("MAX_PKG_ROOT") {
        let candidate = Path::new(&pkg_root).join(custom_path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    if custom_path.exists() {
        return Ok(custom_path.to_path_buf());
    }
    bail!("файл кастомного звука не найден: {}", custom_path.display())
}

fn resolve_backup_dir() -> Result<PathBuf> {
    let data_dir = std::env::var("MAX_PKG_DATA")
        .unwrap_or_else(|_| "/data/local/tmp/max-platform/store/data/shared".into());
    let dir = Path::new(&data_dir).join("sound_backups");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(unix)]
fn fix_permissions_like_dir(file: &Path, dir: &Path) {
    use std::ffi::CString;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let _ = fs::set_permissions(file, fs::Permissions::from_mode(0o644));
    if let Ok(meta) = fs::metadata(dir) {
        if let Ok(c_path) = CString::new(file.as_os_str().to_string_lossy().as_bytes()) {
            unsafe {
                let _ = libc::chown(c_path.as_ptr(), meta.uid(), meta.gid());
            }
        }
    }
}

#[cfg(not(unix))]
fn fix_permissions_like_dir(_file: &Path, _dir: &Path) {}
