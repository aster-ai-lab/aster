# Retain single-H200 deployment and recover request availability (R14)

The user stopped further performance optimization. Both Windows optimization
controllers were stopped after checking executable, command line and process
creation identity. The exact C15 job and replica reached Stopped. The C14
formal job, VIP, key, model weights, engine binary and 1M configuration remain
in use. No further optimization GPU or profiler is scheduled.

The retained C15 graph-type check measured74.348 token/s on the short-input
8x1024 API test versus75.294 control: about1.26 percent lower. Its actual1M
comparison was interrupted by user instruction; no speed improvement or
experimental engine promotion is claimed.

The reported WorkBuddy wait included a672888-token read and later queued
requests. Later observations found the API still reporting reading while
native CPU, IO and GPU activity were idle. An attempted exact-native manual
recovery returned an error; the native identity remained unchanged afterwards.
No successful manual restart, exact underlying race diagnosis, or proof that
all of the wait was productive prefill is claimed. The old requests later
left and the service became idle.

Two actual installed WorkBuddy CLI sessions then sent exactly HI with the
default tool definitions and no persisted conversation. Both completed with
the greeting and no tool use. Each carried24870 input tokens despite the
two-character user input. Cold first output was47.859 seconds, with43.297
seconds of server prompt read. The next independent session reused24870
tokens, took0.849 seconds inside the client API call and4.531 seconds to
first client output including startup. CLI timings are not GUI timings or
GPU token-generation benchmarks.

A bounded availability guard now runs as one owned process inside the same
C14 pod. It references the existing protected credential file rather than
copying it. It changes no model setting or GPU allocation. Recovery requires
an active request with90 seconds of no CPU, IO, GPU or prompt/output progress,
then an immediate identity and activity recheck. It signals only the exact
supervised API process group and uses the existing bounded restart mechanism.
Telemetry errors or changing process generations reset the decision window.
There are at most two recoveries, constrained by the existing supervisor
budget and resource lease. The actual fresh guard observation reports zero
errors, zero recoveries and an idle service.

Validation: python -B -m unittest test_stall_window_v415 in helpers/.
Ten methods pass on Windows and in the actual Linux deployment. Cases cover
idle services, genuine CPU/GPU/IO work, prompt progress, generation changes,
missing observations, invalid counters and the full inactivity interval.

## Explicitly not claimed

WorkBuddy GUI end-to-end acceptance, automatic recovery exercised against a
real forced production stall, instantaneous cold HI response, root-cause race
elimination, 300/1000 token/s, new actual1M input acceptance, or performance
goal completion. Performance optimization remains cancelled by the user.
No credentials, raw user sessions, private prompt bodies or model weights
are included. Rust product sources were not edited and Cargo was not run.
