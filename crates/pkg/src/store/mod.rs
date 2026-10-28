//! Модуль `store`: установка, удаление, список пакетов и вычисление путей (`root`, `config`, `data`, `tmp`).

use crate::manifest::PackageManifest;
use anyhow::{bail, Context, Result};
use max_hal::DeviceProfile;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Персональные изолированные пути конкретного пакета (передаются в `device.paths`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagePaths {
    pub root: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
    pub tmp: PathBuf,
}

/// Хранилище установленных пакетов с автоматическим выбором путей под Android / Linux / ПК.
#[derive(Debug, Clone)]
pub struct PackageStore {
    pub base_dir: PathBuf,
    pub tmp_base_dir: PathBuf,
}

impl PackageStore {
    /// Создаёт хранилище, выбирая правильные системные папки по `device.os`.
    pub fn for_device(dev: &DeviceProfile, custom_root: Option<PathBuf>) -> Self {
        let base_dir = custom_root.unwrap_or_else(|| match dev.os.as_str() {
            "android" => PathBuf::from("/data/local/tmp/max-platform/store"),
            "linux" => PathBuf::from("/mnt/data/max-platform"),
            _ => std::env::temp_dir().join("max-platform-store"),
        });

        let tmp_base_dir = match dev.os.as_str() {
            // На Android 9 (Станция Макс) нет /tmp, используем /data/local/tmp
            "android" => PathBuf::from("/data/local/tmp/max-platform/tmp"),
            "linux" => PathBuf::from("/tmp/max-platform"),
            _ => std::env::temp_dir().join("max-platform-tmp"),
        };

        Self {
            base_dir,
            tmp_base_dir,
        }
    }

    pub fn pkgs_dir(&self) -> PathBuf {
        self.base_dir.join("pkgs")
    }

    /// Вычисляет и создаёт рабочие директории для пакета `pkg_id`.
    pub fn paths_for(&self, pkg_id: &str, override_root: Option<&Path>) -> Result<PackagePaths> {
        let root = override_root
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.pkgs_dir().join(pkg_id));
        let config = self.base_dir.join("config").join(pkg_id);
        let data = self.base_dir.join("data").join(pkg_id);
        let tmp = self.tmp_base_dir.join(pkg_id);

        fs::create_dir_all(&config)?;
        fs::create_dir_all(&data)?;
        fs::create_dir_all(&tmp)?;

        Ok(PackagePaths {
            root,
            config,
            data,
            tmp,
        })
    }

    /// Устанавливает пакет из директории в хранилище `pkgs/<id>`.
    pub fn install(&self, src_dir: &Path, dev: &DeviceProfile) -> Result<PackageManifest> {
        let manifest = PackageManifest::load_from_dir(src_dir)?;
        manifest.check_compatibility(dev)?;

        let dst_dir = self.pkgs_dir().join(&manifest.id);
        if dst_dir.exists() {
            fs::remove_dir_all(&dst_dir)?;
        }
        copy_dir_recursive(src_dir, &dst_dir)?;
        let _ = self.paths_for(&manifest.id, None)?;
        Ok(manifest)
    }

    /// Удаляет установленный пакет из `pkgs/<id>` (сохраняя пользовательский `config/<id>`).
    pub fn remove(&self, pkg_id: &str) -> Result<()> {
        let dst_dir = self.pkgs_dir().join(pkg_id);
        if !dst_dir.exists() {
            bail!("пакет '{}' не установлен", pkg_id);
        }
        fs::remove_dir_all(&dst_dir)?;
        Ok(())
    }

    /// Возвращает список всех установленных пакетов.
    pub fn list(&self) -> Result<Vec<PackageManifest>> {
        let mut out = Vec::new();
        let pkgs = self.pkgs_dir();
        if !pkgs.exists() {
            return Ok(out);
        }
        for entry in fs::read_dir(pkgs)? {
            let entry = entry?;
            if entry.path().is_dir() {
                if let Ok(m) = PackageManifest::load_from_dir(&entry.path()) {
                    out.push(m);
                }
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Загружает манифест установленного пакета по его `id`.
    pub fn load_installed(&self, pkg_id: &str) -> Result<PackageManifest> {
        let dir = self.pkgs_dir().join(pkg_id);
        if !dir.exists() {
            bail!("пакет '{}' не найден в {}", pkg_id, self.pkgs_dir().display());
        }
        PackageManifest::load_from_dir(&dir)
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("create_dir_all {}", dst.display()))?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}
