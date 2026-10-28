// Package maxhal — Файл 2/2: Audio, Mic, Bluetooth, System.
package maxhal

/*
#include "max_hal.h"
#include <stdlib.h>
*/
import "C"
import "unsafe"

// Audio управляет системной громкостью, усилителями SY6045S, AAudio и media_session.
type Audio struct {
	h *C.MaxAudioHandle
}

type AmpStatus struct {
	Woofer     uint8
	Tweeters   uint8
	Aux        uint8
	WooferOn   bool
	TweetersOn bool
	EQ         bool
	DRC        bool
}

type TrackInfo struct {
	Active     bool
	Playing    bool
	State      uint8
	PositionMs int64
	UpdatedMs  int64
	Package    string
	Title      string
	Artist     string
	Album      string
}

func OpenAudio() (*Audio, error) {
	h := C.max_audio_open()
	if h == nil {
		return nil, lastError("max_audio_open")
	}
	return &Audio{h: h}, nil
}

func (a *Audio) Close() {
	if a.h != nil {
		C.max_audio_close(a.h)
		a.h = nil
	}
}

func (a *Audio) GetVolume() (uint8, error) {
	rc := C.max_audio_get_volume(a.h)
	if rc < 0 {
		return 0, lastError("max_audio_get_volume")
	}
	return uint8(rc), nil
}

func (a *Audio) SetVolume(percent uint8) error {
	if C.max_audio_set_volume(a.h, C.uint8_t(percent)) < 0 {
		return lastError("max_audio_set_volume")
	}
	return nil
}

func (a *Audio) IsMuted() (bool, error) {
	rc := C.max_audio_is_muted(a.h)
	if rc < 0 {
		return false, lastError("max_audio_is_muted")
	}
	return rc != 0, nil
}

func (a *Audio) SetMute(mute bool) error {
	m := C.uint8_t(0)
	if mute {
		m = 1
	}
	if C.max_audio_set_mute(a.h, m) < 0 {
		return lastError("max_audio_set_mute")
	}
	return nil
}

func (a *Audio) OverrideSound(targetRel, customPath string) error {
	ct := C.CString(targetRel)
	cc := C.CString(customPath)
	defer C.free(unsafe.Pointer(ct))
	defer C.free(unsafe.Pointer(cc))
	if C.max_audio_override_sound(a.h, ct, cc) < 0 {
		return lastError("max_audio_override_sound")
	}
	return nil
}

func (a *Audio) RestoreSounds() error {
	if C.max_audio_restore_sounds(a.h) < 0 {
		return lastError("max_audio_restore_sounds")
	}
	return nil
}

func (a *Audio) ReadAmp() (AmpStatus, error) {
	var raw C.MaxAmpStatus
	if C.max_audio_read_amp(a.h, &raw) < 0 {
		return AmpStatus{}, lastError("max_audio_read_amp")
	}
	return AmpStatus{
		Woofer:     uint8(raw.woofer),
		Tweeters:   uint8(raw.tweeters),
		Aux:        uint8(raw.aux),
		WooferOn:   raw.woofer_on != 0,
		TweetersOn: raw.tweeters_on != 0,
		EQ:         raw.eq != 0,
		DRC:        raw.drc != 0,
	}, nil
}

// SetAmp устанавливает регистры SY6045S (-1 оставляет параметр без изменений).
func (a *Audio) SetAmp(woofer, tweeters, aux, eq, drc int32) error {
	if C.max_audio_set_amp(a.h, C.int32_t(woofer), C.int32_t(tweeters), C.int32_t(aux), C.int32_t(eq), C.int32_t(drc)) < 0 {
		return lastError("max_audio_set_amp")
	}
	return nil
}

func (a *Audio) PlayTone(freqHz float32, durationMs uint32, volume float32) error {
	if C.max_audio_play_tone(a.h, C.float(freqHz), C.uint32_t(durationMs), C.float(volume)) < 0 {
		return lastError("max_audio_play_tone")
	}
	return nil
}

func (a *Audio) PlayWav(path string) error {
	cp := C.CString(path)
	defer C.free(unsafe.Pointer(cp))
	if C.max_audio_play_wav(a.h, cp) < 0 {
		return lastError("max_audio_play_wav")
	}
	return nil
}

func (a *Audio) Stop() {
	C.max_audio_stop(a.h)
}

func (a *Audio) ReadTrack() (TrackInfo, error) {
	var raw C.MaxTrackInfo
	if C.max_audio_read_track(a.h, &raw) < 0 {
		return TrackInfo{}, lastError("max_audio_read_track")
	}
	return TrackInfo{
		Active:     raw.active != 0,
		Playing:    raw.playing != 0,
		State:      uint8(raw.volume),
		PositionMs: int64(raw.position_ms),
		UpdatedMs:  int64(raw.duration_ms),
		Package:    cStr(raw._package[:]),
		Title:      cStr(raw.title[:]),
		Artist:     cStr(raw.artist[:]),
		Album:      cStr(raw.album[:]),
	}, nil
}

// Mic читает 16-канальный Zero-Copy буфер AudioFlinger::Client.
type Mic struct {
	h *C.MaxMicHandle
}

type MicInfo struct {
	Rate         uint32
	Channels     uint8
	BufferFrames uint32
	PID          uint32
	Rear         uint32
}

