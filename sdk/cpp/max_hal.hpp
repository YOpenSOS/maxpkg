/*
 * max_hal.hpp — Официальный C++17 RAII SDK для `max-platform`.
 * Все аппаратные дескрипторы автоматически освобождают локи и откатывают железо в деструкторах.
 */

#ifndef MAX_HAL_HPP
#define MAX_HAL_HPP

#include "../c/max_hal.h"
#include <array>
#include <stdexcept>
#include <string>
#include <utility>

namespace maxhal {

class HalError : public std::runtime_error {
public:
    explicit HalError(const std::string& ctx)
        : std::runtime_error(ctx + ": " + (max_hal_last_error() ? max_hal_last_error() : "unknown")) {}
};

inline void check_rc(int32_t rc, const char* ctx) {
    if (rc < 0) {
        throw HalError(ctx);
    }
}

struct Runtime {
    static void init(const char* pkg_id = nullptr, uint8_t priority = MAX_PRIORITY_NORMAL, bool replace = false) {
        check_rc(max_hal_init(pkg_id, priority, replace ? 1 : 0), "max_hal_init");
    }
    static bool should_stop() noexcept {
        return max_hal_should_stop() != 0;
    }
    static bool sleep_ms(uint32_t ms) noexcept {
        return max_hal_sleep_ms(ms) != 0;
    }
    static MaxDeviceInfo device_info() {
        MaxDeviceInfo info{};
        check_rc(max_device_info(&info), "max_device_info");
        return info;
    }
};

class Canvas {
    MaxCanvasHandle* h_{nullptr};
public:
    Canvas() : h_(max_canvas_open()) {
        if (!h_) throw HalError("max_canvas_open");
    }
    ~Canvas() {
        if (h_) max_canvas_close(h_);
    }
    Canvas(const Canvas&) = delete;
    Canvas& operator=(const Canvas&) = delete;
    Canvas(Canvas&& o) noexcept : h_(std::exchange(o.h_, nullptr)) {}

    void clear() { check_rc(max_canvas_clear(h_), "max_canvas_clear"); }
    void set_pixel(uint32_t x, uint32_t y, uint8_t br) {
        check_rc(max_canvas_set_pixel(h_, x, y, br), "max_canvas_set_pixel");
    }
    void set_frame(const std::array<uint8_t, 400>& frame) {
        check_rc(max_canvas_set_frame(h_, frame.data(), frame.size()), "max_canvas_set_frame");
    }
    uint8_t get_brightness() {
        int32_t v = max_canvas_get_brightness(h_);
        check_rc(v, "max_canvas_get_brightness");
        return static_cast<uint8_t>(v);
    }
    void set_brightness(uint8_t br) {
        check_rc(max_canvas_set_brightness(h_, br), "max_canvas_set_brightness");
    }
    void flush() { check_rc(max_canvas_flush(h_), "max_canvas_flush"); }
};

class Ring {
    MaxRingHandle* h_{nullptr};
public:
    Ring() : h_(max_ring_open()) {
        if (!h_) throw HalError("max_ring_open");
    }
    ~Ring() {
        if (h_) max_ring_close(h_);
    }
    Ring(const Ring&) = delete;
    Ring& operator=(const Ring&) = delete;
    Ring(Ring&& o) noexcept : h_(std::exchange(o.h_, nullptr)) {}

    void clear() { check_rc(max_ring_clear(h_), "max_ring_clear"); }
    void set_pixel(uint32_t idx, uint8_t r, uint8_t g, uint8_t b) {
        check_rc(max_ring_set_pixel(h_, idx, r, g, b), "max_ring_set_pixel");
    }
    void fill(uint8_t r, uint8_t g, uint8_t b) {
        check_rc(max_ring_fill(h_, r, g, b), "max_ring_fill");
    }
    void flush() { check_rc(max_ring_flush(h_), "max_ring_flush"); }
    void off() { check_rc(max_ring_off(h_), "max_ring_off"); }
};

class EventBus {
    MaxEventsHandle* h_{nullptr};
public:
    EventBus() : h_(max_events_open()) {
        if (!h_) throw HalError("max_events_open");
    }
    ~EventBus() {
        if (h_) max_events_close(h_);
    }
    EventBus(const EventBus&) = delete;
    EventBus& operator=(const EventBus&) = delete;
    EventBus(EventBus&& o) noexcept : h_(std::exchange(o.h_, nullptr)) {}

