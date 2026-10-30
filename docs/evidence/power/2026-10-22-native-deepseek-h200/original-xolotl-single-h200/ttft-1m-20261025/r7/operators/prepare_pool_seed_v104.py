"""Extend the existing bounded pool's auxiliary blobs; never create a separate unbounded seed cache."""
import difflib
import hashlib
import json
from pathlib import Path

B = Path(__file__).resolve().parent
R = B / 'pool-seed-v104'


def one(s, old, new):
    if s.count(old) != 1:
        raise RuntimeError('exact source anchor differs: ' + old[:120])
    return s.replace(old, new)


original = (R / 'ds41_conv_reuse.hpp.original').read_text()
s = original
s = one(s, '    bool live_clobbered = false;///< a failure after the device was written: the live state is garbage',
    '    std::vector<std::vector<uint8_t>> blobs; ///< optional boundary metadata, owned by the same budgeted record\n'
    '    bool live_clobbered = false;///< a failure after the device was written: the live state is garbage')
signature = '                 const float * logits, size_t n_logits, std::string & err) {'
if s.count(signature) != 2:
    raise RuntimeError('exact host/pinned save signatures differ')
s = s.replace(signature, '                 const float * logits, size_t n_logits, std::string & err,\n'
    '                 const std::vector<std::vector<uint8_t>> & blobs = {}) {')
s = one(s, '        size_t need = r.tokens.size() * 4 + r.ident.size() * 8 + r.logits.size() * 4;',
    '        size_t need = r.tokens.size() * 4 + r.ident.size() * 8 + r.logits.size() * 4;\n'
    '        for (const auto & blob : blobs) {\n'
    '            if (blob.size() > budget_ || need > budget_ - blob.size()) { err = "pool metadata exceeds budget"; return 0; }\n'
    '            need += blob.size();\n'
    '        }')
s = one(s, '        // supersede: a record this one extends holds nothing it does not (positions [0, its n) are the same)',
    '        r.blobs = blobs;\n'
    '        // supersede: a record this one extends holds nothing it does not (positions [0, its n) are the same)')
s = one(s, '        out.ident = it->ident;', '        out.ident = it->ident;\n        out.blobs = it->blobs;')
s = one(s, '        std::vector<float> logits;\n        struct Region',
    '        std::vector<float> logits;\n        std::vector<std::vector<uint8_t>> blobs;\n        struct Region')
s = one(s, '        return (int64_t) pool_.save(snap_, reinterpret_cast<const int32_t *>(tokens), ids, n, logits, n_logits, err);',
    '        Ds41PoolSaveOpts opts;\n'
    '        for (const auto & blob : blobs) opts.blobs.push_back({blob.data(), blob.size()});\n'
    '        return (int64_t) pool_.save(snap_, reinterpret_cast<const int32_t *>(tokens), ids, n, logits, n_logits, err, opts);')
s = one(s, '        out.logits = std::move(rr.logits);',
    '        out.logits = std::move(rr.logits);\n        out.blobs = std::move(rr.blobs);')
s = one(s, '    int64_t save(Ds41Dense & d, const int * t, const uint64_t * id, size_t idn, int64_t n, const float * lg, size_t nl, std::string & err) {\n        return DS41_STORE_CALL(save(d, t, id, idn, n, lg, nl, err));',
    '    int64_t save(Ds41Dense & d, const int * t, const uint64_t * id, size_t idn, int64_t n, const float * lg, size_t nl, std::string & err,\n'
    '                 const std::vector<std::vector<uint8_t>> & blobs = {}) {\n'
    '        return DS41_STORE_CALL(save(d, t, id, idn, n, lg, nl, err, blobs));')
with (R / 'ds41_conv_reuse.hpp').open('x') as f:
    f.write(s)
patch = ''.join(difflib.unified_diff(original.splitlines(True), s.splitlines(True),
    fromfile='a/tools/v41/engine/ds41_conv_reuse.hpp', tofile='b/tools/v41/engine/ds41_conv_reuse.hpp'))

