//! maxhal.zig — Официальный Zig SDK для `max-platform` поверх `libmax_hal`.
//! Полное покрытие всех 10 аппаратных модулей (`device`, `canvas`, `ring`, `input`,
//! `remote`, `sensors`, `audio`, `mic`, `bluetooth`, `system`) с `defer obj.close()`.

const std = @import("std");

pub const Priority = enum(u8) {
    background = 0,
    normal = 1,
    overlay = 2,
};

pub const HalError = error{
    OpenFailed,
    OperationFailed,
};

pub const MaxEventsHandle = opaque {};
pub const MaxCanvasHandle = opaque {};
pub const MaxRingHandle = opaque {};
pub const MaxInputHandle = opaque {};
pub const MaxRemoteHandle = opaque {};
pub const MaxSensorsHandle = opaque {};
pub const MaxAudioHandle = opaque {};
pub const MaxMicHandle = opaque {};
pub const MaxBtHandle = opaque {};

pub const DeviceInfo = extern struct {
    model: [32]u8,
    name: [64]u8,
    sku: [32]u8,
    serial: [32]u8,
    os: [16]u8,
    arch: [16]u8,
    kernel: [32]u8,
    firmware: [32]u8,
    led_count: u8,
    screen_w: u16,
    screen_h: u16,
    mic_count: u8,
    has_encoder: u8,
    has_hdmi: u8,
};

pub const EventKind = enum(u32) {
    none = 0,
    stop = 1,
    tick = 2,
    input = 3,
    bt_power = 4,
    bt_config = 5,
    alice_state = 6,
    sound_play = 7,
};

pub const Event = extern struct {
    kind: u32,
    code: i32,
    delta: i32,
    held_ms: u64,
    timestamp_ms: u64,
    source: [32]u8,
    name: [64]u8,
};

pub const InputEvent = extern struct {
    source: [16]u8,
    action: [16]u8,
    delta: i32,
    held_ms: u64,
    code: u16,
    value: i32,
    time_ms: u64,
};

pub const LightReading = extern struct {
    lux: f32,
    raw: u32,
    gain: u8,
    integration_ms: u16,
};

pub const ThermalReading = extern struct {
    soc_c: f32,
    ddr_c: f32,
    wifi_c: f32,
};

pub const AmpStatus = extern struct {
    woofer: u8,
    tweeters: u8,
    aux: u8,
    woofer_on: u8,
    tweeters_on: u8,
    eq: u8,
    drc: u8,
};

pub const TrackInfo = extern struct {
    active: u8,
    playing: u8,
    volume: u8,
    _pad: u8,
    position_ms: i64,
    duration_ms: i64,
    package: [64]u8,
    title: [128]u8,
    artist: [128]u8,
    album: [128]u8,
};

pub const MicInfo = extern struct {
    rate: u32,
    channels: u8,
    _pad: [3]u8,
    buffer_frames: u32,
    pid: u32,
    rear: u32,
};

pub const BtStatus = extern struct {
    enabled: u8,
    connected: u8,
    scan_mode: u8,
    _pad: u8,
    bonded_count: u32,
    address: [24]u8,
    name: [64]u8,
};

pub const SysInfo = extern struct {
    unix_sec: u64,
    uptime_sec: f64,
    mem_total_kb: u64,
    mem_avail_kb: u64,
    load1: f32,
    wifi_rssi_dbm: i32,
    disk_free_mb: u64,
    wifi_connected: u8,
    _pad: [7]u8,
    wifi_ip: [32]u8,
    wifi_ssid: [64]u8,
};

extern fn max_hal_last_error() [*:0]const u8;
extern fn max_hal_init(pkg_id: ?[*:0]const u8, priority: u8, replace: u8) i32;
extern fn max_hal_should_stop() i32;
extern fn max_hal_sleep_ms(ms: u32) i32;
extern fn max_device_info(out: *DeviceInfo) i32;

extern fn max_canvas_open() ?*MaxCanvasHandle;
extern fn max_canvas_close(h: *MaxCanvasHandle) void;
extern fn max_canvas_clear(h: *MaxCanvasHandle) i32;
extern fn max_canvas_set_pixel(h: *MaxCanvasHandle, x: u32, y: u32, br: u8) i32;
extern fn max_canvas_set_frame(h: *MaxCanvasHandle, pixels400: [*]const u8, len: usize) i32;
extern fn max_canvas_get_brightness(h: *MaxCanvasHandle) i32;
extern fn max_canvas_set_brightness(h: *MaxCanvasHandle, br: u8) i32;
extern fn max_canvas_flush(h: *MaxCanvasHandle) i32;

