#include <cfenv>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>
#include "aster_engram_fp8_simd.hpp"

float reference_fp8(uint8_t b) {
    const int s = b >> 7, e = (b >> 3) & 15, m = b & 7;
    float v;
    if (e == 0) v = std::ldexp((float) m, -9);                 // subnormal: m/8 * 2^-6
    else if (e == 15 && m == 7) v = NAN;
    else v = std::ldexp(1.0f + m / 8.0f, e - 7);
    return s ? -v : v;
}


static uint32_t bits(float value) {
    uint32_t result;
    std::memcpy(&result, &value, 4);
    return result;
}

template<bool Candidate>
__attribute__((noinline)) static void run_rows(const uint8_t * w, const uint8_t * scales, float * out, int blocks) {
    for (int block = 0; block < blocks; ++block) {
        if constexpr (Candidate) {
            aster_engram_fp8::dequantize32(w + block * 32, scales[block], out + block * 32, reference_fp8);
        } else {
            const float factor = std::ldexp(1.0f, static_cast<int>(scales[block]) - 127);
            for (int i = 0; i < 32; ++i) out[block * 32 + i] = reference_fp8(w[block * 32 + i]) * factor;
        }
    }
}

int main() {
    const int modes[] = {FE_TONEAREST, FE_TOWARDZERO, FE_UPWARD, FE_DOWNWARD};
    uint64_t cases = 0, normal_cases = 0, fallback_cases = 0;
    const int offsets[] = {0, 1, 7};
    alignas(32) uint8_t bytes[64] = {};
    alignas(32) float output[64] = {};
    for (int mode : modes) {
        if (std::fesetround(mode) != 0) return 2;
        for (int scale = 0; scale < 256; ++scale) {
            for (int first = 0; first < 256; first += 32) {
                for (int offset : offsets) {
                    for (int i = 0; i < 32; ++i) bytes[offset + i] = static_cast<uint8_t>(first + i);
                    const float factor = std::ldexp(1.0f, scale - 127);
                    aster_engram_fp8::dequantize32(bytes + offset, static_cast<uint8_t>(scale), output + offset, reference_fp8);
                    for (int i = 0; i < 32; ++i) {
                        volatile float reference = reference_fp8(bytes[offset + i]) * factor;
                        if (bits(reference) != bits(output[offset + i])) {
                            std::fprintf(stderr, "mismatch mode=%d byte=%d scale=%d offset=%d expected=%08x actual=%08x\n",
                                         mode, first + i, scale, offset, bits(reference), bits(output[offset + i]));
                            return 3;
                        }
                        ++cases;
                        if (scale >= 10 && scale <= 246 && aster_engram_fp8::avx2_enabled()) ++normal_cases;
                        else ++fallback_cases;
                    }
                }
            }
        }
    }
    if (std::fesetround(FE_TONEAREST) != 0) return 4;
    constexpr int blocks = 65536;
    std::vector<uint8_t> weights(blocks * 32), scales(blocks);
    std::vector<float> reference(blocks * 32), candidate(blocks * 32);
    uint32_t random = 731927;
    for (auto & value : weights) {
        random = 1664525u * random + 1013904223u;
        value = static_cast<uint8_t>(random >> 24);
        if ((value & 127) == 127) value = static_cast<uint8_t>(value - 1);
    }
    for (auto & value : scales) {
        random = 1664525u * random + 1013904223u;
        value = static_cast<uint8_t>(100 + (random % 40));
    }
    run_rows<false>(weights.data(), scales.data(), reference.data(), blocks);
    run_rows<true>(weights.data(), scales.data(), candidate.data(), blocks);
    double old_seconds[7] = {}, new_seconds[7] = {};
    volatile uint32_t checksum = 0;
    for (int round = 0; round < 7; ++round) {
        for (int order = 0; order < 2; ++order) {
            const bool fast = ((round + order) % 2) != 0;
            const auto begin = std::chrono::steady_clock::now();
            if (fast) run_rows<true>(weights.data(), scales.data(), candidate.data(), blocks);
            else run_rows<false>(weights.data(), scales.data(), reference.data(), blocks);
            const double seconds = std::chrono::duration<double>(std::chrono::steady_clock::now() - begin).count();
            if (fast) new_seconds[round] = seconds;
            else old_seconds[round] = seconds;
            checksum = checksum ^ bits(fast ? candidate[round * 997] : reference[round * 997]);
        }
        if (std::memcmp(reference.data(), candidate.data(), reference.size() * sizeof(float)) != 0) return 5;
    }
    std::printf("{\"phase\":\"passed\",\"scaled_value_cases\":%llu,\"normal_vector_cases\":%llu,\"scalar_fallback_cases\":%llu,\"rounding_modes\":4,\"input_output_offsets\":3,\"bitwise_equal\":true,\"avx2_enabled\":%s,\"model_executed\":false,\"benchmark_values_per_round\":%d,\"checksum\":%u,\"rounds\":[",
                static_cast<unsigned long long>(cases), static_cast<unsigned long long>(normal_cases),
                static_cast<unsigned long long>(fallback_cases), aster_engram_fp8::avx2_enabled() ? "true" : "false", blocks * 32, static_cast<unsigned>(checksum));
    for (int round = 0; round < 7; ++round) {
        std::printf("%s{\"reference_seconds\":%.9f,\"candidate_seconds\":%.9f}",
                    round ? "," : "", old_seconds[round], new_seconds[round]);
    }
    std::puts("]}");
    return 0;
}
