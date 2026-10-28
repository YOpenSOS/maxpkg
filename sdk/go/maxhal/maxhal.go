// Package maxhal предоставляет официальный Go SDK для `max-platform` поверх `libmax_hal`.
// Файл 1/2: Жизненный цикл, Device, Canvas, Ring, Input, Remote, Sensors.
package maxhal

/*
#cgo CFLAGS: -I${SRCDIR}/../../c
#include "max_hal.h"
#include <stdlib.h>
*/
import "C"
import (
	"errors"
	"fmt"
	"unsafe"
)

const (
	PriorityBackground uint8 = 0
	PriorityNormal     uint8 = 1
	PriorityOverlay    uint8 = 2
)

func lastError(ctx string) error {
	msg := C.GoString(C.max_hal_last_error())
	if msg == "" {
		msg = "operation failed"
	}
	return fmt.Errorf("%s: %s", ctx, msg)
}

func cStr(buf []C.char) string {
	for i, b := range buf {
		if b == 0 {
			return C.GoStringN(&buf[0], C.int(i))
		}
	}
	return C.GoStringN(&buf[0], C.int(len(buf)))
}

// Init регистрирует сигналы SIGINT/SIGTERM и контекст арбитра ресурсов.
func Init(pkgID string, priority uint8, replace bool) error {
	var cID *C.char
	if pkgID != "" {
		cID = C.CString(pkgID)
		defer C.free(unsafe.Pointer(cID))
	}
	rep := C.uint8_t(0)
	if replace {
		rep = 1
	}
	if C.max_hal_init(cID, C.uint8_t(priority), rep) < 0 {
		return lastError("max_hal_init")
	}
	return nil
}

func ShouldStop() bool {
	return C.max_hal_should_stop() != 0
}

func SleepMs(ms uint32) bool {
	return C.max_hal_sleep_ms(C.uint32_t(ms)) != 0
}

type DeviceInfo struct {
	Model      string
	Name       string
	SKU        string
	Serial     string
	OS         string
	Arch       string
	Kernel     string
	Firmware   string
	LedCount   uint8
	ScreenW    uint16
	ScreenH    uint16
	MicCount   uint8
	HasEncoder bool
	HasHDMI    bool
}

func GetDeviceInfo() (DeviceInfo, error) {
	var raw C.MaxDeviceInfo
	if C.max_device_info(&raw) < 0 {
		return DeviceInfo{}, lastError("max_device_info")
	}
	return DeviceInfo{
		Model:      cStr(raw.model[:]),
		Name:       cStr(raw.name[:]),
		SKU:        cStr(raw.sku[:]),
		Serial:     cStr(raw.serial[:]),
		OS:         cStr(raw.os[:]),
		Arch:       cStr(raw.arch[:]),
		Kernel:     cStr(raw.kernel[:]),
		Firmware:   cStr(raw.firmware[:]),
		LedCount:   uint8(raw.led_count),
		ScreenW:    uint16(raw.screen_w),
		ScreenH:    uint16(raw.screen_h),
		MicCount:   uint8(raw.mic_count),
		HasEncoder: raw.has_encoder != 0,
		HasHDMI:    raw.has_hdmi != 0,
	}, nil
}

// Canvas управляет матричным экраном 25x16 (/dev/graphics/fb4).
type Canvas struct {
	h *C.MaxCanvasHandle
}

func OpenCanvas() (*Canvas, error) {
	h := C.max_canvas_open()
	if h == nil {
		return nil, lastError("max_canvas_open")
	}
	return &Canvas{h: h}, nil
}

func (c *Canvas) Close() {
	if c.h != nil {
		C.max_canvas_close(c.h)
		c.h = nil
	}
}

func (c *Canvas) Clear() error {
	if C.max_canvas_clear(c.h) < 0 {
		return lastError("max_canvas_clear")
	}
	return nil
}

func (c *Canvas) SetPixel(x, y uint32, br uint8) error {
	if C.max_canvas_set_pixel(c.h, C.uint32_t(x), C.uint32_t(y), C.uint8_t(br)) < 0 {
		return lastError("max_canvas_set_pixel")
	}
	return nil
}

func (c *Canvas) SetFrame(pixels400 []byte) error {
	if len(pixels400) < 400 {
		return errors.New("frame must contain 400 bytes")
	}
	if C.max_canvas_set_frame(c.h, (*C.uint8_t)(unsafe.Pointer(&pixels400[0])), C.size_t(len(pixels400))) < 0 {
		return lastError("max_canvas_set_frame")
	}
	return nil
}

func (c *Canvas) GetBrightness() (uint8, error) {
	rc := C.max_canvas_get_brightness(c.h)
	if rc < 0 {
		return 0, lastError("max_canvas_get_brightness")
	}
	return uint8(rc), nil
}

