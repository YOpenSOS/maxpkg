//! Модуль `manifest`: чтение и валидация `package.toml` и проверка совместимости с колонкой.

use anyhow::{bail, Context, Result};
use max_hal::DeviceProfile;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    Binary,
}

impl Default for PackageKind {
    fn default() -> Self {
        Self::Binary
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompatPolicy {
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub kind: PackageKind,
    #[serde(default)]
    pub lang: Option<String>,
    pub entry: String,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub compat: CompatPolicy,
}

impl PackageManifest {
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let path = dir.join("package.toml");
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("не найден манифест пакета: {}", path.display()))?;
        let manifest: Self = toml::from_str(&raw)
            .with_context(|| format!("ошибка разбора {}", path.display()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty()
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            bail!("некорректный идентификатор пакета id: {:?}", self.id);
        }
        if self.entry.is_empty() || self.entry.contains("..") {
            bail!("некорректная точка входа entry: {:?}", self.entry);
        }
        Ok(())
    }

    /// Проверяет, подходит ли пакет для текущей колонки (`DeviceProfile`).
    pub fn check_compatibility(&self, dev: &DeviceProfile) -> Result<()> {
        if !self.compat.models.is_empty()
            && !self
                .compat
                .models
                .iter()
                .any(|m| m == "*" || m == &dev.model)
        {
            bail!(
                "пакет '{}' требует модель {:?}, а текущая колонка — '{}'",
                self.id,
                self.compat.models,
                dev.model
            );
        }
        for req_feat in &self.compat.features {
            if !dev
                .features
                .iter()
                .any(|f| f == req_feat || f.starts_with(&format!("{req_feat}.")))
            {
                bail!(
                    "пакет '{}' требует возможность '{}', которой нет на '{}'",
                    self.id,
                    req_feat,
                    dev.model
                );
            }
        }
        Ok(())
    }
}
