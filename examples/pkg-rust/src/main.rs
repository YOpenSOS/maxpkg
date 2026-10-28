//! Эталонный Event-Driven пакет на Rust (`examples/pkg-rust`).

use anyhow::Result;
use max_sdk::prelude::*;

fn main() -> Result<()> {
    let dev = max_sdk::init();
    println!(
        "[pkg-rust] started on {} ({}), sku={}, serial={}",
        dev.name, dev.model, dev.sku, dev.serial
    );

    let mut canvas = Fb4CanvasDriver::open_for_device(&dev)?;
    canvas.clear_buf();
    canvas.set_pixel(0, 0, 111);
    canvas.set_pixel(1, 0, 222);
    canvas.set_pixel(2, 0, 135);
    canvas.set_brightness(77)?;
    canvas.flush()?;

    let mut mic = AudioFlingerShmDriver::open();
    let minfo = mic.info()?;
    let peaks = mic.read_peaks(512)?;
    println!(
        "[pkg-rust] mic shm: rate={}, ch={}, frames={}, pid={}, ch0_peak={}",
        minfo.rate, minfo.channels, minfo.buffer_frames, minfo.pid, peaks[0]
    );

    let sys = LinuxSystemDriver::open();
    let prop_val = format!("{}_{}ch_{}", minfo.rate, minfo.channels, dev.model);
    sys.setprop("debug.max.oracle.rust", &prop_val)?;
    max_sdk::write_data("oracle.txt", &prop_val)?;

    let mut bus = EventBus::open(&dev)?;
    while let Some(ev) = bus.next_event(-1)? {
        if ev.kind_enum() == EventKind::Stop {
            break;
        }
    }

    println!("[pkg-rust] EventKind::Stop received, dropping Canvas and rolling back hardware.");
    Ok(())
}
