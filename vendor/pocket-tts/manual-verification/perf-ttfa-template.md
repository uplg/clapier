# TTFA Performance Template

Date:
Tester:
Commit/Branch:
Hardware:
OS:
Browser (for UI tests):

## Goal
Maintain `TTFA <= 600ms` (max) for warmed local short-prompt runs.

Definitions:
- TTFC: Time from Generate click/request start to first received audio chunk.
- TTFA: Time from Generate click/request start to first audible sample from the AudioWorklet.
- Total: Time from Generate click/request start to stream completion.

## Test Conditions
- Release build only (`--release`)
- Model warmed (startup warmup complete)
- Voice warmed (use default `alba` unless otherwise specified)
- Prompt type: short sentence (~5-12 words)
- Runs per mode: at least 30

## Commands
Standard mode:
`cargo run --release -p pocket-tts-cli -- serve`

WASM mode:
`cargo run --release -p pocket-tts-cli -- serve --ui wasm-experimental --port 8080`

## Run Log
| Run | Mode | Prompt | TTFC (ms) | TTFA (ms) | Total (ms) | Pass (`TTFA <= 600`) |
|-----|------|--------|-----------|-----------|------------|----------------------|
| 1 | standard | | | | | |
| 2 | standard | | | | | |
| ... | ... | | | | | |
| 30 | standard | | | | | |
| 1 | wasm-experimental | | | | | |
| 2 | wasm-experimental | | | | | |
| ... | ... | | | | | |
| 30 | wasm-experimental | | | | | |

## Summary Statistics
Standard mode:
- TTFC min/p50/p95/max:
- TTFA min/p50/p95/max:
- Total min/p50/p95/max:

WASM mode:
- TTFC min/p50/p95/max:
- TTFA min/p50/p95/max:
- Total min/p50/p95/max:

## Gate Result
- Standard mode TTFA max <= 600ms: [ ] Yes [ ] No
- WASM mode TTFA max <= 600ms: [ ] Yes [ ] No

Overall:
- [ ] PASS
- [ ] FAIL

## Notes
