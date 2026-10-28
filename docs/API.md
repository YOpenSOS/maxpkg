# Документация API `libmax_hal` и языковых SDK (`v0.2.0` — Experimental)

Настоящий документ описывает текущую архитектуру ядра **`libmax_hal`**, протокол межпроцессного арбитра (`arbiter.shm` + `flock`), событийную подсистему **`EventBus`** и справочник функций для всех 5 поддерживаемых языков (**Rust**, **C99**, **C++17**, **Go**, **Zig**).

> [!WARNING]
> **API не зафиксирован (Pre-1.0 / Unstable)**
> Платформа `maxpkg` находится в активной разработке: ещё не все аппаратные и программные подсистемы реализованы до конца, а существующие сигнатуры C-ABI (`max_hal.h`), структура `MaxEvent` и модули SDK могут подвергаться полным переработкам (rewrites) в будущих версиях. При линковке пакетов всегда используйте заголовочные файлы и библиотеки `libmax_hal.a` / `libmax_hal.so` из одного и того же релиза.

---

## Содержание

1. [Инициализация, жизненный цикл и арбитр приоритетов](#1-инициализация-жизненный-цикл-и-арбитр-приоритетов)
2. [Событийная шина `EventBus` (`events`)](#2-событийная-шина-eventbus-events)
3. [Аудиосистема, усилители `SY6045S` и транзакционные звуки (`audio`)](#3-аудиосистема-усилители-sy6045s-и-транзакционные-звуки-audio)
4. [Фронтальный LED-экран `25x16` (`canvas`)](#4-фронтальный-led-экран-25x16-canvas)
5. [Верхнее LED-кольцо `24x1` (`ring`)](#5-верхнее-led-кольцо-24x1-ring)
6. [Кнопки, тумблер Mute и шайба громкости (`input`)](#6-кнопки-тумблер-mute-и-шайба-громкости-input)
7. [Виртуальный пульт управления (`remote`)](#7-виртуальный-пульт-управления-remote)
8. [16-канальная микрофонная матрица (`mic`)](#8-16-канальная-микрофонная-матрица-mic)
9. [Датчики освещённости и температуры SoC (`sensors`)](#9-датчики-освещённости-и-температуры-soc-sensors)
10. [Адаптер Bluetooth `WCN3990` (`bluetooth`)](#10-адаптер-bluetooth-wcn3990-bluetooth)
11. [Системная телеметрия и свойства Android Bionic (`system`)](#11-системная-телеметрия-и-свойства-android-bionic-system)
12. [Сводная таблица символов C-ABI (`max_hal.h`)](#12-сводная-таблица-символов-c-abi-max_halh)

---

## 1. Инициализация, жизненный цикл и арбитр приоритетов

При запуске пакета через `maxctl start <id> --priority <level>` супервизор передаёт уровень приоритета через переменную окружения `MAX_PKG_PRIORITY` и каталог данных пакета через `MAX_PKG_DATA`.

Вызов `max_sdk::init()` / `max_hal_init()` выполняет:
1. Автоопределение модели и ревизии железа (`/proc/bus/input/devices`, `/sys/class/graphics/fb*/name`, `/system/build.prop`).
2. Установку обработчиков сигналов `SIGTERM` и `SIGINT` для гарантированного вызова деструкторов (`Drop` / `close`) и отката оборудования.
3. Подключение к разделяемой памяти межпроцессного арбитра `/data/local/tmp/max-store/locks/arbiter.shm` (`MAP_SHARED`).

### Уровни приоритета (`Priority`)

| Константа C-ABI | Значение | Назначение |
| :--- | :---: | :--- |
| `MAX_PRIORITY_BACKGROUND` | `0` | Фоновые сервисы и хуки (`bt-memes`, телеметрия, мониторинг событий) |
| `MAX_PRIORITY_NORMAL` | `1` | Полноэкранные приложения, визуализаторы, кастомные часы |
| `MAX_PRIORITY_OVERLAY` | `2` | Всплывающие уведомления и алерты поверх работающего `normal`/`background` пакета |

Когда пакет уровня `Overlay` захватывает `canvas` (`fb4`) или `ring` (`fb5`), кадры нижестоящего пакета продолжают прозрачно записываться в теневой буфер `arbiter.shm` и мгновенно возвращаются на физический экран в момент закрытия `Overlay`-пакета.

---

## 2. Событийная шина `EventBus` (`events`)

Шина `EventBus` заменяет циклы опроса (`sleep_ms`) и парсинг `logcat` единым блокирующим системным вызовом `poll(2)`, слушающим одновременно дескрипторы ядра Linux:

- `/dev/input/event0..3` (`evdev` кнопок, тумблера Mute, разъёма AUX и энкодера громкости);
- `/dev/rfkill` (8-байтовые бинарные пакеты ядра `struct rfkill_event` для Bluetooth-радио);
- `inotify` (`IN_OPEN | IN_CLOSE_WRITE`) на системные каталоги звуков `/data/data/com.yandex.io.sdk/files/system/sounds/{common,ru-RU}`, анимаций `ledpatterns` и конфига `/data/misc/bluedroid/bt_config.conf`;
- `timerfd_create(CLOCK_MONOTONIC)` для опционального периодического таймера (`Tick`);
- атомарный флаг сигнала остановки (`SIGTERM` / `SIGINT`).

### Типы событий (`EventKind`)

| C-ABI константа | Rust `EventKind` | Код | `arg0` | `arg1` | `source` | `action` | `detail` |
| :--- | :--- | :---: | :--- | :--- | :--- | :--- | :--- |
| `MAX_EVENT_NONE` | `EventKind::None` | `0` | `0` | `0` | — | — | Таймаут `next_event(timeout_ms)` истёк |
| `MAX_EVENT_STOP` | `EventKind::Stop` | `1` | `0` | `0` | `"signal"` | `"stop"` | Получен `SIGTERM`/`SIGINT` — пакет должен выйти из цикла |
| `MAX_EVENT_TICK` | `EventKind::Tick` | `2` | число срабатываний | интервал (мс) | `"timer"` | `"tick"` | Сработал периодический таймер `set_tick_ms()` |
| `MAX_EVENT_INPUT` | `EventKind::Input` | `3` | код / дельта шайбы | `1` / `0` | `"mic_mute"`, `"alice"`, `"volume"`, `"aux"` | `"down"`, `"up"`, `"click"`, `"double_click"`, `"quadro_click"`, `"long_press"`, `"rotate"` | Аппаратный ввод или жест |
| `MAX_EVENT_BT_POWER` | `EventKind::BtPower` | `4` | `1` (ON) / `0` (OFF) | `rfkill idx` | `"bluetooth"` | `"power_on"` / `"power_off"` | Изменение питания Bluetooth через `/dev/rfkill` |
| `MAX_EVENT_BT_CONFIG`| `EventKind::BtConfig`| `5` | число устройств | `1` (active) | `"bluetooth"` | `"config_changed"` | Name и MAC последнего сопряжённого клиента (`Name [AA:BB:..]`) |
| `MAX_EVENT_ALICE_STATE`| `EventKind::AliceState`| `6` | `0` | `0` | `"alice"` | `"listening"`, `"speaking"`, `"thinking"`, `"bluetooth"`, `"muted"`, `"idle"` | Имя открытого `.led`-паттерна (напр. `listening.led`) |
| `MAX_EVENT_SOUND_PLAY`| `EventKind::SoundPlay`| `7` | `0` | `0` | `"audio"` | `"play"` | Имя файла, открытого `quasmodromd` (напр. `bluetooth_connected.opus`) |

### Структура `MaxEvent` (C-ABI `#[repr(C)]`)

```c
typedef struct {
    int32_t  kind;        /* MAX_EVENT_* (0..7) */
    int32_t  arg0;        /* Основной числовой аргумент */
    int32_t  arg1;        /* Дополнительный числовой аргумент */
    uint64_t time_ms;     /* Монотонное время события (мс) */
    char     source[32];  /* Источник ("mic_mute", "bluetooth", "alice", "audio", ...) */
    char     action[32];  /* Действие ("quadro_click", "power_on", "play", ...) */
    char     detail[128]; /* Детали (имя звукового файла, MAC-адрес и т.д.) */
} MaxEvent;
```

---

## 3. Аудиосистема, усилители `SY6045S` и транзакционные звуки (`audio`)

Модуль `audio` управляет цифровыми усилителями `SY6045S` напрямую через `ioctl(SNDRV_CTL_IOCTL_ELEM_READ/WRITE)` на `/dev/snd/controlC0`, системными звуками `com.yandex.io.sdk` и аппаратным воспроизведением через `libaaudio.so`.

### Ключевые возможности
1. **Аппаратный DSP Master Mute (`ctl 67` / `NUMID_MASTER_MUTE = 68`)**:
   - `audio.is_muted()` / `max_audio_is_muted(handle)` — чтение состояния аппаратного мьюта.
   - `audio.set_mute(true/false)` / `max_audio_set_mute(handle, muted)` — мгновенное (`< 0.2 мс`) отключение/включение звука на уровне регистра усилителя.
2. **Транзакционная подмена системных звуков (`SoundOverrideManager`)**:
   - `audio.override_sound(target_rel, custom_src)` / `max_audio_override_sound(handle, target_rel, custom_src)`:
     - Сохраняет заводской файл `/data/data/com.yandex.io.sdk/files/system/sounds/<target_rel>` в `MAX_PKG_DATA/sound_backups/`.
     - Копирует `custom_src` на место системного звука, выставляет права `0644` и владельца `u0_a38:u0_a38` (`chown`).
   - `audio.restore_sounds()` / `max_audio_restore_sounds(handle)`:
     - Мгновенно возвращает все оригинальные заводские звуки из резервной копии (вызывается **автоматически** в `Drop` при закрытии драйвера или остановке пакета через `maxctl stop`).
3. **Управление трактом `SY6045S`**:
   - Громкость НЧ-вуфера (`Woofer 0..255`), ВЧ-твитеров (`Tweeters 0..255`) и линейного выхода `AUX` (`0..255`), аппаратный 15-полосный эквалайзер (`EQ`) и компрессор (`DRC`).
4. **Воспроизведение и синтез (`AAudio`)**:
   - `tone(freq_hz, duration_ms, volume)` — генерация синусоидального тона через `libaaudio.so`.
   - `play_wav(path)` / `play(path)` — воспроизведение аудиофайлов.

---

## 4. Фронтальный LED-экран `25x16` (`canvas`)

- Узел ядра: `/dev/graphics/fb4` (`mmap`, `25x16` пикселей, `RGB888`, `1200` байт).
- При открытии `Fb4CanvasDriver` автоматически находит в `/proc/<pid>/maps` и `/proc/<pid>/mem` процесса `com.yandex.io.sdk` экземпляр класса `YandexStationFrontalController` по сигнатуре `{ fb4_mmap_addr, height=16, width=25 }` и ставит его на паузу, записывая `0` в поле `isInitialized_`.
- При закрытии драйвера (`Drop` / `max_canvas_close`) байт `isInitialized_` восстанавливается в `1`, а штатные часы Яндекса продолжают работу без перезапуска `quasmodromd`.

---

## 5. Верхнее LED-кольцо `24x1` (`ring`)

- Узел ядра: `/dev/graphics/fb5` (`mmap`, `24` RGB-светодиода, `72` байта).
- Поддерживает индивидуальную адресацию каждого из 24 светодиодов (`set_pixel(idx, r, g, b)`), полную заливку (`fill(r, g, b)`), очистку (`off()`) и цветовые пространства `HSV` / `HEX`.

---

## 6. Кнопки, тумблер Mute и шайба громкости (`input`)

- Узлы ядра: `/dev/input/event0` (`gpio_keys`), `event1` (`adc_keys`), `event2` (`rotary-volume`), `event3` (`tas5805m` / AUX).
- Поддерживает эксклюзивный перехват `ioctl(EVIOCGRAB)` (`set_grab(true/false)`) и изменение чувствительности энкодера громкости `/sys/devices/platform/rotary-volume/event_step` (`1..10`) с авто-возвратом при выходе.

---

## 7. Виртуальный пульт управления (`remote`)

- Узел ядра: `/dev/uinput` (`MaxPlatform Virtual Remote`).
- Создаёт виртуальное устройство ввода в ядре Linux для управления воспроизведением и навигацией (`play_pause`, `stop`, `next`, `prev`, `vol_up`, `vol_down`, `mute`, `up`, `down`, `left`, `right`, `enter`, `back`, `home`).

---

## 8. 16-канальная микрофонная матрица (`mic`)

- Подключается напрямую (`Zero-copy mmap`) к кольцевому буферу `AudioFlinger` (`anon_inode:dmabuf` / `/dev/ashmem`) через дескрипторы `/proc/<audioflinger_pid>/fd/*`.
- Возвращает мгновенные пиковые амплитуды всех 16 каналов (`0..6` — 7 физических микрофонов верхней крышки, `7..15` — внутренние AEC-референсы) и аппаратный статус отключения микрофонов.

---

## 9. Датчики освещённости и температуры SoC (`sensors`)

- Люксметр `ltr308als01` (`/sys/bus/iio/devices/iio:device0`): чтение сырого потока (`raw`), освещённости в люксах (`lux`), настройка аппаратного усиления (`1x`, `3x`, `6x`, `9x`, `18x`) и времени интеграции.
- Температура кристалла Amlogic S905X2 (`/sys/class/thermal/thermal_zone0/temp`) в градусах Цельсия.

---

## 10. Адаптер Bluetooth `WCN3990` (`bluetooth`)

- Читает конфигурацию `/data/misc/bluedroid/bt_config.conf` и состояние `/sys/class/bluetooth/hci0`.
- Возвращает имя и MAC-адрес колонки, список всех сопряжённых устройств (`Name`, `MAC`, `Class of Device`, `Last Connected`) и позволяет управлять питанием Bluetooth с автоматическим откатом в `Drop`.

---

## 11. Системная телеметрия и свойства Android Bionic (`system`)

Работает **без создания дочерних процессов (`fork`/`exec`)**:
- `get_prop(key)` и `set_prop(key, val)` вызывают напрямую функции `__system_property_get` и `__system_property_set` из системной библиотеки Android Bionic `libc.so`.
- `wifi()` получает IPv4-адрес интерфейса `wlan0` через ядерный `ioctl(SIOCGIFADDR)`, статистику трафика из `/sys/class/net/wlan0/statistics/{rx,tx}_bytes` и параметры сигнала (`SSID`, `BSSID`, `RSSI`, `Link Speed`) через сокет `wpa_supplicant`.
- `clock()`, `uptime()`, `memory()`, `cpu()`, `storage()`, `find_pid(name)` читают данные напрямую из `/proc` и `statvfs(2)`.

---

## 12. Сводная таблица символов C-ABI (`max_hal.h`)

| Группа | Экспортируемые функции C-ABI (`libmax_hal.so` / `libmax_hal.a`) |
| :--- | :--- |
| **Core & Lifecycle** | `max_hal_init`, `max_hal_priority`, `max_hal_should_stop`, `max_hal_sleep_ms`, `max_hal_device_model`, `max_hal_device_board_rev` |
| **EventBus (`events`)** | `max_events_open`, `max_events_close`, `max_events_set_tick_ms`, `max_events_set_input_grab`, `max_events_next` |
| **Canvas (`fb4`)** | `max_canvas_open`, `max_canvas_close`, `max_canvas_clear`, `max_canvas_fill`, `max_canvas_set_pixel`, `max_canvas_draw_frame`, `max_canvas_flush`, `max_canvas_get_brightness`, `max_canvas_set_brightness` |
| **Ring (`fb5`)** | `max_ring_open`, `max_ring_close`, `max_ring_set_pixel`, `max_ring_fill`, `max_ring_off` |
| **Input (`evdev`)** | `max_input_open`, `max_input_close`, `max_input_set_grab`, `max_input_get_wheel_step`, `max_input_set_wheel_step`, `max_input_next_event` |
| **Remote (`uinput`)** | `max_remote_open`, `max_remote_close`, `max_remote_tap`, `max_remote_action` |
| **Sensors (`iio`)** | `max_sensors_open`, `max_sensors_close`, `max_sensors_read_als`, `max_sensors_set_als_gain`, `max_sensors_set_als_integration_time`, `max_sensors_read_thermal` |
| **Audio (`SY6045S` / `AAudio`)** | `max_audio_open`, `max_audio_close`, `max_audio_read_amp`, `max_audio_set_amp`, `max_audio_is_muted`, `max_audio_set_mute`, `max_audio_override_sound`, `max_audio_restore_sounds`, `max_audio_tone`, `max_audio_play_wav`, `max_audio_read_media` |
| **Microphone (`shm`)** | `max_mic_open`, `max_mic_close`, `max_mic_read` |
| **Bluetooth (`hci0`)** | `max_bt_open`, `max_bt_close`, `max_bt_status`, `max_bt_device_count`, `max_bt_device_get`, `max_bt_set_power` |
| **System (`proc` / `bionic`)** | `max_sys_clock`, `max_sys_uptime`, `max_sys_memory`, `max_sys_cpu`, `max_sys_storage`, `max_sys_wifi`, `max_sys_find_pid`, `max_sys_get_prop`, `max_sys_set_prop` |