    void set_tick_ms(uint32_t period_ms) {
        check_rc(max_events_set_tick_ms(h_, period_ms), "max_events_set_tick_ms");
    }
    void set_input_grab(bool grab) {
        check_rc(max_events_set_input_grab(h_, grab ? 1 : 0), "max_events_set_input_grab");
    }
    bool next_event(int32_t timeout_ms, MaxEvent& out) {
        int32_t rc = max_events_next(h_, timeout_ms, &out);
        check_rc(rc, "max_events_next");
        return rc > 0;
    }
};

class Input {
    MaxInputHandle* h_{nullptr};
public:
    Input() : h_(max_input_open()) {
        if (!h_) throw HalError("max_input_open");
    }
    ~Input() {
        if (h_) max_input_close(h_);
    }
    Input(const Input&) = delete;
    Input& operator=(const Input&) = delete;
    Input(Input&& o) noexcept : h_(std::exchange(o.h_, nullptr)) {}

    bool is_mic_muted() const {
        int32_t v = max_input_is_mic_muted(h_);
        check_rc(v, "max_input_is_mic_muted");
        return v != 0;
    }
    bool is_aux_inserted() const {
        int32_t v = max_input_is_aux_inserted(h_);
        check_rc(v, "max_input_is_aux_inserted");
        return v != 0;
    }
    uint8_t get_wheel_step() const {
        int32_t v = max_input_get_wheel_step(h_);
        check_rc(v, "max_input_get_wheel_step");
        return static_cast<uint8_t>(v);
    }
    void set_wheel_step(uint8_t step) {
        check_rc(max_input_set_wheel_step(h_, step), "max_input_set_wheel_step");
    }
    void set_grab(bool grab) {
        check_rc(max_input_set_grab(h_, grab ? 1 : 0), "max_input_set_grab");
    }
    bool next_event(uint32_t timeout_ms, MaxInputEvent& out) {
        int32_t rc = max_input_next_event(h_, timeout_ms, &out);
        check_rc(rc, "max_input_next_event");
        return rc > 0;
    }
};

class Remote {
    MaxRemoteHandle* h_{nullptr};
public:
    Remote() : h_(max_remote_open()) {
        if (!h_) throw HalError("max_remote_open");
    }
    ~Remote() {
        if (h_) max_remote_close(h_);
    }
    Remote(const Remote&) = delete;
    Remote& operator=(const Remote&) = delete;
    Remote(Remote&& o) noexcept : h_(std::exchange(o.h_, nullptr)) {}

    void send_key(const std::string& key) {
        check_rc(max_remote_send_key(h_, key.c_str()), "max_remote_send_key");
    }
};

class Sensors {
    MaxSensorsHandle* h_{nullptr};
public:
    Sensors() : h_(max_sensors_open()) {
        if (!h_) throw HalError("max_sensors_open");
    }
    ~Sensors() {
        if (h_) max_sensors_close(h_);
    }
    Sensors(const Sensors&) = delete;
    Sensors& operator=(const Sensors&) = delete;
    Sensors(Sensors&& o) noexcept : h_(std::exchange(o.h_, nullptr)) {}

    MaxLightReading read_light() const {
        MaxLightReading r{};
        check_rc(max_sensors_read_light(h_, &r), "max_sensors_read_light");
        return r;
    }
    void configure_light(uint8_t gain, uint16_t integration_ms) {
        check_rc(max_sensors_configure_light(h_, gain, integration_ms), "max_sensors_configure_light");
    }
    MaxThermalReading read_thermal() const {
        MaxThermalReading t{};
        check_rc(max_sensors_read_thermal(h_, &t), "max_sensors_read_thermal");
        return t;
    }
    uint32_t cpu_freq_mhz() const {
        int32_t v = max_sensors_cpu_freq_mhz(h_);
        check_rc(v, "max_sensors_cpu_freq_mhz");
        return static_cast<uint32_t>(v);
    }
    uint64_t rtc_epoch() const {
        int64_t v = max_sensors_rtc_epoch(h_);
        if (v < 0) throw HalError("max_sensors_rtc_epoch");
        return static_cast<uint64_t>(v);
    }
};

class Audio {
    MaxAudioHandle* h_{nullptr};
public:
    Audio() : h_(max_audio_open()) {
        if (!h_) throw HalError("max_audio_open");
    }
    ~Audio() {
        if (h_) max_audio_close(h_);
    }
    Audio(const Audio&) = delete;
    Audio& operator=(const Audio&) = delete;
    Audio(Audio&& o) noexcept : h_(std::exchange(o.h_, nullptr)) {}