extern fn max_ring_open() ?*MaxRingHandle;
extern fn max_ring_close(h: *MaxRingHandle) void;
extern fn max_ring_clear(h: *MaxRingHandle) i32;
extern fn max_ring_set_pixel(h: *MaxRingHandle, idx: u32, r: u8, g: u8, b: u8) i32;
extern fn max_ring_fill(h: *MaxRingHandle, r: u8, g: u8, b: u8) i32;
extern fn max_ring_flush(h: *MaxRingHandle) i32;
extern fn max_ring_off(h: *MaxRingHandle) i32;

extern fn max_events_open() ?*MaxEventsHandle;
extern fn max_events_close(h: *MaxEventsHandle) void;
extern fn max_events_set_tick_ms(h: *MaxEventsHandle, period_ms: u32) i32;
extern fn max_events_set_input_grab(h: *MaxEventsHandle, grab: u8) i32;
extern fn max_events_next(h: *MaxEventsHandle, timeout_ms: i32, out: *Event) i32;

extern fn max_input_open() ?*MaxInputHandle;
extern fn max_input_close(h: *MaxInputHandle) void;
extern fn max_input_is_mic_muted(h: *MaxInputHandle) i32;
extern fn max_input_is_aux_inserted(h: *MaxInputHandle) i32;
extern fn max_input_get_wheel_step(h: *MaxInputHandle) i32;
extern fn max_input_set_wheel_step(h: *MaxInputHandle, step: u8) i32;
extern fn max_input_set_grab(h: *MaxInputHandle, grab: u8) i32;
extern fn max_input_next_event(h: *MaxInputHandle, timeout_ms: u32, out: *InputEvent) i32;

extern fn max_remote_open() ?*MaxRemoteHandle;
extern fn max_remote_close(h: *MaxRemoteHandle) void;
extern fn max_remote_send_key(h: *MaxRemoteHandle, key: [*:0]const u8) i32;

extern fn max_sensors_open() ?*MaxSensorsHandle;
extern fn max_sensors_close(h: *MaxSensorsHandle) void;
extern fn max_sensors_read_light(h: *MaxSensorsHandle, out: *LightReading) i32;
extern fn max_sensors_configure_light(h: *MaxSensorsHandle, gain: u8, integration_ms: u16) i32;
extern fn max_sensors_read_thermal(h: *MaxSensorsHandle, out: *ThermalReading) i32;
extern fn max_sensors_cpu_freq_mhz(h: *MaxSensorsHandle) i32;
extern fn max_sensors_rtc_epoch(h: *MaxSensorsHandle) i64;

extern fn max_audio_open() ?*MaxAudioHandle;
extern fn max_audio_close(h: *MaxAudioHandle) void;
extern fn max_audio_get_volume(h: *MaxAudioHandle) i32;
extern fn max_audio_set_volume(h: *MaxAudioHandle, percent: u8) i32;
extern fn max_audio_is_muted(h: *MaxAudioHandle) i32;
extern fn max_audio_set_mute(h: *MaxAudioHandle, mute: u8) i32;
extern fn max_audio_read_amp(h: *MaxAudioHandle, out: *AmpStatus) i32;
extern fn max_audio_set_amp(h: *MaxAudioHandle, woofer: i32, tweeters: i32, aux: i32, eq: i32, drc: i32) i32;
extern fn max_audio_override_sound(h: *MaxAudioHandle, target_rel: [*:0]const u8, custom_path: [*:0]const u8) i32;
extern fn max_audio_restore_sounds(h: *MaxAudioHandle) i32;
extern fn max_audio_play_tone(h: *MaxAudioHandle, freq_hz: f32, duration_ms: u32, volume: f32) i32;
extern fn max_audio_play_wav(h: *MaxAudioHandle, path: [*:0]const u8) i32;
extern fn max_audio_stop(h: *MaxAudioHandle) void;
extern fn max_audio_read_track(h: *MaxAudioHandle, out: *TrackInfo) i32;

extern fn max_mic_open() ?*MaxMicHandle;
extern fn max_mic_close(h: *MaxMicHandle) void;
extern fn max_mic_info(h: *MaxMicHandle, out: *MicInfo) i32;
extern fn max_mic_is_muted(h: *MaxMicHandle) i32;
extern fn max_mic_read_peaks(h: *MaxMicHandle, frames: u32, out16: [*]u16) i32;

extern fn max_bt_open() ?*MaxBtHandle;
extern fn max_bt_close(h: *MaxBtHandle) void;
extern fn max_bt_status(h: *MaxBtHandle, out: *BtStatus) i32;
extern fn max_bt_set_power(h: *MaxBtHandle, on: u8) i32;

