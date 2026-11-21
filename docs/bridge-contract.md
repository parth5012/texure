# Bridge HTTP Contract (v1 draft)

Single source of truth for the transport seam. The Bridge SDK implements this; the Rust MCP server consumes it. Change this doc first, then code.

## Conventions

- Base URL: `http://<phone-ip>:<port>`; **port is FIXED `8765`** (LLD A, docs/lld-bridge.md). Overlay always shows the live URL; conflict on bind = loud `PORT_IN_USE`, no fallback range.
- Server transport: LAN HTTP/WS, NanoHTTPD-based implementation detail (engine swappable; contracts below are engine-agnostic). Token required on all routes.
- Auth: `Authorization: Bearer <token>` header. 401 with JSON error body on mismatch. Token rotates ONLY manually via the debug overlay; old holders fail 401 `AUTH_FAILED` immediately after rotation (docs/lld-bridge.md §7).
- Bind behavior: `SO_REUSEADDR`; bind conflict stops the service and surfaces `PORT_IN_USE` on the device (overlay + notification); agents see a connection failure (docs/lld-bridge.md §6).
- All bodies JSON except `/screenshot` (PNG).
- **Staleness contract:** every `/ui-tree` response carries `rev`. Mutation routes accept the `rev` the agent is acting on; mismatch or a newer rev on the bridge is a `NODE_OUTDATED` error. The agent re-fetches.
- **WS channel:** `ws://<phone-ip>:<port>/ws` pushes `{ "type": "tree_changed", "rev": N }` events. It feeds `wait_for` reactive waits; HTTP routes stay authoritative for snapshots.
- Node ids are stable within one screen interaction; a tree refresh may renumber (rev bump signals it).

### mDNS LAN discovery (map #36, ticket #32)

Additive discovery only; no route, tool, error code, or `schema_version` change.

