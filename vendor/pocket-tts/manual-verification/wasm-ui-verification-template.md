# WASM UI Manual Verification Template

Date:
Tester:
Commit/Branch:
Hardware:
OS/Browser:

## Setup
- Build web assets: `cd crates/pocket-tts-cli/web && bun run build`
- Build WASM assets: `./scripts/build-wasm.sh` or `./scripts/build-wasm.ps1`
- Standard UI command: `cargo run --release -p pocket-tts-cli -- serve`
- WASM UI command: `cargo run --release -p pocket-tts-cli -- serve --ui wasm-experimental --port 8080`

## Standard UI Checks (`http://localhost:8000`)
- [ ] App loads and renders main controls
- [ ] Generate -> playback works
- [ ] Stop works during buffering and during playback
- [ ] Buffer status transitions are sensible (`buffering -> playing -> finished`)
- [ ] Preset voice switch works (e.g. `alba` -> `marius`)
- [ ] Voice clone WAV upload works
- [ ] Download WAV produces valid output file

## WASM UI Checks (`http://localhost:8080`)
- [ ] WASM initialization stages are visible and ordered
- [ ] HF repo/token inputs are visible
- [ ] Manual override section is collapsed by default
- [ ] Preset voice load works
- [ ] WAV cloning works
- [ ] Safetensors embedding upload works
- [ ] Generate -> playback works
- [ ] Stop works during buffering and during playback
- [ ] Download WAV produces valid output file

## HF Loading UX Cases
- [ ] Missing token for gated repo shows clear actionable error
- [ ] Invalid token shows clear actionable error
- [ ] Retry after failure works without page refresh
- [ ] Successful init reports source (`local`, `hf`, or `manual`)

## Notes / Bugs
- Issue:
  - Steps:
  - Expected:
  - Actual:
  - Severity:

## Result
- [ ] PASS
- [ ] FAIL
