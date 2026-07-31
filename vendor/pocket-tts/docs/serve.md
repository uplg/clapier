# Serve Command Documentation

The `serve` command starts an HTTP API server with a web interface for text-to-speech generation.

## Basic Usage

```bash
cargo run --release -p pocket-tts-cli -- serve
# or if installed:
pocket-tts serve
```

This starts a server on `http://localhost:8000` with the default voice.

> [!TIP]
> **Lite Build:** You can build a "lite" version of the server without the web interface assets by using `--no-default-features`. This is useful for environments where binary size is a concern or only the API is needed.
> ```bash
> cargo build --release -p pocket-tts-cli --no-default-features
> ```

## Command Options

- `--host HOST`: Host address to bind (default: `127.0.0.1`)
- `--port PORT`, `-p`: Port number (default: `8000`)
- `--voice VOICE`: Default voice for requests (default: `alba`)
- `--variant VARIANT`: Model variant (default: `b6369a24`)
- `--temperature FLOAT`: Sampling temperature (default: `0.7`)
- `--lsd-decode-steps INT`: LSD decode steps (default: `1`)
- `--eos-threshold FLOAT`: EOS threshold (default: `-4.0`)
- `--ui UI`: Web UI mode (`standard` or `wasm-experimental`, default: `standard`)

## Examples

### Basic Server

```bash
# Start with defaults
pocket-tts serve

# Custom host and port
pocket-tts serve --host 0.0.0.0 --port 8080

# Custom default voice
pocket-tts serve --voice marius

# Experimental WASM web UI mode
pocket-tts serve --ui wasm-experimental --port 8080
```

## UI Modes

`serve` uses one React UI with two backends:

- `standard` (default): server-side generation via `/stream`
- `wasm-experimental`: browser-side generation via `WasmTTSModel`

If you run `--ui wasm-experimental`, ensure WASM artifacts are built first:

```powershell
.\scripts\build-wasm.ps1
```

```bash
./scripts/build-wasm.sh
```

## API Endpoints

### Health Check

```
GET /health
```

Returns server status:

```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

### Generate Audio (JSON)

```
POST /generate
Content-Type: application/json
```

Request body:

```json
{
  "text": "Hello, world!",
  "voice": "alba"
}
```

Response: WAV audio file

**Example:**

```bash
curl -X POST http://localhost:8000/generate \
  -H 'Content-Type: application/json' \
  -d '{"text": "Hello world", "voice": "alba"}' \
  --output output.wav
```

### Streaming Generation

```
POST /stream
Content-Type: application/json
```

Request body: Same as `/generate`

Response: Chunked audio stream (raw PCM, 16-bit, 24kHz, mono)

**Example:**

```bash
curl -X POST http://localhost:8000/stream \
  -H 'Content-Type: application/json' \
  -d '{"text": "Streaming audio generation"}' | \
  ffplay -f s16le -ar 24000 -ac 1 -nodisp -autoexit -
```

### Python API Compatibility

```
POST /tts
Content-Type: multipart/form-data
```

Form fields:
- `text`: Text to synthesize
- `voice`: Voice name or file

Response: WAV audio file

This endpoint maintains compatibility with the Python server's multipart form API.

### OpenAI Compatibility

```
POST /v1/audio/speech
Content-Type: application/json
```

Request body:

```json
{
  "model": "pocket-tts",
  "input": "Hello, world!",
  "voice": "alba"
}
```

Response: Audio file (WAV format)

This endpoint is compatible with OpenAI's text-to-speech API format.

## Web Interface

Navigate to `http://localhost:8000` to access the built-in web interface:

- Text input field
- Voice selection dropdown
- Generate button
- Audio playback
- Download link

## Voice Options

Voices can be specified as:

1. **Predefined name**: `alba`, `marius`, `javert`, `jean`, `fantine`, `cosette`, `eponine`, `azelma`
2. **Base64 audio**: Include audio data directly in the request

## Response Formats

| Endpoint | Content-Type | Format |
|----------|--------------|--------|
| `/generate` | `audio/wav` | Complete WAV file |
| `/stream` | `application/octet-stream` | Raw PCM chunks |
| `/tts` | `audio/wav` | Complete WAV file |
| `/v1/audio/speech` | `audio/wav` | Complete WAV file |

## Error Handling

Errors return JSON with status code:

```json
{
  "error": "Error message here"
}
```

| Status | Meaning |
|--------|---------|
| 200 | Success |
| 400 | Bad request (invalid input) |
| 500 | Server error |

## Performance Notes

- Model is loaded once at startup and kept in memory
- Voice states are resolved per-request (consider caching on client side)
- Streaming endpoint provides lower latency for first audio

## Manual Verification and TTFA

Use these templates:

- `manual-verification/wasm-ui-verification-template.md`
- `manual-verification/perf-ttfa-template.md`

Recommended TTFA evaluation:

1. Warm server startup and voice cache.
2. Run at least 30 short-prompt measurements.
3. Capture:
   - TTFC (time to first chunk)
   - TTFA (time to first audible audio)
   - Total generation time
4. Pass criterion:
   - TTFA max must be `<= 600ms` for warmed local short-prompt runs.

## See Also

- [Generate Command](generate.md) - CLI generation
- [Rust API](rust-api.md) - Library integration