func (c *Canvas) SetBrightness(br uint8) error {
	if C.max_canvas_set_brightness(c.h, C.uint8_t(br)) < 0 {
		return lastError("max_canvas_set_brightness")
	}
	return nil
}

func (c *Canvas) Flush() error {
	if C.max_canvas_flush(c.h) < 0 {
		return lastError("max_canvas_flush")
	}
	return nil
}

// Ring управляет 24 RGB светодиодами верхнего кольца (/dev/graphics/fb5).
type Ring struct {
	h *C.MaxRingHandle
}

func OpenRing() (*Ring, error) {
	h := C.max_ring_open()
	if h == nil {
		return nil, lastError("max_ring_open")
	}
	return &Ring{h: h}, nil
}

func (r *Ring) Close() {
	if r.h != nil {
		C.max_ring_close(r.h)
		r.h = nil
	}
}

func (r *Ring) Clear() error {
	if C.max_ring_clear(r.h) < 0 {
		return lastError("max_ring_clear")
	}
	return nil
}

func (r *Ring) SetPixel(idx uint32, red, green, blue uint8) error {
	if C.max_ring_set_pixel(r.h, C.uint32_t(idx), C.uint8_t(red), C.uint8_t(green), C.uint8_t(blue)) < 0 {
		return lastError("max_ring_set_pixel")
	}
	return nil
}

func (r *Ring) Fill(red, green, blue uint8) error {
	if C.max_ring_fill(r.h, C.uint8_t(red), C.uint8_t(green), C.uint8_t(blue)) < 0 {
		return lastError("max_ring_fill")
	}
	return nil
}

func (r *Ring) Flush() error {
	if C.max_ring_flush(r.h) < 0 {
		return lastError("max_ring_flush")
	}
	return nil
}

func (r *Ring) Off() error {
	if C.max_ring_off(r.h) < 0 {
		return lastError("max_ring_off")
	}
	return nil
}

// EventBus предоставляет доступ к единой шине ядерных событий (evdev + rfkill + inotify + timerfd + SIGTERM).
const (
	EventNone       uint32 = 0
	EventStop       uint32 = 1
	EventTick       uint32 = 2
	EventInput      uint32 = 3
	EventBtPower    uint32 = 4
	EventBtConfig   uint32 = 5
	EventAliceState uint32 = 6
	EventSoundPlay  uint32 = 7
)

type Event struct {
	Kind        uint32
	Code        int32
	Delta       int32
	HeldMs      uint64
	TimestampMs uint64
	Source      string
	Name        string
}

type EventBus struct {
	h *C.MaxEventsHandle
}

func OpenEventBus() (*EventBus, error) {
	h := C.max_events_open()
	if h == nil {
		return nil, lastError("max_events_open")
	}
	return &EventBus{h: h}, nil
}

func (b *EventBus) Close() {
	if b.h != nil {
		C.max_events_close(b.h)
		b.h = nil
	}
}

func (b *EventBus) SetTickMs(periodMs uint32) error {
	if C.max_events_set_tick_ms(b.h, C.uint32_t(periodMs)) < 0 {
		return lastError("max_events_set_tick_ms")
	}
	return nil
}

func (b *EventBus) SetInputGrab(grab bool) error {
	g := C.uint8_t(0)
	if grab {
		g = 1
	}
	if C.max_events_set_input_grab(b.h, g) < 0 {
		return lastError("max_events_set_input_grab")
	}
	return nil
}

func (b *EventBus) NextEvent(timeoutMs int32) (*Event, error) {
	var raw C.MaxEvent
	rc := C.max_events_next(b.h, C.int32_t(timeoutMs), &raw)
	if rc < 0 {
		return nil, lastError("max_events_next")
	}
	if rc == 0 {
		return nil, nil
	}
	return &Event{
		Kind:        uint32(raw.kind),
		Code:        int32(raw.code),
		Delta:       int32(raw.delta),
		HeldMs:      uint64(raw.held_ms),
		TimestampMs: uint64(raw.timestamp_ms),
		Source:      cStr(raw.source[:]),
		Name:        cStr(raw.name[:]),
	}, nil
}

// Input читает физические кнопки, крутилку, тумблер микрофонов и AUX.
type Input struct {
	h *C.MaxInputHandle
}

type InputEvent struct {
	Source string
	Action string
	Delta  int32
	HeldMs uint64
	Code   uint16
	Value  int32
	TimeMs uint64
}

func OpenInput() (*Input, error) {
	h := C.max_input_open()
	if h == nil {
		return nil, lastError("max_input_open")
	}
	return &Input{h: h}, nil
}

func (in *Input) Close() {
	if in.h != nil {
		C.max_input_close(in.h)
		in.h = nil
	}
}

func (in *Input) IsMicMuted() (bool, error) {
	rc := C.max_input_is_mic_muted(in.h)
	if rc < 0 {
		return false, lastError("max_input_is_mic_muted")
	}
	return rc != 0, nil
}

