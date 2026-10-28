/*
 * max_hal.h — Официальный C99/C11 SDK для `max-platform` (`libmax_hal.so` / `libmax_hal.a`).
 * Поддерживает C, C++, Go (cgo) и Zig (@cImport).
 */

#ifndef MAX_HAL_H
#define MAX_HAL_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Версия ABI и SDK `libmax_hal` */
#define MAX_HAL_VERSION_MAJOR  0
#define MAX_HAL_VERSION_MINOR  2
#define MAX_HAL_VERSION_PATCH  0
#define MAX_HAL_VERSION_STRING "0.2.0"

/* Уровни приоритета арбитра ресурсов (`flock` + `arbiter.shm`) */
#define MAX_PRIORITY_BACKGROUND 0
#define MAX_PRIORITY_NORMAL     1
#define MAX_PRIORITY_OVERLAY    2

/* Непрозрачные дескрипторы аппаратных драйверов */
typedef struct MaxEventsHandle  MaxEventsHandle;
typedef struct MaxCanvasHandle  MaxCanvasHandle;
typedef struct MaxRingHandle    MaxRingHandle;
typedef struct MaxInputHandle   MaxInputHandle;
typedef struct MaxRemoteHandle  MaxRemoteHandle;
typedef struct MaxSensorsHandle MaxSensorsHandle;
typedef struct MaxAudioHandle   MaxAudioHandle;
typedef struct MaxMicHandle     MaxMicHandle;
typedef struct MaxBtHandle      MaxBtHandle;

/* 1. Паспорт устройства (`device`) */
typedef struct MaxDeviceInfo {
    char     model[32];
    char     name[64];
    char     sku[32];
    char     serial[32];
    char     os[16];
    char     arch[16];
    char     kernel[32];
    char     firmware[32];
    uint8_t  led_count;
    uint16_t screen_w;
    uint16_t screen_h;
    uint8_t  mic_count;
    uint8_t  has_encoder;
    uint8_t  has_hdmi;
} MaxDeviceInfo;

/* Категории событий единой шины `EventBus` (`MaxEvent.kind`) */
#define MAX_EVENT_NONE        0
#define MAX_EVENT_STOP        1
#define MAX_EVENT_TICK        2
#define MAX_EVENT_INPUT       3
#define MAX_EVENT_BT_POWER    4
#define MAX_EVENT_BT_CONFIG   5
#define MAX_EVENT_ALICE_STATE 6
#define MAX_EVENT_SOUND_PLAY  7

/* Универсальное событие ядра (`events`) */
typedef struct MaxEvent {
    uint32_t kind;
    int32_t  code;
    int32_t  delta;
    uint64_t held_ms;
    uint64_t timestamp_ms;
    char     source[32];
    char     name[64];
} MaxEvent;

/* 2. Событие ввода (`input`) */
typedef struct MaxInputEvent {
    char     source[16];
    char     action[16];
    int32_t  delta;
    uint64_t held_ms;
    uint16_t code;
    int32_t  value;
    uint64_t time_ms;
} MaxInputEvent;

/* 3. Датчик освещённости (`sensors.light`) */
typedef struct MaxLightReading {
    float    lux;
    uint32_t raw;
    uint8_t  gain;
    uint16_t integration_ms;
} MaxLightReading;

/* 4. Датчики температуры (`sensors.thermal`) */
typedef struct MaxThermalReading {
    float soc_c;
    float ddr_c;
    float wifi_c;
} MaxThermalReading;

/* 5. Статус усилителей SY6045S (`audio.amp`) */
typedef struct MaxAmpStatus {
    uint8_t woofer;
    uint8_t tweeters;
    uint8_t aux;
    uint8_t woofer_on;
    uint8_t tweeters_on;
    uint8_t eq;
    uint8_t drc;
} MaxAmpStatus;

/* 6. Текущий трек (`audio.track`) */
typedef struct MaxTrackInfo {
    uint8_t active;
    uint8_t playing;
    uint8_t volume;
    uint8_t _pad;
    int64_t position_ms;
    int64_t duration_ms;
    char    package[64];
    char    title[128];
    char    artist[128];
    char    album[128];
} MaxTrackInfo;