extern fn max_sys_info(out: *SysInfo) i32;
extern fn max_sys_getprop(key: [*:0]const u8, out_buf: [*]u8, out_len: usize) i32;
extern fn max_sys_setprop(key: [*:0]const u8, val: [*:0]const u8) i32;
extern fn max_sys_write_data(name: [*:0]const u8, content: [*:0]const u8) i32;
extern fn max_sys_read_data(name: [*:0]const u8, out_buf: [*]u8, out_len: usize) i32;

pub fn lastErrorSlice() []const u8 {
    return std.mem.span(max_hal_last_error());
}

pub fn init() !void {
    if (max_hal_init(null, @intFromEnum(Priority.normal), 0) < 0) return HalError.OperationFailed;
}

pub fn shouldStop() bool {
    return max_hal_should_stop() != 0;
}

pub fn sleepMs(ms: u32) bool {
    return max_hal_sleep_ms(ms) != 0;
}

pub fn deviceInfo() !DeviceInfo {
    var info: DeviceInfo = std.mem.zeroes(DeviceInfo);
    if (max_device_info(&info) < 0) return HalError.OperationFailed;
    return info;
}

pub const Canvas = struct {
    h: *MaxCanvasHandle,
    pub fn open() !Canvas {
        return Canvas{ .h = max_canvas_open() orelse return HalError.OpenFailed };
    }
    pub fn close(self: Canvas) void { max_canvas_close(self.h); }
    pub fn clear(self: Canvas) !void {
        if (max_canvas_clear(self.h) < 0) return HalError.OperationFailed;
    }
    pub fn setPixel(self: Canvas, x: u32, y: u32, br: u8) !void {
        if (max_canvas_set_pixel(self.h, x, y, br) < 0) return HalError.OperationFailed;
    }
    pub fn setFrame(self: Canvas, frame: *const [400]u8) !void {
        if (max_canvas_set_frame(self.h, frame, 400) < 0) return HalError.OperationFailed;
    }
    pub fn getBrightness(self: Canvas) !u8 {
        const rc = max_canvas_get_brightness(self.h);
        if (rc < 0) return HalError.OperationFailed;
        return @intCast(rc);
    }
    pub fn setBrightness(self: Canvas, br: u8) !void {
        if (max_canvas_set_brightness(self.h, br) < 0) return HalError.OperationFailed;
    }
    pub fn flush(self: Canvas) !void {
        if (max_canvas_flush(self.h) < 0) return HalError.OperationFailed;
    }
};

pub const Ring = struct {
    h: *MaxRingHandle,
    pub fn open() !Ring {
        return Ring{ .h = max_ring_open() orelse return HalError.OpenFailed };
    }
    pub fn close(self: Ring) void { max_ring_close(self.h); }
    pub fn clear(self: Ring) !void {
        if (max_ring_clear(self.h) < 0) return HalError.OperationFailed;
    }
    pub fn setPixel(self: Ring, idx: u32, r: u8, g: u8, b: u8) !void {
        if (max_ring_set_pixel(self.h, idx, r, g, b) < 0) return HalError.OperationFailed;
    }
    pub fn fill(self: Ring, r: u8, g: u8, b: u8) !void {
        if (max_ring_fill(self.h, r, g, b) < 0) return HalError.OperationFailed;
    }
    pub fn flush(self: Ring) !void {
        if (max_ring_flush(self.h) < 0) return HalError.OperationFailed;
    }
    pub fn off(self: Ring) !void {
        if (max_ring_off(self.h) < 0) return HalError.OperationFailed;
    }
};

pub const EventBus = struct {
    h: *MaxEventsHandle,
    pub fn open() !EventBus {
        return EventBus{ .h = max_events_open() orelse return HalError.OpenFailed };
    }
    pub fn close(self: EventBus) void { max_events_close(self.h); }
    pub fn setTickMs(self: EventBus, period_ms: u32) !void {
        if (max_events_set_tick_ms(self.h, period_ms) < 0) return HalError.OperationFailed;
    }
    pub fn setInputGrab(self: EventBus, grab: bool) !void {
        if (max_events_set_input_grab(self.h, if (grab) 1 else 0) < 0) return HalError.OperationFailed;
    }
    pub fn next(self: EventBus, timeout_ms: i32) !?Event {
        var ev: Event = std.mem.zeroes(Event);
        const rc = max_events_next(self.h, timeout_ms, &ev);
        if (rc < 0) return HalError.OperationFailed;
        if (rc == 0) return null;
        return ev;
    }
};