func OpenMic() (*Mic, error) {
	h := C.max_mic_open()
	if h == nil {
		return nil, lastError("max_mic_open")
	}
	return &Mic{h: h}, nil
}

func (m *Mic) Close() {
	if m.h != nil {
		C.max_mic_close(m.h)
		m.h = nil
	}
}

func (m *Mic) Info() (MicInfo, error) {
	var raw C.MaxMicInfo
	if C.max_mic_info(m.h, &raw) < 0 {
		return MicInfo{}, lastError("max_mic_info")
	}
	return MicInfo{
		Rate:         uint32(raw.rate),
		Channels:     uint8(raw.channels),
		BufferFrames: uint32(raw.buffer_frames),
		PID:          uint32(raw.pid),
		Rear:         uint32(raw.rear),
	}, nil
}

func (m *Mic) IsMuted() bool {
	return C.max_mic_is_muted(m.h) != 0
}

func (m *Mic) ReadPeaks(frames uint32) ([16]uint16, error) {
	var out [16]uint16
	if C.max_mic_read_peaks(m.h, C.uint32_t(frames), (*C.uint16_t)(unsafe.Pointer(&out[0]))) < 0 {
		return out, lastError("max_mic_read_peaks")
	}
	return out, nil
}

// Bluetooth читает статус адаптера и управляет питанием Bluedroid.
type Bluetooth struct {
	h *C.MaxBtHandle
}

type BtStatus struct {
	Enabled     bool
	Connected   bool
	ScanMode    uint8
	BondedCount uint32
	Address     string
	Name        string
}

func OpenBluetooth() (*Bluetooth, error) {
	h := C.max_bt_open()
	if h == nil {
		return nil, lastError("max_bt_open")
	}
	return &Bluetooth{h: h}, nil
}

func (b *Bluetooth) Close() {
	if b.h != nil {
		C.max_bt_close(b.h)
		b.h = nil
	}
}

func (b *Bluetooth) Status() (BtStatus, error) {
	var raw C.MaxBtStatus
	if C.max_bt_status(b.h, &raw) < 0 {
		return BtStatus{}, lastError("max_bt_status")
	}
	return BtStatus{
		Enabled:     raw.enabled != 0,
		Connected:   raw.connected != 0,
		ScanMode:    uint8(raw.scan_mode),
		BondedCount: uint32(raw.bonded_count),
		Address:     cStr(raw.address[:]),
		Name:        cStr(raw.name[:]),
	}, nil
}

func (b *Bluetooth) SetPower(on bool) error {
	v := C.uint8_t(0)
	if on {
		v = 1
	}
	if C.max_bt_set_power(b.h, v) < 0 {
		return lastError("max_bt_set_power")
	}
	return nil
}

type SysInfo struct {
	UnixSec       uint64
	UptimeSec     float64
	MemTotalKB    uint64
	MemAvailKB    uint64
	Load1         float32
	DiskFreeMB    uint64
	WifiConnected bool
	WifiRssiDbm   int32
	WifiIP        string
	WifiSSID      string
}

func GetSysInfo() (SysInfo, error) {
	var raw C.MaxSysInfo
	if C.max_sys_info(&raw) < 0 {
		return SysInfo{}, lastError("max_sys_info")
	}
	return SysInfo{
		UnixSec:       uint64(raw.unix_sec),
		UptimeSec:     float64(raw.uptime_sec),
		MemTotalKB:    uint64(raw.mem_total_kb),
		MemAvailKB:    uint64(raw.mem_avail_kb),
		Load1:         float32(raw.load1),
		DiskFreeMB:    uint64(raw.disk_free_mb),
		WifiConnected: raw.wifi_connected != 0,
		WifiRssiDbm:   int32(raw.wifi_rssi_dbm),
		WifiIP:        cStr(raw.wifi_ip[:]),
		WifiSSID:      cStr(raw.wifi_ssid[:]),
	}, nil
}

func SetProp(key, val string) error {
	ck := C.CString(key)
	cv := C.CString(val)
	defer C.free(unsafe.Pointer(ck))
	defer C.free(unsafe.Pointer(cv))
	if C.max_sys_setprop(ck, cv) < 0 {
		return lastError("max_sys_setprop")
	}
	return nil
}

func GetProp(key string) (string, error) {
	ck := C.CString(key)
	defer C.free(unsafe.Pointer(ck))
	var buf [256]C.uint8_t
	rc := C.max_sys_getprop(ck, &buf[0], 256)
	if rc < 0 {
		return "", lastError("max_sys_getprop")
	}
	return C.GoString((*C.char)(unsafe.Pointer(&buf[0]))), nil
}

func WriteData(name, content string) error {
	cn := C.CString(name)
	cc := C.CString(content)
	defer C.free(unsafe.Pointer(cn))
	defer C.free(unsafe.Pointer(cc))
	if C.max_sys_write_data(cn, cc) < 0 {
		return lastError("max_sys_write_data")
	}
	return nil
}

func ReadData(name string) (string, error) {
	cn := C.CString(name)
	defer C.free(unsafe.Pointer(cn))
	var buf [1024]C.uint8_t
	rc := C.max_sys_read_data(cn, &buf[0], 1024)
	if rc < 0 {
		return "", lastError("max_sys_read_data")
	}
	return C.GoString((*C.char)(unsafe.Pointer(&buf[0]))), nil
}
