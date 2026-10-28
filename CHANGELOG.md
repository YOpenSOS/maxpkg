# Changelog

Все значимые изменения API, ядра `libmax_hal`, пакетного менеджера `maxctl` и языковых SDK (**Rust**, **C99**, **C++17**, **Go**, **Zig**) документируются в этом файле.

Формат основан на [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/), проект следует семантическому версионированию ([SemVer](https://semver.org/lang/ru/)).

> [!WARNING]
> До выхода версии `1.0.0` архитектура ядра `libmax_hal` и контракты SDK считаются **экспериментальными (Unstable)** — между минорными релизами `0.x` возможны архитектурные реврайты и ломающие изменения API.

---

## [0.2.0] - 2026-10-28

### Added
- **Ядерная событийная шина `EventBus` (`modules::events` / `max_events_*`)**:
  - Единый блокирующий мультиплексор `poll(2)` с нулевой нагрузкой на CPU (`0%` в режиме ожидания), объединяющий:
    - `EventKind::Input (3)` — прямое чтение `/dev/input/event0..3` со встроенным конечным автоматом распознавания жестов (`click`, `double_click`, `quadro_click`, `long_press`) и вращения энкодера.
    - `EventKind::BtPower (4)` — мгновенное чтение 8-байтовых бинарных структур ядра `struct rfkill_event` из `/dev/rfkill` (`RFKILL_TYPE_BLUETOOTH`) с автоматической фильтрацией стартовых `RFKILL_OP_ADD` состояний.
    - `EventKind::SoundPlay (7)` — ядерный `inotify` (`IN_OPEN | IN_CLOSE_WRITE`) на `/data/data/com.yandex.io.sdk/files/system/sounds/{common,ru-RU}`, перехватывающий момент открытия любого системного звука демоном `quasmodromd`.
    - `EventKind::AliceState (6)` — ядерный `inotify` (`IN_OPEN`) на каталог `ledpatterns`, определяющий переходы состояний Алисы (`listening`, `speaking`, `thinking`, `bluetooth`, `muted`, `idle`) без чтения `logcat`.
    - `EventKind::BtConfig (5)` — ядерный `inotify` (`IN_CLOSE_WRITE`) на `/data/misc/bluedroid/bt_config.conf` при сопряжении или обновлении профиля Bluetooth-клиента.
    - `EventKind::Tick (2)` — встроенный периодический таймер `timerfd_create(CLOCK_MONOTONIC)` (`set_tick_ms`) для совмещения реактивной обработки событий и рендера кадров `fb4`/`fb5` в одном потоке.
    - `EventKind::Stop (1)` — мгновенное пробуждение `poll(2)` при получении `SIGTERM` / `SIGINT` от супервизора `maxctl`.
- **Транзакционная подмена системных звуков в модуле `audio` (`SoundOverrideManager`)**:
  - Новые методы `override_sound(target_rel, custom_src)` и `restore_sounds()` во всех 5 SDK (`max_audio_override_sound`, `max_audio_restore_sounds`).
  - Автоматическое резервное копирование оригинальных звуков Яндекса в `MAX_PKG_DATA/sound_backups/` при первой подмене.
  - Выставление прав `0644` и системного владельца `u0_a38:u0_a38` (`libc::chown`) для бесшовного чтения процессом `com.yandex.io.sdk`.
  - Автоматическая проверка и самовосстановление подменённых файлов при попытке их перезаписи со стороны OTA (`ensure_all_applied()`) и гарантированный возврат заводских звуков в `Drop` при остановке пакета.
- **Аппаратный DSP Master Mute в модуле `audio`**:
  - Методы `is_muted()` и `set_mute(bool)` (`max_audio_is_muted`, `max_audio_set_mute`), управляющие напрямую регистром `NUMID_MASTER_MUTE = 68` (`ctl 67`) в `/dev/snd/controlC0` за `< 0.2 мс`.
- **Приватная сборка и поставка готовых бинарных артефактов**:
  - Полное удаление локальных путей хост-машины из исходников и бинарных артефактов (`--remap-path-prefix`, `-ffile-prefix-map`, `-trimpath`, `llvm-strip`) с автоматическим аудитом приватности на этапе сборки (`scripts/build_all.ps1`).
  - Скрипт автоматической сборки и публикации релизов `scripts/release.ps1` и GitHub Actions workflow `.github/workflows/release.yml`, формирующие готовые архивы `maxpkg-runtime-aarch64` (для установки на колонку) и `maxpkg-sdk-aarch64` (предсобранные `libmax_hal.a`, `libmax_hal.so` и заголовки для разработки пакетов без компиляции ядра).

### Changed
- **Полный отказ от порождения дочерних процессов (`fork`/`exec`) в модуле `system`**:
  - Чтение и запись системных свойств Android (`get_prop` / `set_prop`) переведены с вызовов `/system/bin/getprop` и `/system/bin/setprop` на прямые FFI-вызовы `__system_property_get` и `__system_property_set` из системной библиотеки Bionic `libc.so`.
  - Получение IPv4-адреса и сетевой статистики `wlan0` (`wifi()`) переведено с вызова `/system/bin/ip` на прямой ядерный вызов `ioctl(SIOCGIFADDR)` и чтение `/sys/class/net/wlan0/statistics/{rx,tx}_bytes`.
- **Перевод всех пакетов (`bt-memes`, `pkg-rust`, `pkg-c`, `pkg-cpp`, `pkg-go`, `pkg-zig`) на `EventBus`**:
  - Убраны циклы поллинга `sleep_ms()` в пользу блокирующего ожидания ядерных событий `next_event(-1)`.

---

## [0.1.0] - 2026-10-28

### Added
- Первый релиз платформы `maxpkg` для **Яндекс Станции Макс (`yandexstation_2`, Android 9 / Linux `4.9.113`, `aarch64`)**:
  - Нативное ядро `libmax_hal` (`crates/hal`) с 10 базовыми модулями: `device`, `canvas` (`fb4` `25x16` + патч памяти `YandexStationFrontalController::isInitialized_`), `ring` (`fb5` `24x1`), `input` (`evdev`), `remote` (`uinput`), `sensors` (`ltr308als01` + `thermal_zone0`), `audio` (`SY6045S` ALSA `ioctl` + `AAudio` + `media_session`), `mic` (Zero-copy 16-канальный буфер `AudioFlinger`), `bluetooth` (`WCN3990`), `system`.
  - Межпроцессный арбитр ресурсов (`flock` + `arbiter.shm`) с 3 уровнями приоритетов (`Background < Normal < Overlay`) и автоматическим теневым сохранением/восстановлением кадров `fb4` и `fb5`.
  - Пакетный менеджер и супервизор процессов `maxctl` (`crates/pkg`, `crates/cli`).
  - Официальные SDK для 5 языков программирования: **Rust**, **C99**, **C++17**, **Go**, **Zig**.