func (in *Input) IsAuxInserted() (bool, error) {
	rc := C.max_input_is_aux_inserted(in.h)
	if rc < 0 {
		return false, lastError("max_input_is_aux_inserted")
	}
	return rc != 0, nil
}

func (in *Input) GetWheelStep() (uint8, error) {
	rc := C.max_input_get_wheel_step(in.h)
	if rc < 0 {
		return 0, lastError("max_input_get_wheel_step")
	}
	return uint8(rc), nil
}

func (in *Input) SetWheelStep(step uint8) error {
	if C.max_input_set_wheel_step(in.h, C.uint8_t(step)) < 0 {
		return lastError("max_input_set_wheel_step")
	}
	return nil
}

func (in *Input) SetGrab(grab bool) error {
	g := C.uint8_t(0)
	if grab {
		g = 1
	}
	if C.max_input_set_grab(in.h, g) < 0 {
		return lastError("max_input_set_grab")
	}
	return nil
}

func (in *Input) NextEvent(timeoutMs uint32) (*InputEvent, error) {
	var raw C.MaxInputEvent
	rc := C.max_input_next_event(in.h, C.uint32_t(timeoutMs), &raw)
	if rc < 0 {
		return nil, lastError("max_input_next_event")
	}
	if rc == 0 {
		return nil, nil
	}
	return &InputEvent{
		Source: cStr(raw.source[:]),
		Action: cStr(raw.action[:]),
		Delta:  int32(raw.delta),
		HeldMs: uint64(raw.held_ms),
		Code:   uint16(raw.code),
		Value:  int32(raw.value),
		TimeMs: uint64(raw.time_ms),
	}, nil
}

// Remote управляет виртуальным пультом /dev/uinput (max-virtual-remote).
type Remote struct {
	h *C.MaxRemoteHandle
}

func OpenRemote() (*Remote, error) {
	h := C.max_remote_open()
	if h == nil {
		return nil, lastError("max_remote_open")
	}
	return &Remote{h: h}, nil
}

func (r *Remote) Close() {
	if r.h != nil {
		C.max_remote_close(r.h)
		r.h = nil
	}
}

func (r *Remote) SendKey(key string) error {
	ck := C.CString(key)
	defer C.free(unsafe.Pointer(ck))
	if C.max_remote_send_key(r.h, ck) < 0 {
		return lastError("max_remote_send_key")
	}
	return nil
}

// Sensors читает и настраивает датчики освещённости (ltr308als01), температуры, CPU и RTC.
type Sensors struct {
	h *C.MaxSensorsHandle
}

type LightReading struct {
	Lux           float32
	Raw           uint32
	Gain          uint8
	IntegrationMs uint16
}

type ThermalReading struct {
	SocC  float32
	DdrC  float32
	WifiC float32
}

func OpenSensors() (*Sensors, error) {
	h := C.max_sensors_open()
	if h == nil {
		return nil, lastError("max_sensors_open")
	}
	return &Sensors{h: h}, nil
}

func (s *Sensors) Close() {
	if s.h != nil {
		C.max_sensors_close(s.h)
		s.h = nil
	}
}

func (s *Sensors) ReadLight() (LightReading, error) {
	var raw C.MaxLightReading
	if C.max_sensors_read_light(s.h, &raw) < 0 {
		return LightReading{}, lastError("max_sensors_read_light")
	}
	return LightReading{
		Lux:           float32(raw.lux),
		Raw:           uint32(raw.raw),
		Gain:          uint8(raw.gain),
		IntegrationMs: uint16(raw.integration_ms),
	}, nil
}

func (s *Sensors) ConfigureLight(gain uint8, integrationMs uint16) error {
	if C.max_sensors_configure_light(s.h, C.uint8_t(gain), C.uint16_t(integrationMs)) < 0 {
		return lastError("max_sensors_configure_light")
	}
	return nil
}

func (s *Sensors) ReadThermal() (ThermalReading, error) {
	var raw C.MaxThermalReading
	if C.max_sensors_read_thermal(s.h, &raw) < 0 {
		return ThermalReading{}, lastError("max_sensors_read_thermal")
	}
	return ThermalReading{
		SocC:  float32(raw.soc_c),
		DdrC:  float32(raw.ddr_c),
		WifiC: float32(raw.wifi_c),
	}, nil
}

func (s *Sensors) CpuFreqMhz() (uint32, error) {
	rc := C.max_sensors_cpu_freq_mhz(s.h)
	if rc < 0 {
		return 0, lastError("max_sensors_cpu_freq_mhz")
	}
	return uint32(rc), nil
}

func (s *Sensors) RtcEpoch() (uint64, error) {
	rc := C.max_sensors_rtc_epoch(s.h)
	if rc < 0 {
		return 0, lastError("max_sensors_rtc_epoch")
	}
	return uint64(rc), nil
}
