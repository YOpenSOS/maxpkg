package main

import (
	"fmt"
	"os"

	"maxplatform/sdk/go/maxhal"
)

func main() {
	if err := maxhal.Init("", 0, false); err != nil {
		fmt.Fprintf(os.Stderr, "[pkg-go] init error: %v\n", err)
		os.Exit(1)
	}

	dev, err := maxhal.GetDeviceInfo()
	if err != nil {
		fmt.Fprintf(os.Stderr, "[pkg-go] device info error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("[pkg-go] started on %s (%s)\n", dev.Name, dev.Model)

	canvas, err := maxhal.OpenCanvas()
	if err != nil {
		fmt.Fprintf(os.Stderr, "[pkg-go] canvas open error: %v\n", err)
		os.Exit(1)
	}
	defer canvas.Close()

	_ = canvas.Clear()
	_ = canvas.SetPixel(0, 0, 42)
	_ = canvas.SetPixel(1, 0, 84)
	_ = canvas.SetPixel(2, 0, 126)
	_ = canvas.SetBrightness(144)
	_ = canvas.Flush()

	bt, err := maxhal.OpenBluetooth()
	if err != nil {
		fmt.Fprintf(os.Stderr, "[pkg-go] bt open error: %v\n", err)
		os.Exit(1)
	}
	defer bt.Close()
	st, _ := bt.Status()

	in, err := maxhal.OpenInput()
	if err != nil {
		fmt.Fprintf(os.Stderr, "[pkg-go] input open error: %v\n", err)
		os.Exit(1)
	}
	defer in.Close()
	muted, _ := in.IsMicMuted()
	aux, _ := in.IsAuxInserted()

	prop := fmt.Sprintf("bonded=%d_muted=%v_aux=%v", st.BondedCount, muted, aux)
	_ = maxhal.SetProp("debug.max.oracle.go", prop)
	_ = maxhal.WriteData("oracle_go.txt", prop)
	fmt.Printf("[pkg-go] telemetry: %s\n", prop)

	bus, err := maxhal.OpenEventBus()
	if err != nil {
		fmt.Fprintf(os.Stderr, "[pkg-go] eventbus open error: %v\n", err)
		os.Exit(1)
	}
	defer bus.Close()

	for {
		ev, err := bus.NextEvent(-1)
		if err != nil || ev == nil || ev.Kind == maxhal.EventStop {
			break
		}
	}

	fmt.Println("[pkg-go] EventStop received, defer Close() restoring lower-priority frame.")
}
