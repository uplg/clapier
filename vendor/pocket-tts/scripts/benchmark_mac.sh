#!/bin/bash

# Run Mac-specific Benchmarks: Python vs Rust (Metal)
# Requirements: hyperfine, cargo, uv

# Check for hyperfine
if ! command -v hyperfine &> /dev/null; then
    echo "Error: hyperfine not found in PATH. Please install it: brew install hyperfine"
    exit 1
fi

# Check for uv
if ! command -v uv &> /dev/null; then
    echo "Error: uv not found in PATH. Please install it: curl -LsSf https://astral.sh/uv/install.sh | sh"
    exit 1
fi

# Determine Rust binary path
if [ -f "./target/release/pocket-tts-cli" ]; then
    RS_BIN="./target/release/pocket-tts-cli"
    echo "Using local build binary: $RS_BIN"
elif command -v pocket-tts-cli &> /dev/null; then
    RS_BIN=$(command -v pocket-tts-cli)
    echo "Using installed binary: $RS_BIN"
else
    echo "Error: pocket-tts-cli not found locally or in PATH."
    echo "Please build it with: cargo build --release --features metal"
    echo "Or install it with: cargo install pocket-tts-cli --features metal"
    exit 1
fi

TEXTS=(
    "Hello world"
    "This is a medium length sentence for benchmarking on Mac with Metal."
    "The sun was beginning to set over the horizon, casting a warm golden glow across the quiet valley. A gentle breeze rustled the leaves of the old oak trees, carrying the sweet scent of blooming wildflowers. In the distance, the faint sound of a rushing stream provided a soothing backdrop to the peaceful evening."
    "Artificial intelligence is rapidly transforming the way we interact with technology and each other. From advanced natural language processing to sophisticated image recognition, these systems are becoming increasingly integrated into our daily lives. As we continue to develop and refine these models, it is crucial to consider the ethical implications and ensure that they are used for the benefit of all humanity. The future of AI holds great promise, but it also requires careful stewardship and a commitment to transparency and accountability."
)

# Setup temporary and final output paths
TMP_DIR=".bench_tmp"
mkdir -p "$TMP_DIR"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
FINAL_OUTPUT="benchmark_results_mac_$TIMESTAMP.md"

for i in "${!TEXTS[@]}"; do
    text="${TEXTS[$i]}"
    echo -e "\n=== Benchmark $((i + 1)): ${#text} chars ==="
    
    # We use uvx to run the python reference directly
    PY_CMD="uvx pocket-tts generate --text \"$text\" --output-path bench_py.wav"
    # Rust with Metal acceleration
    RS_CMD="$RS_BIN generate --text \"$text\" --output bench_rs.wav --use-metal"
    
    EXPORT_FILE="$TMP_DIR/results_$((i + 1)).md"
    
    hyperfine --warmup 1 --runs 3 \
        --command-name "Python (uvx)" "$PY_CMD" \
        --command-name "Rust (Metal)" "$RS_CMD" \
        --export-markdown "$EXPORT_FILE"
done

echo -e "\nConcatenating results into $FINAL_OUTPUT..."
echo "# Mac Benchmark Results (Metal) - $TIMESTAMP" > "$FINAL_OUTPUT"
echo "" >> "$FINAL_OUTPUT"

for f in "$TMP_DIR"/results_*.md; do
    cat "$f" >> "$FINAL_OUTPUT"
    echo -e "\n---\n" >> "$FINAL_OUTPUT"
done

echo "Cleaning up..."
rm -rf "$TMP_DIR"

echo "Done! Results saved to $FINAL_OUTPUT"
