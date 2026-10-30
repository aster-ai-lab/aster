"""Prepare a default-off exact AVX2 FP8/exponent translator, preserving every model read."""
import difflib
import hashlib
import json
import math
import struct
from xolotl_trial_common import BASE

root = BASE / 'engram-fp8-simd-v136'
original = (BASE / 'engram-fp8-lut-v72/ds41_engram.original.cpp').read_bytes()
if hashlib.sha256(original).hexdigest() != 'f416e336beb9bba744ebd43753172ec1f3dcbc019f19e944a44171fe2d3cb42b':
    raise RuntimeError('pinned original Engram source differs')
source = original.decode().replace('\r\n', '\n')
start = source.index('float fp8_e4m3(uint8_t b) {')
end = source.index('\n}\n', start) + 3
reference = source[start:end].replace('fp8_e4m3(', 'reference_fp8(', 1)
values = []
for byte in range(256):
    sign, exponent, mantissa = byte >> 7, (byte >> 3) & 15, byte & 7
    if exponent == 15 and mantissa == 7:
        bits = (sign << 31) | 0x7fc00000
    else:
        value = math.ldexp(mantissa, -9) if exponent == 0 else math.ldexp(1 + mantissa / 8, exponent - 7)
        if sign:
            value = -value
        bits = struct.unpack('<I', struct.pack('<f', value))[0]
    values.append(f'0x{bits:08x}u')
table = '\n'.join('        ' + ', '.join(values[i:i+8]) + ',' for i in range(0, 256, 8))
header = (root / 'aster_engram_fp8_simd.hpp.in').read_text().replace('@TABLE@', table)
test = (root / 'fp8_simd_exact_test.cpp.in').read_text().replace('@REFERENCE@', reference)
anchor = '#include "ds41_sys.hpp"'
if source.count(anchor) != 1:
    raise RuntimeError('exact original Engram include differs')
source = source.replace(anchor, anchor + '''
#ifndef ASTER_XQ_ENGRAM_FP8_SIMD
#define ASTER_XQ_ENGRAM_FP8_SIMD 0
#endif
#if ASTER_XQ_ENGRAM_FP8_SIMD != 0 && ASTER_XQ_ENGRAM_FP8_SIMD != 1
#error "ASTER_XQ_ENGRAM_FP8_SIMD must be 0 or 1"
#endif
#if ASTER_XQ_ENGRAM_FP8_SIMD
#include "aster_engram_fp8_simd.hpp"
#endif''')
replacements = [
    ('                const float sc = std::ldexp(1.0f, (int) s[q] - 127);\n                for (int64_t i = 0; i < 32; ++i) o[q * 32 + i] = fp8_e4m3(w[q * 32 + i]) * sc;',
     '                aster_engram_fp8::dequantize32(w + q * 32, s[q], o + q * 32, fp8_e4m3);'),
    ('                            const float f = std::ldexp(1.0f, (int) sc[q] - 127);\n                            for (int64_t x = 0; x < 32; ++x) o0[q * 32 + x] = fp8_e4m3(H.w[q * 32 + x]) * f;',
     '                            aster_engram_fp8::dequantize32(H.w + q * 32, sc[q], o0 + q * 32, fp8_e4m3);'),
    ('                        const float sc = std::ldexp(1.0f, (int) s[q] - 127);\n                        for (int64_t x = 0; x < 32; ++x) o0[q * 32 + x] = fp8_e4m3(w[q * 32 + x]) * sc;',
     '                        aster_engram_fp8::dequantize32(w + q * 32, s[q], o0 + q * 32, fp8_e4m3);')]
for before, after in replacements:
    if source.count(before) != 1:
        raise RuntimeError('exact original scalar row-decoder site differs')
    source = source.replace(before, '#if ASTER_XQ_ENGRAM_FP8_SIMD\n' + after + '\n#else\n' + before + '\n#endif')
files = {'ds41_engram.cpp': source.encode(), 'ds41_engram.original.cpp': original,
         'aster_engram_fp8_simd.hpp': header.encode(), 'fp8_simd_exact_test.cpp': test.encode(),
         'engram-fp8-simd-v136.patch': ''.join(difflib.unified_diff(original.decode().replace('\r\n','\n').splitlines(True), source.splitlines(True),
             fromfile='a/tools/v41/engine/ds41_engram.cpp', tofile='b/tools/v41/engine/ds41_engram.cpp')).encode()}
for name, raw in files.items():
    with (root / name).open('xb') as output:
        output.write(raw)
manifest = dict(schema='aster-xolotl-engram-fp8-simd-v136',
    upstream_revision='e9d0826d71c59072cbff77d2a31dd51f27ab7a12',
    original_source_sha256=hashlib.sha256(original).hexdigest(), default_enabled=False,
    compile_flag='ASTER_XQ_ENGRAM_FP8_SIMD', closed_values=[0,1], cpu_isa='runtime AVX2 detection; original scalar fallback',
    normal_exact_scale_range=[10,246], arithmetic='Translate FP32 exponent bits only for normal products; preserve signed zeros/NaNs; original arithmetic otherwise',
    source_reads_changed=False, row_order_changed=False, model_weights_changed=False,
    gpu_qualified=False, model_speed_claimed=False,
    files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
(root / 'source-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps(dict(phase='prepared', files=len(files), gpu_executed=False)))