original = (B / 'admission-seed-v93/ds41_generate.cpp').read_text()
s = original
s = one(s, '#include "aster_draft_cumulative.hpp"', '#include "aster_draft_cumulative.hpp"\n#include "aster_dspark_seed.hpp"')
s = one(s, '    bool prefill_qos = false;',
    '    bool pool_draft_seed = false;\n'
    '    if (!aster_prefill::qos_value(std::getenv("ASTER_XQ_POOL_DRAFT_SEED"), pool_draft_seed)) {\n'
    '        std::fprintf(stderr, "ASTER_XQ_POOL_DRAFT_SEED must be 0 or 1\\n"); return 2;\n'
    '    }\n    bool prefill_qos = false;')
s = one(s, '        // the live conversation\'s turn boundary into the pool (logged as serve/server.py ConvCacheLog reads it)',
    '        auto seed_blobs = [&](const std::vector<float> & seed, int rows, int position) {\n'
    '            aster_dspark_seed::Blobs blobs;\n'
    '            if (pool_draft_seed && rows > 0 && !aster_dspark_seed::encode(seed, rows, (int) n_embd, position, blobs))\n'
    '                std::fprintf(stderr, "serve: DSpark seed metadata refused at save; ordinary decode after restore\\n");\n'
    '            return blobs;\n'
    '        };\n'
    '        // the live conversation\'s turn boundary into the pool (logged as serve/server.py ConvCacheLog reads it)')
s = one(s, 'snap_logits.empty() ? nullptr : snap_logits.data(), snap_logits.size(), perr);',
    'snap_logits.empty() ? nullptr : snap_logits.data(), snap_logits.size(), perr, seed_blobs(snap_seed, snap_seed_n, snap_len));')
s = one(s, 'c.snap_logits.empty() ? nullptr : c.snap_logits.data(), c.snap_logits.size(), perr);',
    'c.snap_logits.empty() ? nullptr : c.snap_logits.data(), c.snap_logits.size(), perr, seed_blobs(c.snap_seed, c.snap_seed_n, c.snap_len));')
s = one(s, '                        snap_seed_n = 0;   // no DSpark seed rows in a record: a repeat decodes its first tokens draftless',
    '                        snap_seed.clear(); snap_seed_n = 0;\n'
    '                        if (pool_draft_seed && pr == rs.pos) {\n'
    '                            if (!aster_dspark_seed::decode(rs.blobs, (int) n_embd, rs.pos, snap_seed, snap_seed_n))\n'
    '                                std::fprintf(stderr, "serve: DSpark seed metadata refused at restore; ordinary decode\\n");\n'
    '                            else if (snap_seed_n > 0)\n'
    '                                std::fprintf(stderr, "serve: conversation cache: DSpark seed restored %d rows at %lld\\n", snap_seed_n, (long long) rs.pos);\n'
    '                        }')
with (R / 'ds41_generate.cpp').open('x') as f:
    f.write(s)
patch += ''.join(difflib.unified_diff(original.splitlines(True), s.splitlines(True),
    fromfile='a/tools/v41/engine/ds41_generate.cpp', tofile='b/tools/v41/engine/ds41_generate.cpp'))
with (R / 'pool-seed-v104.patch').open('x') as f:
    f.write(patch)
for name in ['aster_prefill_policy.hpp','aster_draft_cumulative.hpp']:
    with (R / name).open('xb') as f:
        f.write((B / 'admission-seed-v93' / name).read_bytes())
for name in ['ds41_conv_pool.hpp','ds41_conv_pool.cpp','test_ds41_conv_pool.cpp']:
    with (R / name).open('xb') as f:
        f.write((R / (name + '.original')).read_bytes())
manifest = [dict(file=p.name, bytes=p.stat().st_size, sha256=hashlib.sha256(p.read_bytes()).hexdigest())
    for p in sorted(R.iterdir()) if p.is_file() and p.name != 'source-manifest.json']
with (R / 'source-manifest.json').open('x') as f:
    f.write(json.dumps(dict(parent='admission-seed-v93 generator/c96 native', files=manifest,
        opt_in_flag='ASTER_XQ_POOL_DRAFT_SEED=0|1', default_enabled=False,
        pool_budget_accounts_metadata=True, partial_boundary_seed_reuse=False,
        gpu_qualified=False, performance_claimed=False), indent=2) + '\n')
print(json.dumps(dict(phase='prepared', files=len(manifest), gpu_executed=False)))