/* 7. Параметры 16-канального микрофонного буфера AudioFlinger (`mic`) */
typedef struct MaxMicInfo {
    uint32_t rate;
    uint8_t  channels;
    uint8_t  _pad[3];
    uint32_t buffer_frames;
    uint32_t pid;
    uint32_t rear;
} MaxMicInfo;

/* 8. Статус Bluetooth (`bluetooth`) */
typedef struct MaxBtStatus {
    uint8_t  enabled;
    uint8_t  connected;
    uint8_t  scan_mode;
    uint8_t  _pad;
    uint32_t bonded_count;
    char     address[24];
    char     name[64];
} MaxBtStatus;

/* 9. Системная телеметрия (`system`) */
typedef struct MaxSysInfo {
    uint64_t unix_sec;
    double   uptime_sec;
    uint64_t mem_total_kb;
    uint64_t mem_avail_kb;
    float    load1;
    int32_t  wifi_rssi_dbm;
    uint64_t disk_free_mb;
    uint8_t  wifi_connected;
    uint8_t  _pad[7];
    char     wifi_ip[32];
    char     wifi_ssid[64];
} MaxSysInfo;

/* === Жизненный цикл и ошибки === */
const char* max_hal_last_error(void);
int32_t     max_hal_init(const char* pkg_id, uint8_t priority, uint8_t replace);
int32_t     max_hal_should_stop(void);
int32_t     max_hal_sleep_ms(uint32_t ms);
int32_t     max_device_info(MaxDeviceInfo* out);

/* === Модуль Canvas (экран 25x16, /dev/graphics/fb4) === */
MaxCanvasHandle* max_canvas_open(void);
void             max_canvas_close(MaxCanvasHandle* h);
int32_t          max_canvas_clear(MaxCanvasHandle* h);
int32_t          max_canvas_set_pixel(MaxCanvasHandle* h, uint32_t x, uint32_t y, uint8_t br);
int32_t          max_canvas_set_frame(MaxCanvasHandle* h, const uint8_t* pixels400, size_t len);
int32_t          max_canvas_get_brightness(MaxCanvasHandle* h);
int32_t          max_canvas_set_brightness(MaxCanvasHandle* h, uint8_t br);
int32_t          max_canvas_flush(MaxCanvasHandle* h);

/* === Модуль Ring (24 RGB LED, /dev/graphics/fb5) === */
MaxRingHandle* max_ring_open(void);
void           max_ring_close(MaxRingHandle* h);
int32_t        max_ring_clear(MaxRingHandle* h);
int32_t        max_ring_set_pixel(MaxRingHandle* h, uint32_t idx, uint8_t r, uint8_t g, uint8_t b);
int32_t        max_ring_fill(MaxRingHandle* h, uint8_t r, uint8_t g, uint8_t b);
int32_t        max_ring_flush(MaxRingHandle* h);
int32_t        max_ring_off(MaxRingHandle* h);

/* === Модуль Events (Единая ядерная шина событий: evdev + rfkill + inotify + timerfd + SIGTERM) === */
MaxEventsHandle* max_events_open(void);
void             max_events_close(MaxEventsHandle* h);
int32_t          max_events_set_tick_ms(MaxEventsHandle* h, uint32_t period_ms);
int32_t          max_events_set_input_grab(MaxEventsHandle* h, uint8_t grab);
int32_t          max_events_next(MaxEventsHandle* h, int32_t timeout_ms, MaxEvent* out);

/* === Модуль Input (кнопки, крутилка, тумблер микрофона, AUX) === */
MaxInputHandle* max_input_open(void);
void            max_input_close(MaxInputHandle* h);
int32_t         max_input_is_mic_muted(MaxInputHandle* h);
int32_t         max_input_is_aux_inserted(MaxInputHandle* h);
int32_t         max_input_get_wheel_step(MaxInputHandle* h);
int32_t         max_input_set_wheel_step(MaxInputHandle* h, uint8_t step);
int32_t         max_input_set_grab(MaxInputHandle* h, uint8_t grab);
int32_t         max_input_next_event(MaxInputHandle* h, uint32_t timeout_ms, MaxInputEvent* out);

