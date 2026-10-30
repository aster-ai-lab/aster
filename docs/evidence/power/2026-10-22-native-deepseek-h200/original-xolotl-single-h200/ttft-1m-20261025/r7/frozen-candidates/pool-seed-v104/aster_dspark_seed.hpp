// Optional DSpark prompt-tail metadata. Position, shape and finite payload are checked before use.
#pragma once
#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <utility>
#include <vector>

namespace aster_dspark_seed {
constexpr size_t header_bytes = 24;
constexpr size_t max_payload_bytes = 64 * 1024 * 1024;
constexpr uint8_t magic[8] = {'A', 'D', 'S', 'E', 'E', 'D', '0', '1'};
using Blobs = std::vector<std::vector<uint8_t>>;

inline void put(std::vector<uint8_t> & out, size_t offset, uint64_t value, size_t count) {
    for (size_t i = 0; i < count; ++i) out[offset + i] = (uint8_t) (value >> (8 * i));
}
inline uint64_t get(const std::vector<uint8_t> & in, size_t offset, size_t count) {
    uint64_t value = 0;
    for (size_t i = 0; i < count; ++i) value |= (uint64_t) in[offset + i] << (8 * i);
    return value;
}
inline bool shape(uint64_t rows, uint64_t width, uint64_t position, size_t & elements) {
    if (!rows || rows > 128 || !width || position < rows
        || width > max_payload_bytes / (rows * 3 * sizeof(float))) return false;
    elements = (size_t) (rows * 3 * width);
    return true;
}
inline bool encode(const std::vector<float> & seed, int rows, int width, int64_t position, Blobs & out) {
    out.clear();
    size_t elements = 0;
    if (rows <= 0 || width <= 0 || position < 0 || !shape(rows, width, position, elements)
        || seed.size() != elements || !std::all_of(seed.begin(), seed.end(), [](float f) { return std::isfinite(f); })) return false;
    std::vector<uint8_t> blob(header_bytes + elements * sizeof(float));
    std::copy_n(magic, sizeof(magic), blob.data());
    put(blob, 8, (uint32_t) width, 4); put(blob, 12, (uint32_t) rows, 4); put(blob, 16, (uint64_t) position, 8);
    std::memcpy(blob.data() + header_bytes, seed.data(), elements * sizeof(float));
    out.emplace_back(std::move(blob));
    return true;
}
inline bool decode(const Blobs & in, int width, int64_t position, std::vector<float> & seed, int & rows) {
    seed.clear(); rows = 0;
    if (in.empty()) return true;
    if (in.size() != 1 || width <= 0 || position < 0) return false;
    const auto & blob = in[0];
    if (blob.size() < header_bytes || !std::equal(magic, magic + sizeof(magic), blob.begin())) return false;
    const uint64_t saved_width = get(blob, 8, 4), saved_rows = get(blob, 12, 4), saved_pos = get(blob, 16, 8);
    size_t elements = 0;
    if (saved_width != (uint64_t) width || saved_pos != (uint64_t) position
        || !shape(saved_rows, saved_width, saved_pos, elements)
        || blob.size() != header_bytes + elements * sizeof(float)) return false;
    std::vector<float> checked(elements);
    std::memcpy(checked.data(), blob.data() + header_bytes, elements * sizeof(float));
    if (!std::all_of(checked.begin(), checked.end(), [](float f) { return std::isfinite(f); })) return false;
    seed.swap(checked); rows = (int) saved_rows;
    return true;
}
} // namespace aster_dspark_seed