pub const Input = struct {
    h: *MaxInputHandle,
    pub fn open() !Input {
        return Input{ .h = max_input_open() orelse return HalError.OpenFailed };
    }
    pub fn close(self: Input) void { max_input_close(self.h); }
    pub fn isMicMuted(self: Input) !bool {
        const rc = max_input_is_mic_muted(self.h);
        if (rc < 0) return HalError.OperationFailed;
        return rc != 0;
    }
    pub fn isAuxInserted(self: Input) !bool {
        const rc = max_input_is_aux_inserted(self.h);
        if (rc < 0) return HalError.OperationFailed;
        return rc != 0;
    }
    pub fn getWheelStep(self: Input) !u8 {
        const rc = max_input_get_wheel_step(self.h);
        if (rc < 0) return HalError.OperationFailed;
        return @intCast(rc);
    }
    pub fn setWheelStep(self: Input, step: u8) !void {
        if (max_input_set_wheel_step(self.h, step) < 0) return HalError.OperationFailed;
    }
    pub fn setGrab(self: Input, grab: bool) !void {
        if (max_input_set_grab(self.h, if (grab) 1 else 0) < 0) return HalError.OperationFailed;
    }
    pub fn nextEvent(self: Input, timeout_ms: u32) !?InputEvent {
        var ev: InputEvent = std.mem.zeroes(InputEvent);
        const rc = max_input_next_event(self.h, timeout_ms, &ev);
        if (rc < 0) return HalError.OperationFailed;
        if (rc == 0) return null;
        return ev;
    }
};

pub const Remote = struct {
    h: *MaxRemoteHandle,
    pub fn open() !Remote {
        return Remote{ .h = max_remote_open() orelse return HalError.OpenFailed };
    }
    pub fn close(self: Remote) void { max_remote_close(self.h); }
    pub fn sendKey(self: Remote, key: [*:0]const u8) !void {
        if (max_remote_send_key(self.h, key) < 0) return HalError.OperationFailed;
    }
};

pub const Sensors = struct {
    h: *MaxSensorsHandle,
    pub fn open() !Sensors {
        return Sensors{ .h = max_sensors_open() orelse return HalError.OpenFailed };
    }
    pub fn close(self: Sensors) void { max_sensors_close(self.h); }
    pub fn readLight(self: Sensors) !LightReading {
        var r: LightReading = std.mem.zeroes(LightReading);
        if (max_sensors_read_light(self.h, &r) < 0) return HalError.OperationFailed;
        return r;
    }
    pub fn configureLight(self: Sensors, gain: u8, integration_ms: u16) !void {
        if (max_sensors_configure_light(self.h, gain, integration_ms) < 0) return HalError.OperationFailed;
    }
    pub fn readThermal(self: Sensors) !ThermalReading {
        var t: ThermalReading = std.mem.zeroes(ThermalReading);
        if (max_sensors_read_thermal(self.h, &t) < 0) return HalError.OperationFailed;
        return t;
    }
    pub fn cpuFreqMhz(self: Sensors) !u32 {
        const rc = max_sensors_cpu_freq_mhz(self.h);
        if (rc < 0) return HalError.OperationFailed;
        return @intCast(rc);
    }
    pub fn rtcEpoch(self: Sensors) !u64 {
        const rc = max_sensors_rtc_epoch(self.h);
        if (rc < 0) return HalError.OperationFailed;
        return @intCast(rc);
    }
};

