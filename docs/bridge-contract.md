# Bridge HTTP Contract (v1 draft)

Single source of truth for the transport seam. The Bridge SDK implements this; the Rust MCP server consumes it. Change this doc first, then code.

## Conventions

- Base URL: `http://<phone-ip>:<port>` (port fixed later; token required on all routes).
- Auth: `Authorization: Bearer <token>` header. 401 with JSON error body on mismatch.
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

Returns `image/png`. Optional `?scale=<0.25|0.5|1.0>` for token-cheap vision.

### GET /ws

WebSocket. Bridge pushes `{ "type": "tree_changed", "rev": N }`. Extra event types later are additive; the route name does not change.

### (tool note, agent-facing)

MCP server exposes **6 tools**: `device_tree`, `tap`, `type`, `keypress`, `screenshot`, `wait_for`.
`wait_for` semantics: listen on WS for a matching condition (text/tag visible/hidden) within timeout; on timeout return `{ "ok": false, "last_rev": N }` and the server asks the agent to look again with a fresh `device_tree`. Timeout is agent-controlled. Sunday-timeout safety: for v1, `wait_for` max sane timeout is 30s.

## Error shape (all routes)

```json
{ "error": { "code": "NODE_OUTDATED", "message": "tree refreshed since reference; re-run /ui-tree" } }
```

Codes (draft): `NODE_OUTDATED`, `NODE_NOT_EDITABLE`, `NODE_NOT_CLICKABLE`, `APP_BUSY`, `AUTH_FAILED`, `INJECT_FAILED`.

## Open questions for LLD

- Port and whether it's fixed or configured per install. (researcher relaunch in progress; session `ses_e74f202baffeYlsvZRCxSG0n6x`)
- Index vs node-id targeting beyond rev check (stability across refreshes is rev-guarded now, but node-id vs index still open).
- Whether `/ui-tree` should include content-descriptions, and security annotations.
- Screenshot scale default.
