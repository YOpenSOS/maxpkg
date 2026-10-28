const std = @import("std");
const maxhal = @import("maxhal.zig");

fn run() !void {
    try maxhal.init();
    const dev = try maxhal.deviceInfo();
    _ = dev;

    const input = try maxhal.Input.open();
    defer input.close();
    try input.setWheelStep(7);

    const ring = try maxhal.Ring.open();
    defer ring.close();
    try ring.clear();
    try ring.setPixel(0, 101, 202, 55);
    try ring.setPixel(23, 250, 150, 50);
    try ring.flush();

    try maxhal.setProp("debug.max.oracle.zig", "step=7_led0=101_202_55");
    try maxhal.writeData("oracle_zig.txt", "step=7_led0=101_202_55");

    const bus = try maxhal.EventBus.open();
    defer bus.close();

    while (try bus.next(-1)) |ev| {
        if (ev.kind == @intFromEnum(maxhal.EventKind.stop)) {
            break;
        }
    }
}

comptime {
    maxhal.exportMain(run);
}