pub const Audio = struct {
    h: *MaxAudioHandle,
    pub fn open() !Audio {
        return Audio{ .h = max_audio_open() orelse return HalError.OpenFailed };
    }
    pub fn close(self: Audio) void { max_audio_close(self.h); }
    pub fn getVolume(self: Audio) !u8 {
        const rc = max_audio_get_volume(self.h);
        if (rc < 0) return HalError.OperationFailed;
        return @intCast(rc);
    }
    pub fn setVolume(self: Audio, pct: u8) !void {
        if (max_audio_set_volume(self.h, pct) < 0) return HalError.OperationFailed;
    }
    pub fn isMuted(self: Audio) !bool {
        const rc = max_audio_is_muted(self.h);
        if (rc < 0) return HalError.OperationFailed;
        return rc != 0;
    }
    pub fn setMute(self: Audio, mute: bool) !void {
        if (max_audio_set_mute(self.h, if (mute) 1 else 0) < 0) return HalError.OperationFailed;
    }
    pub fn readAmp(self: Audio) !AmpStatus {
        var st: AmpStatus = std.mem.zeroes(AmpStatus);
        if (max_audio_read_amp(self.h, &st) < 0) return HalError.OperationFailed;
        return st;
    }
    pub fn setAmp(self: Audio, woofer: i32, tweeters: i32, aux: i32, eq: i32, drc: i32) !void {
        if (max_audio_set_amp(self.h, woofer, tweeters, aux, eq, drc) < 0) return HalError.OperationFailed;
    }
    pub fn overrideSound(self: Audio, target_rel: [*:0]const u8, custom_path: [*:0]const u8) !void {
        if (max_audio_override_sound(self.h, target_rel, custom_path) < 0) return HalError.OperationFailed;
    }
    pub fn restoreSounds(self: Audio) !void {
        if (max_audio_restore_sounds(self.h) < 0) return HalError.OperationFailed;
    }
    pub fn playTone(self: Audio, freq_hz: f32, duration_ms: u32, volume: f32) !void {
        if (max_audio_play_tone(self.h, freq_hz, duration_ms, volume) < 0) return HalError.OperationFailed;
    }
    pub fn playWav(self: Audio, path: [*:0]const u8) !void {
        if (max_audio_play_wav(self.h, path) < 0) return HalError.OperationFailed;
    }
    pub fn stop(self: Audio) void { max_audio_stop(self.h); }
    pub fn readTrack(self: Audio) !TrackInfo {
        var tr: TrackInfo = std.mem.zeroes(TrackInfo);
        if (max_audio_read_track(self.h, &tr) < 0) return HalError.OperationFailed;
        return tr;
    }
};

pub const Mic = struct {
    h: *MaxMicHandle,
    pub fn open() !Mic {
        return Mic{ .h = max_mic_open() orelse return HalError.OpenFailed };
    }
    pub fn close(self: Mic) void { max_mic_close(self.h); }
    pub fn info(self: Mic) !MicInfo {
        var inf: MicInfo = std.mem.zeroes(MicInfo);
        if (max_mic_info(self.h, &inf) < 0) return HalError.OperationFailed;
        return inf;
    }
    pub fn isMuted(self: Mic) bool {
        return max_mic_is_muted(self.h) != 0;
    }
    pub fn readPeaks(self: Mic, frames: u32) ![16]u16 {
        var peaks: [16]u16 = std.mem.zeroes([16]u16);
        if (max_mic_read_peaks(self.h, frames, &peaks) < 0) return HalError.OperationFailed;
        return peaks;
    }
};

pub const Bluetooth = struct {
    h: *MaxBtHandle,
    pub fn open() !Bluetooth {
        return Bluetooth{ .h = max_bt_open() orelse return HalError.OpenFailed };
    }
    pub fn close(self: Bluetooth) void { max_bt_close(self.h); }
    pub fn status(self: Bluetooth) !BtStatus {
        var st: BtStatus = std.mem.zeroes(BtStatus);
        if (max_bt_status(self.h, &st) < 0) return HalError.OperationFailed;
        return st;
    }
    pub fn setPower(self: Bluetooth, on: bool) !void {
        if (max_bt_set_power(self.h, if (on) 1 else 0) < 0) return HalError.OperationFailed;
    }
};

pub fn sysInfo() !SysInfo {
    var inf: SysInfo = std.mem.zeroes(SysInfo);
    if (max_sys_info(&inf) < 0) return HalError.OperationFailed;
    return inf;
}

pub fn setProp(key: [*:0]const u8, val: [*:0]const u8) !void {
    if (max_sys_setprop(key, val) < 0) return HalError.OperationFailed;
}

pub fn getProp(key: [*:0]const u8, buf: []u8) ![]const u8 {
    const rc = max_sys_getprop(key, buf.ptr, buf.len);
    if (rc < 0) return HalError.OperationFailed;
    return std.mem.sliceTo(buf, 0);
}

pub fn writeData(name: [*:0]const u8, content: [*:0]const u8) !void {
    if (max_sys_write_data(name, content) < 0) return HalError.OperationFailed;
}

pub fn readData(name: [*:0]const u8, buf: []u8) ![]const u8 {
    const rc = max_sys_read_data(name, buf.ptr, buf.len);
    if (rc < 0) return HalError.OperationFailed;
    return std.mem.sliceTo(buf, 0);
}

pub fn exportMain(comptime entryFn: fn () anyerror!void) void {
    const Wrapper = struct {
        fn cMain() callconv(.C) c_int {
            entryFn() catch return 1;
            return 0;
        }
    };
    @export(Wrapper.cMain, .{ .name = "main", .linkage = .strong });
}
