# maxpkg (`v0.2.0` — Pre-Release / Unstable API)

Нативный пакетный менеджер (`maxctl`), межпроцессный арбитр аппаратных ресурсов (`arbiter`), событийное ядро Hardware Abstraction Layer (`libmax_hal`) и мульти-языковой SDK (**Rust**, **C99**, **C++17**, **Go**, **Zig**) для **Яндекс Станции Макс** (`yandexstation_2`, Android 9 / Linux `4.9.113`, `aarch64`).

> [!WARNING]
> **Статус архитектуры и API: В активной разработке (Experimental / Unstable)**
> Текущая архитектура ядра `libmax_hal`, шины событий `EventBus`, C-ABI интерфейсов и формата пакетов `maxctl` **ещё не зафиксирована и не является финальной**. Часть подсистем находится в процессе исследования и реализации, поэтому в следующих релизах возможны полные переработки (rewrites) отдельных модулей и ломающие изменения API (breaking changes) без сохранения обратной совместимости. При сборке сторонних пакетов ориентируйтесь на конкретную версию SDK из соответствующего релиза.

- Подробный справочник текущего API и C-ABI символов: **[`docs/API.md`](./docs/API.md)**
- История изменений по версиям: **[`CHANGELOG.md`](./CHANGELOG.md)**
- Готовые бинарники `maxctl`, `libmax_hal.so`, `libmax_hal.a` и SDK-архивы: **[GitHub Releases](https://github.com/YOpenSOS/maxpkg/releases)**

---

## Ключевые архитектурные принципы

1. **Только нативный код (`aarch64-linux-android`), никаких виртуальных машин и форков шелла**
   Каждое приложение или фоновый сервис компилируется в нативный ELF-бинарник на **Rust**, **C**, **C++**, **Go** или **Zig** и работает напрямую с единым C-ABI ядром `libmax_hal` (`libmax_hal.a` / `libmax_hal.so`). Никаких вызовов `logcat`, `getprop`, `setprop`, `ip` или `tinymix2` — все операции идут через `ioctl`, `mmap`, `inotify`, `/dev/rfkill` и прямые FFI-вызовы Android Bionic `libc.so`.
2. **Event-Driven ядро (`EventBus` / `poll(2)` / `0% CPU` в ожидании)**
   Вместо поллинга и таймеров `sleep()` пакеты подписываются на единую ядерную шину `EventBus`, мультиплексирующую `/dev/input/event0..3` (с аппаратным распознаванием `double_click`, `quadro_click`, `long_press`), `/dev/rfkill` (питание Bluetooth), `inotify` (`IN_OPEN | IN_CLOSE_WRITE` на системные звуки и LED-анимации Алисы), `timerfd` и сигналы `SIGTERM`/`SIGINT`.
3. **Zero-Open и Zero-Alloc на горячем пути**
   Ядерные дескрипторы (`/dev/graphics/fb4`, `/dev/graphics/fb5`, `/dev/input/event*`, `/dev/uinput`, `/dev/snd/controlC0`, `/dev/ashmem`) открываются и отображаются в память (`mmap`) ровно один раз при инициализации драйвера. Отрисовка кадров и чтение 16-канального микрофонного буфера выполняются без аллокаций в куче и без повторных `open()`/`write()`.
4. **Межпроцессный арбитр и слои приоритетов (`Background < Normal < Overlay`)**
   Доступ к эксклюзивным аппаратным ресурсам (`canvas`, `ring`, `input`, `amp`) координируется через ядерные блокировки `flock` (`store/locks/<resource>.<priority>.lock`) и разделяемую память `arbiter.shm` (`MAP_SHARED`):
   - Фоновый пакет (`background` или `normal`) и уведомление (`overlay`) могут работать одновременно: пока активен `overlay`, кадры нижнего пакета прозрачно сохраняются в теневой буфер `arbiter.shm`.
   - Как только `overlay`-пакет завершается, экран `fb4` (включая яркость ШИМ) и кольцо `fb5` мгновенно восстанавливают последний кадр нижнего пакета.
5. **Гарантированный транзакционный откат железа и файлов (`RAII / Drop` + `SIGTERM`)**
   При остановке пакета через `maxctl stop <id>` супервизор посылает `SIGTERM`, `EventBus` возвращает событие `EventKind::Stop`, а деструкторы драйверов автоматически возвращают колонку к заводскому состоянию:
   - снимается пауза со штатных часов `com.yandex.io.sdk` (восстановление `YandexStationFrontalController::isInitialized_` в памяти процесса) и возвращаются исходный кадр `fb4` и яркость `/sys/class/backlight/fb4/brightness`;
   - восстанавливается кадр кольца `fb5`;
   - из резервной копии `sound_backups/` атомарно возвращаются все заводские системные звуки Яндекса, подменённые через `override_sound()`;
   - снимается перехват кнопок `EVIOCGRAB` и возвращается заводской шаг энкодера `/sys/devices/platform/rotary-volume/event_step`;
   - уничтожается виртуальный пульт `/dev/uinput` (`UI_DEV_DESTROY`);
   - откатываются регистры цифровых усилителей `SY6045S` (`Woofer`, `Tweeters`, `Headphone DAC`, `EQ`, `DRC`, `Master Mute`) и настройки датчика освещённости `ltr308als01`.

---

## Структура репозитория

```text
max-platform/
├── crates/
│   ├── hal/                  # Ядро HAL (max-hal / libmax_hal), EventBus, арбитр SHM и C-ABI (ffi/)
│   ├── pkg/                  # Менеджер манифестов package.toml, хранилища store/ и супервизор процессов
│   └── cli/                  # Консольная утилита maxctl
├── sdk/
│   ├── rust/                 # Rust SDK (крейт max-sdk)
│   ├── c/                    # C99/C11 заголовок (max_hal.h)
│   ├── cpp/                  # C++17 RAII SDK (max_hal.hpp)
│   ├── go/                   # Go SDK на базе cgo (пакет maxhal)
│   └── zig/                  # Zig 0.13 SDK (модуль maxhal.zig)
├── examples/
│   ├── bt-memes/             # Фоновый Event-Driven плагин кастомизации звуков и жестов Bluetooth
│   ├── pkg-rust/             # Эталонный Event-Driven пакет на Rust
│   ├── pkg-c/                # Эталонный Event-Driven пакет на C99
│   ├── pkg-cpp/              # Эталонный Event-Driven пакет на C++17 (RAII)
│   ├── pkg-go/               # Эталонный Event-Driven пакет на Go (Overlay)
│   └── pkg-zig/              # Эталонный Event-Driven пакет на Zig 0.13
├── docs/
│   └── API.md                # Полный технический справочник модулей и C-ABI символов
├── scripts/
│   ├── build_all.ps1         # Воспроизводимая кросс-компиляция со скрытием путей и аудитом приватности
│   └── release.ps1           # Скрипт упаковки релизных архивов (runtime + SDK) и публикации в GitHub Releases
├── .github/workflows/
│   └── release.yml           # CI/CD пайплайн автоматической сборки релизов по git-тегам v*
└── CHANGELOG.md              # История изменений API и релизов
```

---

## Аппаратные и системные модули (`libmax_hal`)

Все 11 модулей ядра доступны с идентичным функционалом во всех 5 языковых SDK (подробно в [`docs/API.md`](./docs/API.md)):

| Модуль | Ядерный / системный интерфейс на Станции Макс | Возможности |
| :--- | :--- | :--- |
| **`device`** | `/proc/bus/input/devices`, `/sys/class/graphics/fb*/name`, `/system/build.prop` | Автоопределение модели (`yandexstation_2`), ревизии железа и путей к узлам `evdev`/`fbdev` |
| **`events`** | Единая шина `poll(2)` / `evdev` + `/dev/rfkill` + `inotify` (`IN_OPEN`) + `timerfd` + `SIGTERM` | Event-Driven мультиплексор: кнопки (`click`, `double_click`, `quadro_click`, `long_press`), шайба, питание Bluetooth (`BtPower`), конфиг Bluetooth (`BtConfig`), состояния Алисы (`AliceState`), открытие системных звуков (`SoundPlay`), таймер (`Tick`) и остановка (`Stop`) с `0%` CPU в ожидании |
| **`canvas`** | `/dev/graphics/fb4` (`mmap`, `25x16`, `RGB888`), `/sys/class/backlight/fb4/brightness` | Отрисовка пикселей и полных кадров `25x16`, ШИМ-яркость `0..200`, точечное отключение рендера штатных часов в памяти `com.yandex.io.sdk` (`YandexStationFrontalController::isInitialized_`) |
| **`ring`** | `/dev/graphics/fb5` (`mmap`, `24x1`, `RGB888`, `72` байта) | Управление 24 адресными RGB-светодиодами верхней шайбы (`set_pixel`, `fill`, `off`, HSV/HEX) |
| **`input`** | `/dev/input/event0` (`gpio_keypad`), `event2` (`rotary-volume`), `event3` (`Headphones`) | Чтение кнопки Алисы, тумблера Mute, разъёма AUX, вращения шайбы, перехват `EVIOCGRAB`, шаг энкодера `1..10` |
| **`remote`** | `/dev/uinput` (`UI_DEV_CREATE` / `UI_DEV_DESTROY`) | Виртуальный пульт управления медиаплеерами и ТВ-интерфейсом (`play_pause`, `next`, `prev`, стрелки, `enter`, `back`) |
| **`sensors`** | `/sys/bus/iio/devices/iio:device0` (`ltr308als01`), `/sys/class/thermal/thermal_zone0/temp` | Люксметр (`raw`, `lux`), аппаратное усиление (`1x`/`3x`/`6x`/`9x`/`18x`), время интеграции, температура SoC Amlogic S905X2 |
| **`audio`** | `/dev/snd/controlC0` (`SNDRV_CTL_IOCTL_ELEM_READ/WRITE`), `libaaudio.so`, `media_session` | Усилители `SY6045S` (`Woofer`, `Tweeters`, `AUX`, `EQ`, `DRC`, `Master Mute` `ctl 67`), транзакционная подмена системных звуков `override_sound` с авто-бэкапом и откатом в `Drop`, синтез/WAV через `AAudio`, метаданные трека |
| **`mic`** | `/proc/<audioflinger>/maps` + `/proc/<pid>/fd/*` (`mmap` `anon_inode:dmabuf` / `ashmem`) | Zero-copy чтение 16-канального микрофонного буфера `AudioFlinger` (7 физических микрофонов + AEC-референсы): пики каналов и статус аппаратного тумблера Mute |
| **`bluetooth`** | `/data/misc/bluedroid/bt_config.conf`, `/sys/class/bluetooth/hci0` | Статус адаптера `WCN3990`, имя/MAC колонки, список всех сопряжённых клиентов, управление питанием с авто-откатом в `Drop` |
| **`system`** | `/proc/uptime`, `/proc/meminfo`, `/proc/loadavg`, `statvfs`, `ioctl(SIOCGIFADDR)`, `__system_property_get/set` | Локальное время, аптайм, ОЗУ, CPU LoadAvg, статистика раздела `/data`, Wi-Fi (`SSID`, `BSSID`, `RSSI`, IP, трафик), поиск процессов, нативные свойства Android Bionic |

---

## Быстрая установка из готового релиза (без компиляции)

В каждом релизе на странице **[Releases](https://github.com/YOpenSOS/maxpkg/releases)** публикуются готовые артефакты под `aarch64-linux-android`:

1. **`maxpkg-runtime-aarch64-vX.Y.Z.tar.gz`** — готовый комплект для установки на Яндекс Станцию Макс (`bin/maxctl`, `lib/libmax_hal.so`, скрипт `install-on-station.sh` и скомпилированные пакеты `bt-memes`, `pkg-rust`, `pkg-c`, `pkg-cpp`, `pkg-go`, `pkg-zig`).
2. **`maxpkg-sdk-aarch64-vX.Y.Z.zip`** — комплект разработчика со статической и динамической библиотеками (`lib/libmax_hal.a`, `lib/libmax_hal.so`) и заголовками/модулями для **C99**, **C++17**, **Go**, **Zig** и **Rust**, позволяющий собирать собственные плагины без компиляции ядра.

Развёртывание на колонке из архива релиза:

```bash
tar -xzf maxpkg-runtime-aarch64-v0.2.0.tar.gz -C /data/local/tmp/
sh /data/local/tmp/maxpkg-runtime/install-on-station.sh
```

---

## Управление пакетами (`maxctl`)

Каждый пакет представляет собой директорию с манифестом `package.toml` и скомпилированным `aarch64` бинарником:

```toml
id = "bt-memes"
name = "Bluetooth Meme Sounds & Gestures"
version = "1.0.1"
kind = "binary"
lang = "rust" # rust | c | cpp | go | zig
entry = "bt-memes"
priority = "background" # background | normal | overlay

[compat]
models = ["yandexstation_2"]
features = ["events", "audio", "bluetooth"]
```

### Основные команды CLI

```bash
# Паспорт колонки, найденные узлы ядра и таблица аппаратных модулей
maxctl info

# Установка пакета в хранилище /data/local/tmp/max-store/pkgs/<id>
maxctl install ./bt-memes

# Список установленных пакетов
maxctl list

# Запуск фонового плагина
maxctl start bt-memes --priority background

# Запуск полноэкранного приложения
maxctl start pkg-rust --priority normal

# Запуск поверх работающего пакета во временном слое Overlay
maxctl start pkg-go --priority overlay

# Просмотр активных пакетов и занятых блокировок железа (flock + arbiter.shm)
maxctl ps

# Чтение логов пакета
maxctl logs bt-memes -n 50

# Штатная остановка пакета (SIGTERM -> EventKind::Stop -> автоматический откат железа и звуков)
maxctl stop bt-memes
```

---

## Примеры Event-Driven пакетов на 5 языках

### 1. Rust (`sdk/rust`)

```rust
use max_sdk::prelude::*;

fn main() -> anyhow::Result<()> {
    let dev = max_sdk::init();
    let mut canvas = Fb4CanvasDriver::open_for_device(&dev)?;
    let mut audio = AudioDriver::open_for_device(&dev)?;

    // Транзакционная подмена системного звука (автоматически откатится при выходе в Drop)
    audio.override_sound("common/bluetooth_connected.opus", "./sounds/custom.opus")?;

    let mut events = EventBus::open(&dev)?;
    events.set_tick_ms(100)?; // Опциональный таймер 10 FPS для анимации

    while let Some(ev) = events.next_event(-1) {
        match ev.kind() {
            EventKind::Stop => break,
            EventKind::Tick => {
                canvas.clear_buf();
                canvas.set_pixel(12, 8, 255);
                canvas.flush()?;
            }
            EventKind::Input if ev.source_str() == "mic_mute" && ev.action_str() == "quadro_click" => {
                println!("4x click detected on Mute button!");
            }
            EventKind::BtPower => {
                println!("Bluetooth power changed: {}", ev.arg0 == 1);
            }
            _ => {}
        }
    }
    Ok(()) // Drop автоматически возвращает экран часам Яндекса и восстанавливает заводские звуки
}
```

### 2. C99 (`sdk/c/max_hal.h`)

```c
#include "max_hal.h"

int main(void) {
    max_hal_init();
    MaxRingHandle *ring = max_ring_open();
    MaxEventsHandle *events = max_events_open();
    max_events_set_tick_ms(events, 100);

    MaxEvent ev;
    while (max_events_next(events, -1, &ev) == 1) {
        if (ev.kind == MAX_EVENT_STOP) break;
        if (ev.kind == MAX_EVENT_TICK) {
            max_ring_fill(ring, 0, 180, 255);
        }
    }

    max_events_close(events);
    max_ring_close(ring);
    return 0;
}
```

### 3. C++17 RAII (`sdk/cpp/max_hal.hpp`)

```cpp
#include "max_hal.hpp"

int main() {
    maxhal::init();
    maxhal::Audio audio;
    maxhal::EventBus events;
    events.set_tick_ms(100);

    MaxEvent ev{};
    while (events.next_event(-1, ev)) {
        if (ev.kind == MAX_EVENT_STOP) break;
        if (ev.kind == MAX_EVENT_BT_POWER) {
            audio.set_mute(false);
        }
    }
    return 0; // Деструкторы ~EventBus() и ~Audio() автоматически откатывают ресурсы
}
```

### 4. Go (`sdk/go/maxhal`)

```go
package main

import "max-platform/sdk/go/maxhal"

func main() {
    dev := maxhal.Init()
    canvas, _ := dev.OpenCanvas()
    defer canvas.Close()

    events, _ := dev.OpenEventBus()
    defer events.Close()
    _ = events.SetTickMs(100)

    for {
        ev, ok := events.NextEvent(-1)
        if !ok || ev.Kind == maxhal.EventStop {
            break
        }
        if ev.Kind == maxhal.EventTick {
            _ = canvas.Clear()
            _ = canvas.SetPixel(12, 8, 255)
            _ = canvas.Flush()
        }
    }
}
```

### 5. Zig (`sdk/zig/maxhal.zig`)

```zig
const maxhal = @import("maxhal.zig");

pub fn run() !void {
    maxhal.init();
    var ring = try maxhal.Ring.open();
    defer ring.close();

    var events = try maxhal.EventBus.open();
    defer events.close();
    try events.setTickMs(100);

    while (events.next(-1)) |ev| {
        switch (ev.kind()) {
            .stop => break,
            .tick => try ring.fill(100, 200, 50),
            else => {},
        }
    }
}

comptime {
    maxhal.exportMain(run);
}
```

---

## Сборка из исходников и создание релизов

Для кросс-компиляции под `aarch64-linux-android` требуются:
- **Rust** (`rustup target add aarch64-linux-android`)
- **Android NDK r26+** (переменная окружения `ANDROID_NDK_HOME` или локальный файл `.env.local`)
- **Go 1.22+** (для сборки пакетов на Go через `cgo`)
- **Zig 0.13+** (переменная окружения `ZIG_HOME` или `zig` в `PATH`)

### Сборка бинарников с проверкой приватности

Скрипт `scripts/build_all.ps1` автоматически вырезает все локальные пути хост-машины (`--remap-path-prefix`, `-ffile-prefix-map`, `-trimpath`, `llvm-strip`) и проверяет скомпилированные артефакты на отсутствие утечек путей:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build_all.ps1
```

### Выпуск нового релиза

Для обновления версии API, автоматической сборки архивов `maxpkg-runtime-aarch64` + `maxpkg-sdk-aarch64`, генерации `SHA256SUMS.txt` и публикации релиза на GitHub:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/release.ps1 -Version 0.2.0 -Publish
```

Также настроен GitHub Actions пайплайн (`.github/workflows/release.yml`), автоматически собирающий и публикующий релиз при отправке тега `v*.*.*`.

---

## Лицензия и указание авторства (Attribution)

Проект распространяется под лицензией **Apache License, Version 2.0** (см. [`LICENSE`](./LICENSE) и [`NOTICE`](./NOTICE)).

В соответствии с **разделом 4(d) лицензии Apache 2.0**, при создании форков, модификаций, производных платформ или сторонних пакетов на базе `maxpkg` / `libmax_hal` необходимо сохранять файл [`NOTICE`](./NOTICE) и явно указывать ссылку на оригинальный репозиторий (**[YOpenSOS/maxpkg](https://github.com/YOpenSOS/maxpkg)**) в документации или `README`.