/* === Модуль Remote (виртуальный пульт /dev/uinput) === */
MaxRemoteHandle* max_remote_open(void);
void             max_remote_close(MaxRemoteHandle* h);
int32_t          max_remote_send_key(MaxRemoteHandle* h, const char* key);

/* === Модуль Sensors (освещённость IIO ltr308als01, зоны температуры, CPU, RTC) === */
MaxSensorsHandle* max_sensors_open(void);
void              max_sensors_close(MaxSensorsHandle* h);
int32_t           max_sensors_read_light(MaxSensorsHandle* h, MaxLightReading* out);
int32_t           max_sensors_configure_light(MaxSensorsHandle* h, uint8_t gain, uint16_t integration_ms);
int32_t           max_sensors_read_thermal(MaxSensorsHandle* h, MaxThermalReading* out);
int32_t           max_sensors_cpu_freq_mhz(MaxSensorsHandle* h);
int64_t           max_sensors_rtc_epoch(MaxSensorsHandle* h);

/* === Модуль Audio (громкость, усилители SY6045S, AAudio синтез/WAV, media_session) === */
MaxAudioHandle* max_audio_open(void);
void            max_audio_close(MaxAudioHandle* h);
int32_t         max_audio_get_volume(MaxAudioHandle* h);
int32_t         max_audio_set_volume(MaxAudioHandle* h, uint8_t percent);
int32_t         max_audio_is_muted(MaxAudioHandle* h);
int32_t         max_audio_set_mute(MaxAudioHandle* h, uint8_t mute);
int32_t         max_audio_read_amp(MaxAudioHandle* h, MaxAmpStatus* out);
int32_t         max_audio_set_amp(MaxAudioHandle* h, int32_t woofer, int32_t tweeters, int32_t aux, int32_t eq, int32_t drc);
int32_t         max_audio_override_sound(MaxAudioHandle* h, const char* target_rel, const char* custom_path);
int32_t         max_audio_restore_sounds(MaxAudioHandle* h);
int32_t         max_audio_play_tone(MaxAudioHandle* h, float freq_hz, uint32_t duration_ms, float volume);
int32_t         max_audio_play_wav(MaxAudioHandle* h, const char* path);
void            max_audio_stop(MaxAudioHandle* h);
int32_t         max_audio_read_track(MaxAudioHandle* h, MaxTrackInfo* out);

/* === Модуль Mic (Zero-Copy 16-канальный буфер AudioFlinger) === */
MaxMicHandle* max_mic_open(void);
void          max_mic_close(MaxMicHandle* h);
int32_t       max_mic_info(MaxMicHandle* h, MaxMicInfo* out);
int32_t       max_mic_is_muted(MaxMicHandle* h);
int32_t       max_mic_read_peaks(MaxMicHandle* h, uint32_t frames, uint16_t* out16);

/* === Модуль Bluetooth (Bluedroid + hci0) === */
MaxBtHandle* max_bt_open(void);
void         max_bt_close(MaxBtHandle* h);
int32_t      max_bt_status(MaxBtHandle* h, MaxBtStatus* out);
int32_t      max_bt_set_power(MaxBtHandle* h, uint8_t on);

/* === Модуль System (телеметрия, Android properties, приватное хранилище пакета) === */
int32_t max_sys_info(MaxSysInfo* out);
int32_t max_sys_getprop(const char* key, uint8_t* out_buf, size_t out_len);
int32_t max_sys_setprop(const char* key, const char* val);
int32_t max_sys_write_data(const char* name, const char* content);
int32_t max_sys_read_data(const char* name, uint8_t* out_buf, size_t out_len);

#ifdef __cplusplus
}
#endif

#endif /* MAX_HAL_H */
