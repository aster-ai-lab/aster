#include "aster_serve_profile.hpp"
#include <cstdio>
#include <stdexcept>
#include <type_traits>
#include <vector>
static std::vector<bool> calls;
static void callback(bool on) { calls.push_back(on); }
static unsigned cases = 0;
static void check(bool value) { ++cases; if (!value) throw std::runtime_error("profile lifecycle contract"); }
static void early_return(bool enabled) { aster_profile::DecodeWindow window(enabled, callback); }
int main() {
    bool enabled = true;
    check(aster_profile::flag(nullptr, enabled) && !enabled);
    check(aster_profile::flag("0", enabled) && !enabled);
    check(aster_profile::flag("1", enabled) && enabled);
    for (const char * value : {"", "2", "true", " 1", "1 ", "01", "-1", "false"})
        check(!aster_profile::flag(value, enabled));
    aster_profile::FourthGen off(false), on(true);
    for (unsigned index = 1; index <= 10000; ++index) {
        check(!off.next());
        check(on.next() == (index == 4));
    }
    early_return(false);check(calls.empty());
    early_return(true);check(calls == std::vector<bool>({true,false}));
    calls.clear();
    { aster_profile::DecodeWindow window(true,callback);window.finish();window.finish(); }
    check(calls == std::vector<bool>({true,false}));
    calls.clear();
    try { aster_profile::DecodeWindow window(true,callback);throw std::runtime_error("early failure"); }
    catch (const std::runtime_error &) {}
    check(calls == std::vector<bool>({true,false}));
    static_assert(!std::is_copy_constructible_v<aster_profile::DecodeWindow>);
    static_assert(!std::is_move_constructible_v<aster_profile::DecodeWindow>);
    std::printf("{\"phase\":\"passed\",\"contracts\":%u,\"gpu_executed\":false}\n", cases);
}
