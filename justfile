# The burrow's everyday recipes; `just` alone lists them.

default:
    @just --list

# Build the server workspace, release profile
build:
    cargo build --release

# Exactly what CI checks: format, lints, tests
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test

# Licenses, advisories and dependency sources
deny:
    cargo deny check

# Supply-chain audits: every dependency audited or knowingly exempted
vet:
    cargo vet

# The garenne golden-frame suite in the simulator
garenne-test:
    ./garenne/build.sh test

# Device build of the rabbit's bytecode -> garenne/build/garenne.bin
garenne:
    ./garenne/build.sh

# Deploy garenne into a rabbit's burrow (mac like 00:19:db:9c:28:15)
deploy mac:
    ./scripts/deploy-garenne.sh --rabbit {{mac}}

# Serve the local overlay on :8080 (no root needed) for a quick look
serve:
    cargo run --release -p clapier -- --bind 0.0.0.0:8080 --overlay garenne/overlay

# Make the rabbit speak (builds nothing; wants pocket-tts built once)
say text:
    ./scripts/rabbit-say.sh "{{text}}"

# Cut a release: clean tree, bump, tag, push; CI builds and publishes
release version:
    #!/usr/bin/env bash
    set -euo pipefail
    test -z "$(git status --porcelain)" || { echo "working tree not clean" >&2; exit 1; }
    grep -q '^## \[Unreleased\]' CHANGELOG.md || { echo "no Unreleased section in CHANGELOG.md" >&2; exit 1; }
    sed -i '' 's/^version = ".*"$/version = "{{version}}"/' Cargo.toml
    cargo build --quiet
    git add Cargo.toml Cargo.lock
    git commit -m "release: v{{version}}"
    git tag -a "v{{version}}" -m "clapier v{{version}}"
    git push origin main "v{{version}}"
    echo "v{{version}} pushed; CI takes it from here"

# One command to the rabbit's control port (just ctl ping, just ctl color 7c5cff)
ctl *args:
    cargo run --release -q -p clapier --bin garenne-ctl -- {{args}}

# Watch the rabbits' log broadcasts, timestamped
listen:
    cargo run --release -q -p clapier --bin garenne-ctl -- listen
