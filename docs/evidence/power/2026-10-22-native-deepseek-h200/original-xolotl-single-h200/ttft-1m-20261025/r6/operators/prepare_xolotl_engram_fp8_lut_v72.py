"""Prepare an opt-in exact FP8 table, tied to the pinned original Engram source."""
import difflib
import hashlib
import json
import math
from pathlib import Path
from xolotl_trial_common import BASE

root = BASE / 'engram-fp8-lut-v72'
root.mkdir(exist_ok=False)
source = BASE / 'xolotl-trial-source-e9d0826d71c5/source/tools/v41/engine/ds41_engram.cpp'
raw = source.read_bytes()
text = raw.decode()
anchor = 'float fp8_e4m3(uint8_t b) {'
if text.count(anchor)!=1:
    raise RuntimeError('exact upstream FP8 decoder anchor differs')
begin = text.index(anchor)
end = text.index('\n}\n', begin)+3
original = text[begin:end]
values = []
for byte in range(256):
    sign, exponent, mantissa = byte>>7, (byte>>3)&15, byte&7
    if exponent==15 and mantissa==7:
        value = '-NAN' if sign else 'NAN'
    else:
        number = math.ldexp(mantissa, -9) if exponent==0 else math.ldexp(1+mantissa/8, exponent-7)
        if sign:
            number = -number
        value = number.hex()+'f'
    values.append(value)
rows = ['        '+', '.join(values[index:index+8])+',' for index in range(0,256,8)]
candidate = '''#ifndef ASTER_XQ_ENGRAM_FP8_LUT
#define ASTER_XQ_ENGRAM_FP8_LUT 0
#endif
#if ASTER_XQ_ENGRAM_FP8_LUT != 0 && ASTER_XQ_ENGRAM_FP8_LUT != 1
#error "ASTER_XQ_ENGRAM_FP8_LUT must be 0 or 1"
#endif
float fp8_e4m3(uint8_t b) {
#if ASTER_XQ_ENGRAM_FP8_LUT
    // All 256 FP8 encodings, including signed zero and the signed NaNs.
    // Keep the original scale multiplication and its ordering at each caller.
    static constexpr float table[256] = {
@TABLE@
    };
    return table[b];
#else
@ORIGINAL_BODY@
#endif
}
'''.replace('@TABLE@','\n'.join(rows)).replace('@ORIGINAL_BODY@',original.split('{',1)[1].rsplit('}',1)[0].strip('\n'))
patched = text[:begin]+candidate+text[end:]
unit = '''#include <cfenv>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
@REFERENCE@
@CANDIDATE@
static uint32_t bits(float value) { uint32_t result; std::memcpy(&result,&value,4); return result; }
int main() {
    uint64_t conversions=0, scaled=0;
    const int modes[]={FE_TONEAREST,FE_TOWARDZERO,FE_UPWARD,FE_DOWNWARD};
    for (int mode:modes) {
        if (std::fesetround(mode)!=0) return 2;
        for (int byte=0;byte<256;++byte) {
            const float reference=reference_fp8((uint8_t)byte), candidate=fp8_e4m3((uint8_t)byte);
            if (bits(reference)!=bits(candidate)) { std::fprintf(stderr,"encoding mismatch mode%d byte%d\\n",mode,byte); return 3; }
            ++conversions;
            for (int scale=0;scale<256;++scale) {
                const float factor=std::ldexp(1.0f,scale-127);
                volatile float a=reference*factor, b=candidate*factor;
                if (bits(a)!=bits(b)) { std::fprintf(stderr,"scaled mismatch mode%d byte%d scale%d\\n",mode,byte,scale); return 4; }
                ++scaled;
            }
        }
    }
    std::fesetround(FE_TONEAREST);
    std::printf("{\\\"phase\\\":\\\"passed\\\",\\\"conversion_cases\\\":%llu,\\\"scaled_cases\\\":%llu,\\\"rounding_modes\\\":4,\\\"bitwise_equal\\\":true}\\n",(unsigned long long)conversions,(unsigned long long)scaled);
    return 0;
}
'''.replace('@REFERENCE@',original.replace('fp8_e4m3(', 'reference_fp8(',1)).replace('@CANDIDATE@',candidate)
files = {'ds41_engram.original.cpp':raw,'ds41_engram.cpp':patched.encode(),'fp8_exact_test.cpp':unit.encode(),
         'engram-fp8-lut-v72.patch':''.join(difflib.unified_diff(text.splitlines(True),patched.splitlines(True),fromfile='a/tools/v41/engine/ds41_engram.cpp',tofile='b/tools/v41/engine/ds41_engram.cpp')).encode()}
for name,value in files.items():
    (root/name).write_bytes(value)
manifest=dict(schema='aster-xolotl-engram-fp8-lut-v72', upstream_revision='e9d0826d71c59072cbff77d2a31dd51f27ab7a12',
              default_enabled=False, native_compiled=False, gpu_qualified=False, performance_claimed=False,
              files=[dict(file=name,bytes=len(value),sha256=hashlib.sha256(value).hexdigest()) for name,value in files.items()])
(root/'source-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(manifest,indent=2))