- **Advertisement (bridge):** on successful server start the bridge registers an mDNS/DNS-SD service via platform `NsdManager`: service type `_texure._tcp.local.`, port `8765` (the fixed bridge port), service name `Texure-<appId>`, TXT records `version=0.1.0` and `appId=<debugApplicationId>`. Registration is unregistered cleanly on `stopBridge()`/`onDestroy()`. If `NsdManager` is unavailable or registration throws, the bridge logs and continues without discovery (static-IP pairing unaffected).
- **Resolution (Rust MCP server):** when the resolved config `url` is exactly `"auto"`, the server sends a DNS-SD PTR query for `_texure._tcp.local.` to `224.0.0.251:5353`, listens (~2 s), and parses SRV (host/port) + A + TXT (`version`/`appId`) using only existing dependencies (`tokio` UDP; no new Cargo dependency, per the map zero-external-dependency decision — the ticket's `mdns-sd` wording is intentionally not adopted). On a match the resolved URL becomes `http://<host>:8765` and proceeds normally. On no answer within the timeout (blocked multicast / AP isolation), the server falls back to existing error semantics: tool calls return `OFFLINE` — no new error code. An omitted `url` keeps its current semantics (`NO_CONFIG`); only the explicit `"auto"` value triggers discovery (flipping omitted→auto would break the `NO_CONFIG` blackbox expectations).
- **Multi-device profiles (map #36, #34):** `config.toml` supports `[profiles.<name>]` tables with `url`/`token`/`timeout_secs` plus top-level `active_profile`. Active selection precedence: `--profile <name>` CLI flag → `TEXURE_PROFILE` env var → `active_profile` in file → top-level keys. A named profile that does not exist is a hard error (`NO_CONFIG` with a "profile '<name>' not found" message). No new error code; `schema_version` stays 1.
- Blackbox mapping is documented as intended (BB-D intent) and owned by ticket #35; no scenario is added by this ticket.

## Routes

### GET /ui-tree

Returns the accessibility/view tree of the active window, compact form:

```json
{
  "rev": 42,
  "window": "com.velavoice/.MainActivity",
  "nodes": [
    {
      "id": "btn_send",            // view id or test tag; may be null
      "hash": "9f3ac21b04e5d678",  // semantic hash (map #36 #29, additive); see POST /tap
      "text": "Send",              // may be null
      "cd": "Send message",        // contentDescription; may be null (LLD E round 8, additive)
      "cls": "android.widget.Button",
      "bounds": [12, 340, 108, 56], // x, y, w, h (dp)
      "clickable": true,
      "editable": false
    }
  ]
}
```

Optional query params: `?subtree=<node id>`, `?max_depth=<n>`.

### POST /tap

```json
{ "node": 3, "index": 2, "rev": 42 }   // node from the LAST /ui-tree response, at claim-time rev 42
```

Response: `{ "ok": true }`. Index (not raw coords) is the v1 targeting scheme so trees stay the single source of truth; raw coordinate mode parked.

**Semantic-hash targeting (map #36, #29):** `POST /tap` also accepts an optional `hash` — the `hash` value of a node from the LAST `/ui-tree` response. When `hash` is present, `node`/`index` becomes optional (either index or hash targets a node; at least one is required). Verify-at-execute order when `hash` is present: (1) if an index was given and the node at that index still carries the same `hash`, dispatch there; (2) else resolve `hash` across the fresh tree (first node whose `hash` matches); (3) if no node matches, refuse with `NODE_OUTDATED` and the re-fetch hint. The `rev` guard is unchanged: a mismatched `rev` still refuses with `NODE_OUTDATED` before verify-at-execute runs. `hash` is FNV-1a 64-bit over (ancestor path, stable tag/resource id, text snippet, class name), hex-encoded; it is stable across rev bumps and non-structural updates (text tweaks on other nodes, identical re-extractions) and changes with the node's own identity (id/text/class/ancestry). `hash` is an additive nullable field; `schema_version` stays 1.

**Verify-at-execute (LLD E round 8):** mutations do not blind-act at remembered coords. Before dispatch the worker re-traverses the tree and checks the identity chain (`test tag > view id > text > class+index`) at the referenced index still matches the agent's rev-referenced node; mismatch ⇒ `NODE_OUTDATED` + re-fetch hint (no rev bump — nothing happened). A refresh between re-resolve and dispatch is accepted as bounded variance; the job runs inside one worker transaction. No auto-correct, no late resolve onto a "similar" node.

### POST /type

```json
{ "node": 5, "text": "hello world", "rev": 42 }
```

Types into an editable node. Response `{ "ok": true }`.

Optional `hash` (map #36, #29): same verify-at-execute targeting and semantics as `POST /tap` — index optional when `hash` is present, index preferred on hash match, else resolve by `hash` across the fresh tree, no match ⇒ `NODE_OUTDATED`. `rev` guard unchanged.

### POST /keypress

```json
{ "key": "ENTER" }  // enum: ENTER, BACK, DEL, TAB
```

### POST /back

No body. `{ "ok": true }`.

### POST /tag (dynamic view tagging, map #36, #31)

```json
{ "target": { "index": 2, "rev": 42 }, "tag": "my_custom_tag" }
```

Attaches `tag` to the view at `target.index` via `View.setTag(tag)`. Response `{ "ok": true }`. The tag surfaces as that node's `/ui-tree` `id` on the next extraction (existing `resolveNodeId` tag path) and survives in-session tree refreshes (the view object persists). `rev` guard matches `/tap`/`/type`: stale `rev` or out-of-range `index` ⇒ `NODE_OUTDATED` + re-fetch hint (no rev bump on refusal — nothing happened). Blank/oversized tag (max 256 chars) or a missing field ⇒ HTTP 400 `BAD_REQUEST`. A successful mutation bumps `rev` and broadcasts `tree_changed`. Since the tag becomes the node `id` (a `#29` hash input), the tagged node's `hash` rotates on tagging; descendants' hashes rotate too (their ancestor path carries the tag). Agents must re-fetch `/ui-tree` and re-target after tagging. Additive HTTP route only; the `tag` MCP tool is NOT added (6-tool freeze, G7); `schema_version` stays 1.

### GET /screenshot

Returns `image/png`. Query `scale`: `0.25|0.5|1.0`, **default `0.5`** (LLD E round 8; matches behavior-spec 1.5).

Optional query `region=x,y,w,h` (map #36, ticket #30): crops the captured bitmap to that rectangle **before** compression. Coordinates are full-resolution pixels. The rectangle is clamped to the decorView bounds; a malformed value or `w`/`h` ≤ 0 returns HTTP 400 (`BAD_REQUEST`, informative message). `scale` is applied after the crop. Additive query params only; `schema_version` stays 1.

### GET /ws

WebSocket. Bridge pushes `{ "type": "tree_changed", "rev": N }` or real-time error log notifications `{ "event": "log_error", "type": "log_error", "entry": { ... } }` when an in-process ERROR-level log is captured. Extra event types later are additive; the route name does not change.

### GET /last-crash

Returns the newest debug crash/ANR bundle exported to app-private storage (ticket #4):

```json
{
  "timestamp": 1730370000000,
  "stack_trace": "java.lang.RuntimeException: Sample crash\n\tat com.texure.sample.MainActivity...",
  "last_tree": { "rev": 42, "window": "com.texure.sample/.MainActivity", "nodes": [] },
  "last_actions": [
    { "action": "tap", "node": 3, "rev": 42, "result": "ok" }
  ],
  "build_info": {
    "application_id": "com.texure.sample",
    "version_name": "1.0",
    "bridge_rev": "0.1.0"
  }
}
```

If no crash bundle exists, returns HTTP 404 with standard error envelope (`{ "error": { "code": "NOT_FOUND", "message": "no crash bundle available" } }`). Bearer auth required.

### Workflow recording (LLD record/replay, map #6 ticket #23)

Recording is a bridge-side capability: the bridge observes interactions on its own windows and buffers a timestamped trace. Control routes only — the 6 MCP tools stay frozen (recording is driven over bridge HTTP, guardrail G7).

#### POST /recording/start

Starts a recording session: attaches the passive touch observer (own windows only, no gesture consumption) and begins buffering actions. Response `{ "ok": true, "recording": true }`. Idempotent: starting while already recording returns `{ "ok": true, "recording": true }` unchanged (buffer is NOT cleared).

#### POST /recording/stop

Ends the current recording session and detaches the observer. Response `{ "ok": true, "recording": false, "count": N }`. Stopping while no session is active ⇒ HTTP 400 with error code `RECORDING_IDLE`.

#### GET /recording/export

Returns the buffered trace (works while recording or stopped):

```json
{
  "recording": false,
  "count": 2,
  "actions": [
    {
      "seq": 0, "t": 0, "action": "tap",
      "node": { "index": 3, "id": "btn_send", "text": "Send", "cd": null, "cls": "android.widget.Button", "rev": 42 }
    },
    { "seq": 1, "t": 1840, "action": "type", "node": { "index": 2, "id": null, "text": null, "cd": null, "cls": "android.widget.EditText", "rev": 43 }, "value": "hello" },
    { "seq": 2, "t": 3900, "action": "keypress", "key": "BACK" },
    { "seq": 3, "t": 3900, "action": "pause", "ms": 2500 }
  ]
}
```

- `t` = milliseconds since session start (timestamp offsets).
- `node.index` = index into the `/ui-tree` node list observed at record time (same targeting scheme as `/tap`); `rev` = tree rev at record time.
- `type` `value` is the committed text observed on the focused editable node.
- Human-originated input only by nature of the observer; agent-injected synthetic events carry the marker deviceId and are NOT recorded (human wins law, behavior-spec 1.4).

#### DELETE /recording

Clears the buffered trace. Response `{ "ok": true, "count": 0 }`.

### In-app Network Interception & Dynamic API Mocking (map #69)

In debug builds, OkHttpClient is instrumented via AGP ASM to attach TexureNetworkInterceptor. All real network requests pass through 100% untouched by default unless an active mock rule matches. Control routes are authenticated with Bearer token and are exempt from single-flight locking (control plane):

#### POST /network/mocks

Registers a new dynamic mock rule:
```json
{
  "method": "GET",
  "url_pattern": "^https://api\\.example\\.com/users/.*$",
  "response": {
    "status": 200,
    "headers": { "Content-Type": "application/json" },
    "body": "{\"mocked\": true}"
  },
  "times": 1,
  "delay_ms": 50
}
```
Response: `{ "ok": true, "id": "<id>", "rule": { ... } }`.
Invalid regex, invalid status (< 200 or > 599), negative `delay_ms`, delay exceeding 30,000ms (30s), non-integer `times`/`delay_ms`/`status`, or malformed payload returns HTTP 400 with `BAD_REQUEST`. Maximum capacity is 200 active mock rules.

#### DELETE /network/mocks

Clears all registered mock rules. Response `{ "ok": true, "cleared": true }`.

#### GET /network/history

Returns the rolling ring buffer of captured requests (last 100 requests). Accepts optional query parameter `?url_contains=<string>` to filter results. Request bodies are captured up to 64 KB; bodies exceeding 64 KB or one-shot/streaming bodies are reported with placeholder descriptors (`[one-shot / streaming body]` or `[body exceeds 64KB ... - omitted]`) to prevent memory exhaustion.
```json
{
  "history": [
    {
      "url": "https://api.example.com/users/1",
      "method": "GET",
      "status": 200,
      "request_body": null,
      "latency_ms": 12,
      "timestamp": 1700000000000
    }
  ],
  "count": 1
}
```

#### DELETE /network/history

Clears the rolling request history buffer. Response `{ "ok": true, "cleared": true }`.

#### POST /network/mode

Sets runtime mocking enable/disable mode without rebuilding the app:
```json
{ "enabled": false }
```
Response: `{ "ok": true, "enabled": false }`. When disabled, all requests immediately bypass interception and pass through 100% untouched with zero history or memory overhead regardless of active mock rules. Missing or non-boolean `enabled` returns HTTP 400 `BAD_REQUEST`. Mock rules and captured history are reset when the bridge server stops.

#### POST /network/execute

Diagnostic/test execution route that dispatches an HTTP request through the app's OkHttpClient and TexureNetworkInterceptor:
```json
{
  "url": "https://api.example.com/users",
  "method": "GET",
  "body": "{...}"
}
```
Response: `{ "status": 200, "headers": { ... }, "body": "...", "passthrough": false }`. Synthetic mock responses include the `X-Mock-Source: Texure` header.

### State Inspection, Seeding, Reset & Snapshots (map #74)

In debug builds, the bridge provides direct read/write access to SharedPreferences and SQLite/Room databases, Day-0 clean-slate resets, and named state snapshots to bypass repetitive setup flows. Control routes are authenticated with Bearer token:

#### GET /state/prefs

Returns all key-value entries across default or named preferences:
Query parameter: `?name=<file_name>` (optional; if omitted or empty, inspects default shared preferences).
Response:
```json
{
  "file": "app_prefs",
  "entries": {
    "user_id": "u123",
    "is_logged_in": true,
    "login_count": 5,
    "balance": 10000000000,
    "rating": 4.5,
    "tags": ["alpha", "beta"]
  }
}
```

#### POST /state/prefs

Mutates SharedPreferences synchronously via `SharedPreferences.Editor.commit()`:
```json
{
  "file": "app_prefs",
  "set": {
    "user_id": "u456",
    "is_logged_in": true,
    "login_count": 6
  },
  "remove": ["guest_mode"]
}
```
Response: `{ "ok": true }`.
Supported types in `set`: String, Boolean, Int, Long, Float, StringSet (JSON Array of strings).
Bumps tree revision on mutation to trigger UI invalidation and WebSocket `tree_changed` notification.

#### GET /state/db/tables

Discovers `.db` files in `/data/data/<package>/databases/` and returns table names and column schemas:
Query parameter: `?db=<name>`
Response:
```json
{
  "db": "app_db",
  "tables": [
    {
      "name": "users",
      "columns": [
        { "name": "id", "type": "INTEGER" },
        { "name": "email", "type": "TEXT" }
      ]
    }
  ]
}
```

#### POST /state/db/query

Executes read-only SQL queries and returns row objects:
```json
{
  "db": "app_db",
  "sql": "SELECT id, email FROM users WHERE id = ?",
  "params": [1]
}
```
Response:
```json
{
  "rows": [
    { "id": 1, "email": "test@example.com" }
  ],
  "count": 1
}
```

#### POST /state/db/exec

Executes DDL/DML statements:
```json
{
  "db": "app_db",
  "sql": "UPDATE users SET email = ? WHERE id = ?",
  "params": ["new@example.com", 1]
}
```
Response: `{ "ok": true, "affected_rows": 1 }`.
SQL syntax or execution errors return HTTP 400 with `SQL_ERROR`.

#### POST /state/reset

Clean-slate reset simulating a fresh Day-0 install:
```json
{ "scope": "all" }
```
`scope` options: `"all"`, `"prefs"`, `"db"`, `"cache"`.
Response: `{ "ok": true, "reset": "all" }`.

#### POST /state/snapshots/save

Creates a compressed snapshot archive of current `shared_prefs` and `databases` in app-private storage:
```json
{ "name": "post_onboarding" }
```
Response: `{ "ok": true, "name": "post_onboarding" }`.

#### POST /state/snapshots/restore

Restores an archived snapshot:
```json
{ "name": "post_onboarding" }
```
Response: `{ "ok": true, "name": "post_onboarding" }`.

#### GET /state/snapshots

Lists available snapshots:
Response:
```json
{
  "snapshots": ["post_onboarding"]
}
```

### Diagnostic log buffer & query routes (map #17, #83)

In-process capture of own-PID application logcat into a rolling circular ring buffer (last 2,000 entries, ~500 KB RAM limit).

#### GET /logs

Returns buffered log entries matching filter criteria:
- `level=V|D|I|W|E`: filters entries by minimum severity level.
- `tag=<substring>`: case-insensitive tag substring filter.
- `since=<epoch_ms>`: returns entries newer than timestamp.
- `limit=<int>`: default 100, max 1,000.

```json
{
  "logs": [
    {
      "timestamp": 1730370000000,
      "pid": 1234,
      "tid": 5678,
      "level": "ERROR",
      "tag": "MyTag",
      "message": "Something failed"
    }
  ],
  "count": 1
}
```

#### DELETE /logs

Clears the rolling log buffer. Response:
```json
{
  "ok": true,
  "cleared": true,
  "count": 0
}
```

### Deep-link Intent Routing & Notification Simulation (Map 18, #87)

Additive routes for deep-link testing and local notification simulation.

#### POST /intent

Dispatches a package-scoped Android Intent for deep-link navigation testing.

```json
{
  "action": "android.intent.action.VIEW",
  "uri": "app://example/path",
  "extras": { "userId": 123, "ref": "promo", "active": true },
  "flags": ["FLAG_ACTIVITY_NEW_TASK", "FLAG_ACTIVITY_CLEAR_TOP"],
  "package": "com.texure.sample"
}
```

- `action`: optional string, defaults to `android.intent.action.VIEW` if `uri` is provided.
- `uri`: optional URI string.
- `extras`: optional map of primitive extras (String, Int, Long, Double, Boolean).
- `flags`: optional list of flag strings from standard `Intent.FLAG_*`. `FLAG_ACTIVITY_NEW_TASK` is always enforced.
- `package`: optional package name. Enforces package-scoping: must match the target app's own `packageName`, or is rejected with 400 `CROSS_PACKAGE_FORBIDDEN` (Guardrail G6). In all cases, `intent.setPackage(context.packageName)` is enforced.
- **Tree revision:** On successful activity transition, tree revision is bumped and `tree_changed` is broadcast over `/ws`.

Response (200 OK):
```json
{
  "ok": true,
  "action": "android.intent.action.VIEW",
  "uri": "app://example/path",
  "package": "com.texure.sample",
  "rev": 43
}
```

Errors:
- 400 if URI syntax is invalid or activity cannot be resolved (`ACTIVITY_NOT_FOUND`).
- 400 if `package` targets a foreign package (`CROSS_PACKAGE_FORBIDDEN`).

#### POST /notification/simulate

Simulates local push notification delivery with automatic notification channel registration.

```json
{
  "title": "New Message",
  "body": "You have received a new notification",
  "channel_id": "texure_debug_channel",
  "intent_uri": "app://example/chat/42",
  "small_icon": "ic_notification"
}
```

- `title`: required notification title string.
- `body`: required notification content text string.
- `channel_id`: optional notification channel ID; if missing or does not exist, automatically creates debug channel `texure_debug_channel` with high importance.
- `intent_uri`: optional deep-link URI attached as `PendingIntent` to the notification content intent.
- `small_icon`: optional drawable resource name; falls back to app icon.

Response (200 OK):
```json
{
  "ok": true,
  "id": 1,
  "channel_id": "texure_debug_channel"
}
```

#### DELETE /notification

Cancels a simulated notification by ID.

Query param: `?id=<int>`.
Response (200 OK):
```json
{
  "ok": true,
  "id": 1,
  "cancelled": true
}
```

### (tool note, agent-facing)

MCP server exposes **6 tools**: `device_tree`, `tap`, `type`, `keypress`, `screenshot`, `wait_for`.
`wait_for` semantics (grammar frozen LLD E round 8): agent sends `{ "where": "text"|"tag"|"visible"|"hidden", "value": "<string>", "timeout": <int seconds, default 10, cap 30> }`. Semantics: `text` = any VISIBLE interesting node whose text contains `value` (case-insensitive); `tag` = node whose id/test-tag equals `value` is visible; `visible` = explicit positive text match; `hidden` = NO visible node's text contains `value` (the negative predicate: spinners/loaders gone). Single condition per call; no `and`/`or` composition (agents chain calls); no subtree scope; no `enabled` predicate in v1. Subscribes to WS `tree_changed` → immediately fetches `/ui-tree` once to evaluate → re-evaluates upon every incoming `tree_changed` event; WS down ⇒ HTTP polling every 500ms. Timeout cap 30s (Sunday-timeout safety). On timeout return `{ "ok": false, "last_rev": N }` + one fresh `/ui-tree` snapshot so the agent can re-plan (a non-error response, not the error envelope). HTTP routes stay authoritative for snapshots.

## Error shape (all routes)

```json
{ "error": { "code": "NODE_OUTDATED", "message": "tree refreshed since reference; re-run /ui-tree" } }
```

**Single authority: this table** (LLD E round 8, ticket #7). `texure-lint` docs-gate parses these rows directly (uppercase-snake first column); every code must be referenced by ≥1 behavior row or scenario; codes used elsewhere but absent here FAIL lint (orphan check). One spec line = one authority; behavior rows and blackbox scenarios cite codes, never re-list them.

| Code | Surface | HTTP | Trigger | Agent hint |
|---|---|---|---|---|
| `NO_CONFIG` | mcp | — | no config file/env | create `~/.config/texure/config.toml` or set `TEXURE_URL`/`TEXURE_TOKEN` |
| `OFFLINE` | mcp | — | device unreachable | same LAN? app open? fg service alive? |
| `AUTH_FAILED` | bridge | 401 | token mismatch (incl. stale token after rotation) | re-paste fresh token from overlay |
| `BUSY` | bridge | 409 | second client during single-flight | wait for owner release / ask owner |
| `NODE_OUTDATED` | bridge | 400 | rev mismatch or verify-at-execute identity mismatch | re-run `device_tree`, re-target |
| `NODE_NOT_CLICKABLE` | bridge | 400 | tap onto non-clickable node | pick a clickable node from the latest tree |
| `NODE_NOT_EDITABLE` | bridge | 400 | type into non-editable node | tap the editable field first, then type |
| `NODE_NOT_FOUND` | bridge | 400 | `?subtree=<id>` unknown id | ids only come from the latest `device_tree` |
| `INJECT_FAILED` | bridge | 400 | both injection methods failed | re-fetch tree, retry once; bundle evidence |
| `CAPTURE_FAILED` | bridge | 400 | `/screenshot` capture failed | foreground the app; `device_tree` still works |
| `APP_NOT_FOREGROUND` | bridge | 400 | mutation while backgrounded | bring app forward explicitly (no auto-launch) |
| `SCREEN_OFF` | bridge | 400 | display off | wake + reopen app (human step) |
| `DEVICE_LOCKED` | bridge | 400 | keyguard locked | unlock device first |
| `HUMAN_STEERING` | bridge | 409 | human touch mid-action | re-plan signal: `device_tree` then continue |
| `RECORDING_IDLE` | bridge | 400 | `/recording/stop` with no active session | start with `POST /recording/start` first |
| `BRIDGE_STOPPED` | bridge | conn-reset | user stopped bridge mid-action | restart service via overlay, reconnect |
| `PORT_IN_USE` | bridge (device-side, no HTTP) | — | bind conflict on fixed port | free the occupier; service stays stopped |
| `DB_NOT_FOUND` | bridge | 404 | target database does not exist | verify database name via `GET /state/db/tables` |
| `SQL_ERROR` | bridge | 400 | SQL syntax or execution error | verify SQL statement syntax and parameters |
| `SNAPSHOT_NOT_FOUND` | bridge | 404 | snapshot archive does not exist | verify snapshot name via `GET /state/snapshots` |
| `SNAPSHOT_ERROR` | bridge | 400 | snapshot creation failure | verify storage space and permissions |
| `RESTORE_ERROR` | bridge | 400 | snapshot restoration failure | verify snapshot integrity |
| `COMMIT_FAILED` | bridge | 500 | preference commit failure | retry mutation |

**Non-error row (separate so lint's code sweep doesn't false-positive):** `WAIT_TIMEOUT` — `wait_for` expiry returns `{ "ok": false, "last_rev": N }` plus the re-plan hint, NOT the error envelope. Removed: `APP_BUSY` (was the draft alias of the single-flight refusal; `BUSY` is the one name).

## Open questions for LLD

All resolved (LLD A round 7 + LLD E round 8, 2026-10-30):

- Port — RESOLVED: fixed `8765`, `SO_REUSEADDR`, loud `PORT_IN_USE` (docs/lld-bridge.md §6).
- Index vs node-id beyond rev guard — RESOLVED: verify-at-execute identity-chain check on every mutation (`POST /tap` section); semantic-hash ids RESOLVED (map #36, #29): additive nullable `hash` on each `/ui-tree` node + optional `hash` on `/tap`/`/type` with index-preferred, hash-fallback resolution; `schema_version` stays 1.
- `/ui-tree` content-descriptions — RESOLVED: additive `cd` field (nullable), schema_version stays 1. Security annotations — RESOLVED: NOT exposed (bridge reads its own app's tree; no v1 value).
- Screenshot scale default — RESOLVED: `0.5` (matches behavior-spec 1.5).

---

## Remote Companion Gateway & Reverse WebSocket Tunnel (Group R, maps 10–13)

Single authority for remote companion protocol boundaries established in Map 9 (write-first RED suite BB-R1..BB-R13, docs/behavior-spec.md §3R).

1. **Reverse WebSocket Tunnel (`wss://`):** Companion dials the desktop tunnel endpoint with `Authorization: Bearer <token>`. Connection handshake validates bearer token; invalid token returns 401 `AUTH_FAILED`. On drop, the companion supervisor reconnects with exponential backoff per `reverse_ws_envelope.json` (companion/L3 behaviour; the BB-R3 L2 blackbox asserts re-registration only).
2. **Gateway RPC Proxying:** Desktop forwards standard bridge RPCs (`device_tree`, `tap`) encapsulated in tunnel envelopes. Responses, including staleness `NODE_OUTDATED` and verify-at-execute revision errors, are relayed verbatim (`gateway_rpc_envelope.json`).
3. **Pairing QR Payload:** Carries authenticated endpoint metadata (`wss://`, host, port, token) parsed and validated against `contract/fixtures/pairing_qr_payload.json`.
4. **Transport Resolution & Fallback:** Transport resolution checks configured transports in strict priority order (`reverse_ws` primary, falling back to `lan_direct` on absence/failure, per `contract/fixtures/transport_order.json`). The order is configurable via `[network].transport_order` in `config.toml` (default `reverse_ws`, `lan_direct`, `tailscale`); `tailscale` is attempted only when `[network].tailscale_ip` is set. Invalid names are `NO_CONFIG` config errors. All candidates failing is one-line `OFFLINE` (G8); no new tool, error code, or `schema_version` change.
5. **Shizuku Remote Lifecycle Bootstrap:** Remote physical devices running unattended tests utilize Shizuku command execution (`contract/fixtures/shizuku_command_schema.json`) for wake/keyguard dismiss (BB-R7), cold app launch `am start` (BB-R8), runtime permission `pm grant` (BB-R9), and system permission popup tap fallback (BB-R10).
