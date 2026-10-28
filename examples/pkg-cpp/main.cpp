#include <cstdio>
#include <string>
#include "../../sdk/cpp/max_hal.hpp"

int main() {
    try {
        maxhal::Runtime::init();
        auto dev = maxhal::Runtime::device_info();
        std::printf("[pkg-cpp] started on %s (%s)\n", dev.name, dev.model);

        maxhal::Audio audio;
        maxhal::Remote remote;

        audio.set_amp(160, 162, 105);
        auto amp = audio.read_amp();

        std::string prop = "w=" + std::to_string(amp.woofer) +
                           "_t=" + std::to_string(amp.tweeters) +
                           "_a=" + std::to_string(amp.aux);
        maxhal::System::setprop("debug.max.oracle.cpp", prop);
        maxhal::System::write_data("oracle_cpp.txt", prop);
        std::printf("[pkg-cpp] SY6045S amp set: %s, virtual remote active\n", prop.c_str());
        std::fflush(stdout);

        maxhal::EventBus bus;
        MaxEvent ev{};
        while (bus.next_event(-1, ev)) {
            if (ev.kind == MAX_EVENT_STOP) {
                break;
            }
        }

        std::printf("[pkg-cpp] MAX_EVENT_STOP received, C++ RAII destructors rolling back SY6045S & uinput.\n");
    } catch (const std::exception& e) {
        std::fprintf(stderr, "[pkg-cpp] error: %s\n", e.what());
        return 1;
    }
    return 0;
}
