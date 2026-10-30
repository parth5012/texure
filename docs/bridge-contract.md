# Bridge HTTP Contract (v1 draft)

Single source of truth for the transport seam. The Bridge SDK implements this; the Rust MCP server consumes it. Change this doc first, then code.

## Conventions

- Base URL: `http://<phone-ip>:<port>` (port fixed later; token required on all routes).
- Auth: `Authorization: Bearer <token>` header. 401 with JSON error body on mismatch.
- All bodies JSON except `/screenshot` (PNG).
- Node ids are stable within one screen interaction; a tree refresh may renumber.

## Routes

### GET /ui-tree

Returns the accessibility/view tree of the active window, compact form:

```json
{
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
{ "node": 3, "index": 2 }   // array element from the LAST /ui-tree response
```

Response: `{ "ok": true }`. Index (not raw coords) is the v1 targeting scheme so trees stay the single source of truth; raw coordinate mode parked.

### POST /type

```json
{ "node": 5, "text": "hello world" }
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

## Error shape (all routes)

```json
{ "error": { "code": "NODE_OUTDATED", "message": "tree refreshed since reference; re-run /ui-tree" } }
```

Codes (draft): `NODE_OUTDATED`, `NODE_NOT_EDITABLE`, `NODE_NOT_CLICKABLE`, `APP_BUSY`, `AUTH_FAILED`, `INJECT_FAILED`.

## Open questions for LLD

- Port and whether it's fixed or configured per install.
- Index vs node-id targeting tradeoff (stability across refreshes).
- Whether `/ui-tree` should include content-descriptions, and security annotations.
- Screenshot scale default.
