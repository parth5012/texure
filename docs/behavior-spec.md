# Behavior spec

Defines observable behaviors of both components. This is the pre-implementation source of truth; every route/tool behavior below is decided, not proposed. Companion docs: contract (wire shapes), testing-architecture (how behaviors get verified).

Legend: [S] decided in LLD grilling round 3, 2026-10-30.

## 1. Android bridge (texure bridge SDK)

### 1.1 Lifecycle

| Behavior | Test ID | Spec [S unless noted] |
|---|---|---|
| Build variant | BB-A0, BB-B1 | Bridge code exists ONLY in debug builds. Gradle plugin fails release builds that leak it (future work in #2 line). |
| First boot | BB-B3, BB-B11 | Bridge generates auth token, stores it in app-private storage (app data of the debug applicationId). Token: stable across reboots and rebuilds. Rotation only via overlay "rotate" button [S]. |
| Boot with existing token | BB-B3 | Reuses stored token. Dev pairs ONCE per install. |
| Foreground service | BB-B2, BB-B5 | Starts with active client; keeps process alive while agent session may return. MIUI checklist (battery No restrictions + autostart) is shown in the overlay, and non-blocking: bridge warns, does not refuse if unset. |
| Overlay | BB-B11 | Debug-only, small, shows URL + token + active state (@IDLE/@ACTIVE/@HUMAN). Includes a rotate-token button. QR deferred (ROADMAP). |
| Shutdown | BB-B9 | Service stops when: overlay stop, or 10 min with zero clients. WS + HTTP ports close, screen overlay turns off. |

### 1.2 Screen state

| Behavior | Test ID | Spec |
|---|---|---|
| Keep screen on | BB-B6 | Debug-only `FLAG_KEEP_SCREEN_ON` overlay attaches while a client session is active (partial overlay, no wake lock) [S]. Detaches after 60s of zero clients, or when service stops. |
| Screen off while idle | BB-B6 | Overlay detached; agent's next action fails with `SCREEN_OFF` + hint to reopen app (v1 does not wake the device; v-next may add guarded wake). |
| Device locked while active | BB-B6 | Refuse all mutating actions with `DEVICE_LOCKED` + hint (unlock first). Read routes (`/ui-tree`) also refuse; secure screens are never read. |

### 1.3 Tree (`/ui-tree`)

| Behavior | Test ID | Spec |
|---|---|---|
| Extraction source [S] | BB-A1 | Own view hierarchy traversal from the root view (decorView). No accessibility service, ever. Compose = future framework adapter. VelaVoice (React Native) is covered since RN renders real native views. |
| Node filter [S] | BB-A1, BB-A6 | "Interesting" only nodes: `clickable`, `editable`, `scrollable`, non-empty `text`/`contentDescription`, containers capable of holding interesting descendants. Depth cap default 12 (bump-to param). Boring intermediates are dropped. |
| Node identity | BB-A1 | `test tag > view resource id > text > class+index` fallback chain. Weak RN ids expected; spike 1 verifies live whether they are usable (else RN adapter graduates early). |
| Rev | BB-A1, BB-A5 | Monotonic per tree mutation, backgrounding, OR human touch. Bumped rev = broadcast on `/ws` (`tree_changed {rev}`). |

### 1.4 Mutating actions (`/tap`, `/type`, `/keypress`, `/back`)

| Behavior | Test ID | Spec |
|---|---|---|
| rev guard [S] | BB-A2, BB-B4 | Mutating call includes the `rev` it acts on. If it ≠ bridge `rev`: `NODE_OUTDATED` + hint to re-fetch. No auto-correct, no late resolve. |
| App foregrounded [S] | BB-B5 | Rule: only foreground-app windows accept mutations. If in background: `APP_NOT_FOREGROUND` + hint. Bridge does not auto-launch its Activity (Android 10+ background-start rules + MIUI harden it). Agent must call launch/platform first. |
| Injection mechanism | BB-A2, BB-C1 | `Instrumentation.sendPointerSync` (own windows, non-main thread); `dispatchTouchEvent` replay is the documented fallback. See [spike doc](spike-injection.md). Bridge exposes which mechanism is compiled in at runtime via `device_tree` `verbose:true` metadata (parked for v1; discovery needed for test assertions). |
| Keyboard | BB-A2, BB-C1 | `type` dispatches commit-able text into editable; `keypress` handles the enum `ENTER, BACK, DEL, TAB`. BACK is in-process (via `OnBackPressedDispatcher`/`Activity.dispatchKeyEvent`), not system-level. |
| Screen on/lock [S] | BB-B6 | `SCREEN_OFF`/`DEVICE_LOCKED` refusals as in 1.2. |
| Human wins [S] | BB-B7, BB-C3 | Synthetic injection events carry a marker deviceId injected at craft time. Any real digitizer touch observed during an active agent session: cancels in-flight injection, bumps rev, flips state to `HUMAN_STEERING`. The state change is observable so the agent learns human steering happened; agent never auto-fights the human. |

### 1.5 Screenshot (`/screenshot`)

| Behavior | Test ID | Spec |
|---|---|---|
| Source | BB-A4 | Own window capture. No media projection UX. |
| Scale | BB-A4 | Query `scale`: `0.25|0.5|1.0`, default `0.5`. |
| Failure | BB-A4, BB-B6 | `SCREEN_OFF`/`DEVICE_LOCKED`/`APP_NOT_FOREGROUND` refusals like mutations. |

### 1.6 Transport

| Behavior | Test ID | Spec |
|---|---|---|
| Auth | BB-B3 | `Authorization: Bearer <token>` on every route. Mismatch: `AUTH_FAILED`. |
| WS | BB-A5 | `/ws` push: `tree_changed {rev}`. WS flap tolerated; HTTP routes are the snapshot authority. The server reconnects (bridge is passive listener plus pusher). |
| LAN only | BB-B2 | If phone and PC are on different LANs, connections simply fail; server-side hint tells the dev. Tailscale etc. = ROADMAP. |

## 2. MCP server (texure-server, Rust)

### 2.1 Config & startup [S]

| Behavior | Test ID | Spec |
|---|---|---|
| Config file | BB-B1 | TOML at standard config dir (`~/.config/texure/config.toml` on Linux/macOS, analogous on Windows). `TEXURE_URL`, `TEXURE_TOKEN` env vars override file values. |
| Startup | BB-B1, BB-B2 | Server process starts for the MCP host regardless of device health. It does NOT fail fast on missing device. First connection attempt is lazy on first tool call. |
| Missing config | BB-B1 | Any tool call responds with a `NO_CONFIG` error with exact fix hint (create file / set env). |
| Device unreachable | BB-B2 | Tool call responds with `OFFLINE` + hint (same LAN? app running? fg service alive?). Reconnect with 1s-5s-15s backoff on WS. |

### 2.2 Tools (6, frozen)

| Tool | Test ID | Behavior |
|---|---|---|
| `device_tree` | BB-A0, BB-A1, BB-A6 | Fetches `/ui-tree`, returns JSON. Params: `max_depth` override. |
| `tap` / `type` / `keypress` | BB-A0, BB-A2, BB-A3 | Mutations with rev guard. Return `{ok:true}` or the error text. |
| `screenshot` | BB-A0, BB-A4 | Returns PNG image content. `scale` param. |
| `wait_for` | BB-A0, BB-C2 | Condition grammar: `{where: text|tag|visible|hidden, value: string}, timeout`. Listens on `/ws` for matching `tree_changed`; if WS down, falls back to HTTP polling every 500ms. Timeout cap: 30s. On timeout: `{ok:false, last_rev}` + hint "re-fetch device_tree and re-plan". |
| all tools | BB-A0, BB-A3, BB-B10 | Optional `verbose:true` param returns the full JSON error payload instead of the one-liner [S]. |

### 2.3 Error surface [S]

Every tool failure is a TEXT tool result:

```
NODE_OUTDATED: tree refreshed since reference; re-run device_tree
```

- One line: `CODE: human message`
- With `verbose:true`: full JSON error body (bridge code, HTTP status, ws state, device state).

Server daemon NEVER dies from tool errors; it dies only if the harness kills it.

### 2.4 Concurrency [S]

| Behavior | Test ID | Spec |
|---|---|---|
| Single-flight lock | BB-B8 | First MCP client session that acquires becomes owner. Second concurrent session gets `BUSY` + owner identity (client id/connection handle). |
| Release | BB-B9 | Released on client disconnect, or 60s idle with no tool calls and no active WS session. |
| Rationale | BB-B8 | Debug tool, one dev, one bridge. Two drivers drops finger precision; do not build races. |

## 3. Shared state machine (both sides agree)

```
   UNPAIRED  -> (overlay shows token; server config set) -> OFFLINE
   OFFLINE   -> (server dial succeeds)                   -> ONLINE(IDLE)
   ONLINE(IDLE) -> (mutation or screenshot or wait_for)   -> ONLINE(ACTIVE)
   ONLINE(ACTIVE) -> (human touch detected)               -> HUMAN_STEERING (rev++)
   HUMAN_STEERING -> (agent re-fetch)                     -> ONLINE(IDLE)
   ANY        -> (screen off / locked)                    -> REFUSING (`SCREEN_OFF`/`DEVICE_LOCKED`)
   ANY        -> (app backgrounded)                       -> REFUSING_MUTATIONS (`APP_NOT_FOREGROUND`; reads still succeed but the refusal hint warns they may be stale)
   ANY        -> (60s no clients on bridge)               -> OFFLINE (service stops)
   ONLINE     -> (loser client arrives)                   -> REFUSING (`BUSY`, owner shown)
```

Rules:
- Bridge and server compute no state on their own; each derives state from connection + device facts. State is surfaced, not hidden.
- Every refusal carries a correction hint, keeping agents (the primary consumer) self-sufficient.

## 4. Explicit non-behaviors (v1)

- No auto-wake of a powered-off screen.
- No global/system UI interaction outside the app's own windows (keys, nav bar, notifications).
- No accessibility service, no LATIN access shortcuts, no MIUI special toggles beyond the battery checklist.
- No multi-device fanout, no release-build bridge, no token auto-rotation, no CI cloud relay.

## 5. Verification hooks (link to testing-architecture)

- Every behavior row above maps to a fixture or scenario to be authored in testing-architecture doc: FakeBridge scenarios `static_screen`, `stale_screen`, `slow_screen`, `auth_reject`, `human_win` (NEW: synthetic device id marker), `lock_screen`, `backgrounded`.
- The spec as a whole is the input to the behavior-driven test matrix; breaking a row is a bug, not a choice.
