#include <stdio.h>
#include "../../sdk/c/max_hal.h"

int main(void) {
    if (max_hal_init(NULL, MAX_PRIORITY_NORMAL, 0) < 0) {
        fprintf(stderr, "[pkg-c] init failed: %s\n", max_hal_last_error());
        return 1;
    }

    MaxDeviceInfo dev;
    max_device_info(&dev);
    printf("[pkg-c] started on %s (%s), leds=%u\n", dev.name, dev.model, dev.led_count);

    MaxRingHandle* ring = max_ring_open();
    if (!ring) {
        fprintf(stderr, "[pkg-c] ring open failed: %s\n", max_hal_last_error());
        return 1;
    }
    max_ring_clear(ring);
    max_ring_set_pixel(ring, 0, 11, 22, 33);
    max_ring_set_pixel(ring, 1, 44, 55, 66);
    max_ring_set_pixel(ring, 2, 77, 88, 99);
    max_ring_flush(ring);

    MaxSensorsHandle* sens = max_sensors_open();
    if (!sens) {
        fprintf(stderr, "[pkg-c] sensors open failed: %s\n", max_hal_last_error());
        max_ring_close(ring);
        return 1;
    }
    if (max_sensors_configure_light(sens, 6, 200) < 0) {
        fprintf(stderr, "[pkg-c] configure_light failed: %s\n", max_hal_last_error());
    }

    MaxLightReading light;
    MaxThermalReading therm;
    max_sensors_read_light(sens, &light);
    max_sensors_read_thermal(sens, &therm);

    char prop_buf[128];
    snprintf(prop_buf, sizeof(prop_buf), "gain=%u_int=%u_soc=%.1f",
             light.gain, light.integration_ms, therm.soc_c);
    max_sys_setprop("debug.max.oracle.c", prop_buf);
    max_sys_write_data("oracle_c.txt", prop_buf);
    printf("[pkg-c] configured sensors: %s\n", prop_buf);
    fflush(stdout);

    MaxEventsHandle* bus = max_events_open();
    if (!bus) {
        fprintf(stderr, "[pkg-c] events open failed: %s\n", max_hal_last_error());
        max_sensors_close(sens);
        max_ring_close(ring);
        return 1;
    }

    MaxEvent ev;
    while (max_events_next(bus, -1, &ev) > 0) {
        if (ev.kind == MAX_EVENT_STOP) {
            break;
        }
    }

    printf("[pkg-c] MAX_EVENT_STOP received, closing handles and rolling back hardware.\n");
    max_events_close(bus);
    max_sensors_close(sens);
    max_ring_close(ring);
    return 0;
}
