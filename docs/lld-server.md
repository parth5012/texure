# LLD: MCP server internals (surface C)

Authoritative design for `crates/texure-server`. Resolved 2026-11-01, LLD grilling round 9 (HITL ticket [LLD C: MCP server internals](https://github.com/parth5012/texure/issues/8), map 2). Change the contract doc FIRST, then this doc, then code. status: RESOLVED (item 6 & 7 of lld-outline).

---

## 1. MCP Transport & Protocol Architecture

### 1.1 SDK Choice: Hand-rolled Stdio JSON-RPC 2.0
texure-server implements a minimal, hand-rolled JSON-RPC 2.0 loop over standard I/O (stdin/stdout) using `tokio` asynchronous line buffering and `serde_json`.

#### Tradeoff Analysis & Alternatives Cut
| Dimension | Hand-rolled stdio (Selected) | `rmcp` SDK (v3.8.0) |
|---|---|---|
| **Binary Footprint** | Minimal (~4-6 MB static binary) | Larger (~15-20 MB due to heavy deps) |
| **Dependencies** | Lean: `tokio`, `serde`, `serde_json` | Heavy: `schemars`, `uuid`, `pastey`, macros |
| **Compile Speed** | Fast (~3-5s fresh build) | Slower with proc-macro expansions |
| **Target Hardware** | Optimal for 6GB RAM desktop constraint | Marginally higher memory consumption |
| **Surface Freeze** | Tailored to exactly 6 frozen tools | Generic framework overhead |
| **Protocol Control** | Total control over framing and line flushing | Bound to upstream crate abstraction |

- **Alternatives cut:**
  - `rmcp` (official Rust SDK): Rejected due to dependency weight, macro compile times, and unnecessary complexity for a statically frozen 6-tool surface.
  - HTTP/SSE or Streamable-HTTP transports: Rejected for v1; MCP hosts (OpenCode, Claude Code, Cursor) consume local servers directly over stdio (DECISIONS round 1).

### 1.2 Protocol Lifecycle
1. **Startup:** Reads stdin line-by-line using `tokio::io::BufReader`. All stdout emissions must be strict single-line JSON followed by `\n`. Stderr is reserved for debug logging (`tracing` or `eprintln!`).
2. **Handshake (`initialize`):**
   - Responds with `protocolVersion: "2024-11-05"`, server info (`name: "texure-server"`, `version: "0.1.0"`), and capabilities (`tools: {}`).
3. **Notification (`notifications/initialized`):** Acknowledged silently (no response required per JSON-RPC).
4. **Health (`ping`):** Responds with empty object `{}`.
5. **Tool Discovery (`tools/list`):** Emits exactly the 6 frozen tools: `device_tree`, `tap`, `type`, `keypress`, `screenshot`, `wait_for`.
6. **Execution (`tools/call`):** Dispatches to bridge HTTP client or WS waiter, formats tool response, and never panics.

---

## 2. TOML Configuration Schema

### 2.1 File Location
Standard OS config directory resolved via the `dirs` crate:
- **Linux:** `~/.config/texure/config.toml`
- **macOS:** `~/Library/Application Support/texure/config.toml`
- **Windows:** `%APPDATA%\texure\config.toml`

### 2.2 Schema Structure (Hybrid Top-Level + Profiles)
Supports direct top-level fields for single-device simplicity, while establishing a `[profiles]` table for future multi-device support without migration:

```toml
# Active / default target device configuration
url = "http://192.168.1.100:8765"
token = "a1b2c3d4e5f6..."
timeout_secs = 10

# Multi-device profile foundation (optional in v1)
active_profile = "default"

[profiles.default]
url = "http://192.168.1.100:8765"
token = "a1b2c3d4e5f6..."
timeout_secs = 10

[profiles.tablet]
url = "http://192.168.1.105:8765"
token = "9f8e7d6c5b4a..."
timeout_secs = 15
```

### 2.3 Configuration Resolution Order
1. **Environment Variables:** `TEXURE_URL`, `TEXURE_TOKEN`, `TEXURE_TIMEOUT` (highest precedence).
2. **Active Profile in TOML:** If `active_profile` is specified, uses `[profiles.<active_profile>]`.
3. **Top-level TOML keys:** Fallback for single-device setups (`url`, `token`, `timeout_secs`).
4. **Missing Configuration:** If neither env vars nor config file exists, tool calls respond with `NO_CONFIG` error (see §4).

---

## 3. Bridge Client & Reconnection Mechanics

### 3.1 Startup & Lazy Dial
- The MCP server daemon starts immediately and stays running even if the Android device is offline or powered off.
- The daemon **never** exits on connection failure.
- The initial HTTP connection to the bridge is made lazily on the first tool call.

### 3.2 Background WebSocket Loop & Backoff
- A background tokio task manages the WebSocket subscription to `ws://<phone-ip>:8765/ws`.
- **Drop handling:** When the connection drops, the supervisor task attempts reconnection with exponential backoff:
  - 1s → 5s → 15s (cap at 15s).
- **Incoming events:** Collects `{ "type": "tree_changed", "rev": N }` broadcasts and sends notifications over an internal `tokio::sync::broadcast` channel for `wait_for`.

### 3.3 Mid-Reconnect & Tool Call Behavior
- Mutating tools (`tap`, `type`, `keypress`), `device_tree`, and `screenshot` execute HTTP on-demand with a 3-second dial timeout.
- If HTTP fails (dial timeout or `ECONNREFUSED`), the tool immediately returns:
  ```
  OFFLINE: device unreachable (same LAN? app open? fg service alive?)
  ```
- **`wait_for` fallback:**
  - If WS is connected, `wait_for` awaits `tree_changed` signals matching the condition.
  - If WS is disconnected or flaps, `wait_for` falls back to polling `/ui-tree` every 500ms via HTTP.
  - If HTTP also fails during `wait_for`, it surfaces `OFFLINE`.
  - On timeout (default 10s, cap 30s), returns `{ "ok": false, "last_rev": N }` plus the fresh snapshot (non-error result).

---

## 4. Error Surface & Mapping

All errors conform to `docs/bridge-contract.md` §Error shape and `docs/behavior-spec.md` §2.3.

### 4.1 MCP Result Envelope
Every tool failure returns an MCP `CallToolResult` with `isError: true` containing a single text block:
```json
{
  "content": [
    {
      "type": "text",
      "text": "NODE_OUTDATED: tree refreshed since reference; re-run /ui-tree"
    }
  ],
  "isError": true
}
```
If `verbose: true` is supplied by the agent, the text content contains the full serialized JSON error payload from the bridge:
```json
{
  "error": {
    "code": "NODE_OUTDATED",
    "message": "tree refreshed since reference; re-run /ui-tree"
  },
  "http_status": 400,
  "bridge_url": "http://192.168.1.100:8765",
  "ws_connected": true
}
```

### 4.2 Error Code Mapping Table
Direct cross-reference with `docs/bridge-contract.md`:

| Error Code | Origin | HTTP Status | MCP `isError` | Formatted Single-Line Output |
|---|---|---|---|---|
| `NO_CONFIG` | Server | — | `true` | `NO_CONFIG: no config file or env vars; create ~/.config/texure/config.toml or set TEXURE_URL/TEXURE_TOKEN` |
| `OFFLINE` | Server | — | `true` | `OFFLINE: device unreachable; check same LAN, app running, and fg service alive` |
| `AUTH_FAILED` | Bridge | 401 | `true` | `AUTH_FAILED: token mismatch; re-paste fresh token from overlay` |
| `BUSY` | Bridge | 409 | `true` | `BUSY: second client during single-flight; wait for owner release or ask owner` |
| `NODE_OUTDATED` | Bridge | 400 | `true` | `NODE_OUTDATED: tree refreshed since reference; re-run device_tree` |
| `NODE_NOT_CLICKABLE` | Bridge | 400 | `true` | `NODE_NOT_CLICKABLE: tap onto non-clickable node; pick a clickable node from latest tree` |
| `NODE_NOT_EDITABLE` | Bridge | 400 | `true` | `NODE_NOT_EDITABLE: type into non-editable node; tap editable field first` |
| `NODE_NOT_FOUND` | Bridge | 400 | `true` | `NODE_NOT_FOUND: node id not found; ids only come from latest device_tree` |
| `INJECT_FAILED` | Bridge | 400 | `true` | `INJECT_FAILED: injection methods failed; re-fetch tree, retry once` |
| `CAPTURE_FAILED` | Bridge | 400 | `true` | `CAPTURE_FAILED: screenshot capture failed; foreground app` |
| `APP_NOT_FOREGROUND` | Bridge | 400 | `true` | `APP_NOT_FOREGROUND: mutation while backgrounded; bring app forward explicitly` |
| `SCREEN_OFF` | Bridge | 400 | `true` | `SCREEN_OFF: display off; wake and reopen app` |
| `DEVICE_LOCKED` | Bridge | 400 | `true` | `DEVICE_LOCKED: keyguard locked; unlock device first` |
| `HUMAN_STEERING` | Bridge | 409 | `true` | `HUMAN_STEERING: human touch detected mid-action; re-fetch device_tree to re-plan` |
| `BRIDGE_STOPPED` | Bridge | reset | `true` | `BRIDGE_STOPPED: bridge service was stopped mid-action; restart via overlay` |
| `PORT_IN_USE` | Bridge | — | `true` | `PORT_IN_USE: fixed port occupied on device; free port and restart` |

*Non-error row:*
- `WAIT_TIMEOUT`: Returns `isError: false` with JSON body `{ "ok": false, "last_rev": N, "tree": { ... } }` and re-plan hint.

---

## 5. Screenshot Plumbing

The `screenshot` tool fetches `GET /screenshot?scale={0.25|0.5|1.0}` (default 0.5) from the bridge:
1. Validates HTTP 200 and `image/png` header.
2. Encodes raw PNG response bytes into a Base64 string.
3. Constructs an MCP Image Content Block:
   ```json
   {
     "content": [
       {
         "type": "image",
         "data": "iVBORw0KGgoAAAANSUhEUgAA...",
         "mimeType": "image/png"
       }
     ],
     "isError": false
   }
   ```
4. If `verbose: true` is requested, appends an additional text block containing image metadata (bytes length, scale, rev).
5. If the bridge returns an error status (e.g. `SCREEN_OFF`, `CAPTURE_FAILED`), returns standard error block with `isError: true`.

---

## 6. Crate Layout & Dependency Graph Freeze

The crate workspace is finalized as follows:

```
texure/
├── Cargo.toml (workspace)
├── crates/
│   ├── texure-contract/    # Pure types, schemas, and error codes (zero internal deps)
│   ├── texure-server/      # MCP stdio server binary + bridge HTTP/WS client
│   ├── texure-blackbox/    # Protocol test harness and scenarios
│   └── texure-lint/        # Contract and scenario linter
├── android/
│   ├── bridge/             # In-app Android Bridge library (Kotlin)
│   ├── sample/             # Verification sample application
│   └── gradle-plugin/      # Debug-variant injection Gradle plugin
└── contract/
    └── fixtures/           # Static contract JSON fixtures
```

### Dependency DAG
```
     texure-server ──────────┐
                             │
     texure-blackbox ────────┼───► texure-contract (pure data types & serde)
                             │
     texure-lint ────────────┘
```

- **`texure-contract`**: Contains `UiTree`, `Node`, `MutationRequest`, `ErrorCode`, `ErrorResponse`, and config structs. Zero heavy dependencies (only `serde` and `serde_json`).
- **`texure-server`**: Depends on `texure-contract`, `tokio`, `reqwest`, `tokio-tungstenite`, `toml`, `dirs`, `base64`.
- **`texure-blackbox`**: Replaces internal ad-hoc types with `texure-contract`.
- **`texure-lint`**: Verifies doc contracts against `texure-contract` types.
