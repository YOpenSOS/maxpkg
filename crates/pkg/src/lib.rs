//! Подсистема управления пакетами и супервизора процессов (`max-pkg`).
//!
//! Отвечает за:
//! - Чтение и проверку манифестов `package.toml` (`manifest/`);
//! - Управление файловым хранилищем установленных пакетов и данных `store/` (`store/`);
//! - Запуск, демон-супервизирование (`setsid`), логирование и `SIGTERM`-остановку нативных бинарников (`supervisor/`).

pub mod manifest;
pub mod store;
pub mod supervisor;

pub use manifest::{PackageKind, PackageManifest};
pub use store::{PackagePaths, PackageStore};
pub use supervisor::RunningTaskInfo;
