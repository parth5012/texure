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

**Verify-at-execute (LLD E round 8):** mutations do not blind-act at remembered coords. Before dispatch the worker re-traverses the tree and checks the identity chain (`test tag > view id > text > class+index`) at the referenced index still matches the agent's rev-referenced node; mismatch ⇒ `NODE_OUTDATED` + re-fetch hint (no rev bump — nothing happened). A refresh between re-resolve and dispatch is accepted as bounded variance; the job runs inside one worker transaction. No auto-correct, no late resolve onto a "similar" node.

### POST /type

```json
{ "node": 5, "text": "hello world", "rev": 42 }
```

Types into an editable node. Response `{ "ok": true }`.

### POST /keypress

```json
{ "key": "ENTER" }  // enum: ENTER, BACK, DEL, TAB
```

### POST /back

No body. `{ "ok": true }`.

### GET /screenshot

Returns `image/png`. Query `scale`: `0.25|0.5|1.0`, **default `0.5`** (LLD E round 8; matches behavior-spec 1.5).

### GET /ws

WebSocket. Bridge pushes `{ "type": "tree_changed", "rev": N }`. Extra event types later are additive; the route name does not change.

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

### (tool note, agent-facing)

MCP server exposes **6 tools**: `device_tree`, `tap`, `type`, `keypress`, `screenshot`, `wait_for`.
`wait_for` semantics (grammar frozen LLD E round 8): agent sends `{ "where": "text"|"tag"|"visible"|"hidden", "value": "<string>", "timeout": <int seconds, default 10, cap 30> }`. Semantics: `text` = any VISIBLE interesting node whose text contains `value` (case-insensitive); `tag` = node whose id/test-tag equals `value` is visible; `visible` = explicit positive text match; `hidden` = NO visible node's text contains `value` (the negative predicate: spinners/loaders gone). Single condition per call; no `and`/`or` composition (agents chain calls); no subtree scope; no `enabled` predicate in v1. Matches on WS `tree_changed` → fresh `/ui-tree` fetch → evaluate; WS down ⇒ HTTP polling every 500ms. Timeout cap 30s (Sunday-timeout safety). On timeout return `{ "ok": false, "last_rev": N }` + one fresh `/ui-tree` snapshot so the agent can re-plan (a non-error response, not the error envelope). HTTP routes stay authoritative for snapshots.

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
| `BRIDGE_STOPPED` | bridge | conn-reset | user stopped bridge mid-action | restart service via overlay, reconnect |
| `PORT_IN_USE` | bridge (device-side, no HTTP) | — | bind conflict on fixed port | free the occupier; service stays stopped |

**Non-error row (separate so lint's code sweep doesn't false-positive):** `WAIT_TIMEOUT` — `wait_for` expiry returns `{ "ok": false, "last_rev": N }` plus the re-plan hint, NOT the error envelope. Removed: `APP_BUSY` (was the draft alias of the single-flight refusal; `BUSY` is the one name).

## Open questions for LLD

All resolved (LLD A round 7 + LLD E round 8, 2026-10-30):

- Port — RESOLVED: fixed `8765`, `SO_REUSEADDR`, loud `PORT_IN_USE` (docs/lld-bridge.md §6).
- Index vs node-id beyond rev guard — RESOLVED: verify-at-execute identity-chain check on every mutation (`POST /tap` section); semantic-hash ids stay ROADMAP.
- `/ui-tree` content-descriptions — RESOLVED: additive `cd` field (nullable), schema_version stays 1. Security annotations — RESOLVED: NOT exposed (bridge reads its own app's tree; no v1 value).
- Screenshot scale default — RESOLVED: `0.5` (matches behavior-spec 1.5).