    uint8_t get_volume() const {
        int32_t v = max_audio_get_volume(h_);
        check_rc(v, "max_audio_get_volume");
        return static_cast<uint8_t>(v);
    }
    void set_volume(uint8_t pct) {
        check_rc(max_audio_set_volume(h_, pct), "max_audio_set_volume");
    }
    bool is_muted() const {
        int32_t v = max_audio_is_muted(h_);
        check_rc(v, "max_audio_is_muted");
        return v != 0;
    }
    void set_mute(bool mute) {
        check_rc(max_audio_set_mute(h_, mute ? 1 : 0), "max_audio_set_mute");
    }
    MaxAmpStatus read_amp() const {
        MaxAmpStatus st{};
        check_rc(max_audio_read_amp(h_, &st), "max_audio_read_amp");
        return st;
    }
    void set_amp(int32_t woofer, int32_t tweeters, int32_t aux, int32_t eq = -1, int32_t drc = -1) {
        check_rc(max_audio_set_amp(h_, woofer, tweeters, aux, eq, drc), "max_audio_set_amp");
    }
    void override_sound(const std::string& target_rel, const std::string& custom_path) {
        check_rc(max_audio_override_sound(h_, target_rel.c_str(), custom_path.c_str()), "max_audio_override_sound");
    }
    void restore_sounds() {
        check_rc(max_audio_restore_sounds(h_), "max_audio_restore_sounds");
    }
    void play_tone(float freq_hz, uint32_t duration_ms, float volume = 0.25f) {
        check_rc(max_audio_play_tone(h_, freq_hz, duration_ms, volume), "max_audio_play_tone");
    }
    void play_wav(const std::string& path) {
        check_rc(max_audio_play_wav(h_, path.c_str()), "max_audio_play_wav");
    }
    void stop() noexcept { max_audio_stop(h_); }
    MaxTrackInfo track() const {
        MaxTrackInfo tr{};
        check_rc(max_audio_read_track(h_, &tr), "max_audio_read_track");
        return tr;
    }
};

class Mic {
    MaxMicHandle* h_{nullptr};
public:
    Mic() : h_(max_mic_open()) {
        if (!h_) throw HalError("max_mic_open");
    }
    ~Mic() {
        if (h_) max_mic_close(h_);
    }
    Mic(const Mic&) = delete;
    Mic& operator=(const Mic&) = delete;
    Mic(Mic&& o) noexcept : h_(std::exchange(o.h_, nullptr)) {}

    MaxMicInfo info() {
        MaxMicInfo inf{};
        check_rc(max_mic_info(h_, &inf), "max_mic_info");
        return inf;
    }
    bool is_muted() const {
        return max_mic_is_muted(h_) != 0;
    }
    std::array<uint16_t, 16> read_peaks(uint32_t frames = 1024) {
        std::array<uint16_t, 16> peaks{};
        check_rc(max_mic_read_peaks(h_, frames, peaks.data()), "max_mic_read_peaks");
        return peaks;
    }
};

class Bluetooth {
    MaxBtHandle* h_{nullptr};
public:
    Bluetooth() : h_(max_bt_open()) {
        if (!h_) throw HalError("max_bt_open");
    }
    ~Bluetooth() {
        if (h_) max_bt_close(h_);
    }
    Bluetooth(const Bluetooth&) = delete;
    Bluetooth& operator=(const Bluetooth&) = delete;
    Bluetooth(Bluetooth&& o) noexcept : h_(std::exchange(o.h_, nullptr)) {}

    MaxBtStatus status() const {
        MaxBtStatus st{};
        check_rc(max_bt_status(h_, &st), "max_bt_status");
        return st;
    }
    void set_power(bool on) {
        check_rc(max_bt_set_power(h_, on ? 1 : 0), "max_bt_set_power");
    }
};

struct System {
    static MaxSysInfo info() {
        MaxSysInfo inf{};
        check_rc(max_sys_info(&inf), "max_sys_info");
        return inf;
    }
    static std::string getprop(const std::string& key) {
        char buf[256]{};
        check_rc(max_sys_getprop(key.c_str(), reinterpret_cast<uint8_t*>(buf), sizeof(buf)), "max_sys_getprop");
        return std::string(buf);
    }
    static void setprop(const std::string& key, const std::string& val) {
        check_rc(max_sys_setprop(key.c_str(), val.c_str()), "max_sys_setprop");
    }
    static void write_data(const std::string& name, const std::string& content) {
        check_rc(max_sys_write_data(name.c_str(), content.c_str()), "max_sys_write_data");
    }
    static std::string read_data(const std::string& name) {
        char buf[1024]{};
        check_rc(max_sys_read_data(name.c_str(), reinterpret_cast<uint8_t*>(buf), sizeof(buf)), "max_sys_read_data");
        return std::string(buf);
    }
};

} // namespace maxhal

#endif /* MAX_HAL_HPP */
