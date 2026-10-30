# LLD: bridge internals (surface A)

Authoritative design for `android/bridge`. Resolved 2026-10-30, LLD grilling round 7 (HITL ticket [LLD A: bridge internals](https://github.com/parth5012/texure/issues/6), map 1). Change the contract doc FIRST, then this doc, then code. status: RESOLVED (item 4 of lld-outline).

## 1. HTTP server engine — NanoHTTPD + websocket module

- Embedded server = **NanoHTTPD** with its **`nanohttpd-websocket`** module (RFC 6455 handshake done there). Debug-only dependencies in `android/bridge`; G1 keeps it out of release.
- **Route logic never lives inside the server.** Routes are thin adapters that (a) validate auth/shape, (b) hand a job to the bridge core, (c) render the frozen JSON error shape. Engine stays swappable.
- `/screenshot` streams PNG bytes; `/ws` pushes `tree_changed {rev}` events (contract §WS). No other event classes in v1.

## 2. Thread model — serial bridge worker

One concurrency contract, nameable in behavior-spec rows:

1. **NanoHTTPD owns its network threads** (one per connection). Route handlers never block the accept loop and never run view work themselves.
2. One **serial bridge worker** (single background thread) runs ALL mutation verbs in FIFO order:
   - `sendPointerSync` runs **directly on the worker** (non-main-thread requirement from the injection research verdict).
   - `dispatchTouchEvent` replay and `/ui-tree` capture **hop to the main thread** via `Handler(Looper.getMainLooper()).post` + a result latch; worker waits with a **hard 1s timeout**. Timeout ⇒ INJECT_FAILED/timeout error; never a UI hang.
3. Request threads only enqueue and await the worker's outcome. Combined with the single-flight lock (DECISIONS round 3: `BUSY` + owner, released on disconnect or 60s idle), ordering is total and step boundaries are well-defined.
4. `runOnUiThread` calls from route handlers are banned — main-thread hops go only through the worker's latch mechanism.

Derived mechanics (implementation of frozen law, not new decisions): foreground detection = `ActivityLifecycleCallbacks` counting resumed activities (`APP_NOT_FOREGROUND` refusal stays law); screen/lock refusals = `PowerManager.isInteractive` + `KeyguardManager.isKeyguardLocked`; `HUMAN_STEERING` = real touch events on the app's own window touch pipeline detect a non-synthetic deviceId while a synthetic action is in flight → cancel injection, bump rev, surface the code.

## 3. Injection adapter seam — coords in, verbs frozen, auto fallback inside

```kotlin
interface InjectionAdapter {
  fun tap(x: Dp, y: Dp): InjectionResult
  fun swipe(path: Path, ms: Long): InjectionResult
  fun text(node: ResolvedEditableNode, s: String): InjectionResult
  fun key(k: Key): InjectionResult
}
```

- **The tree layer resolves node index → screen coords** before the adapter is called. The adapter is window-physics only; it never consults the tree. One-directional seam; adapter is testable against a fake window.
- **Fallback frozen inside the adapter, invisible to the agent:** every verb tries `sendPointerSync` first; on `INJECT_FAILED` it replays the same verb via `dispatchTouchEvent`; failure of both ⇒ `INJECT_FAILED` upstream. No agent-visible injection-mode knob exists (spike decides which method actually executes).
- AccessibilityService paths: permanently rejected (G5); never enter this seam.

## 4. Overlay + window ownership — activity-scoped, zero user grants

- Debug overlay is a **view injected into the host Activity's content view** (debug builds only, host-agnostic). No `SYSTEM_ALERT_WINDOW`, no `TYPE_APPLICATION_OVERLAY`, no "display over other apps" grant. MIUI has nothing to fight.
- Overlay responsibilities: show URL + token (pairing), start/stop the service, manual token rotation, later surface newest crash bundle (issue #4 rides the same implementation pass with ticket #1).
- Backgrounded app ⇒ overlay disappears with its window; the **service notification** carries bridge status. `FLAG_KEEP_SCREEN_ON` while a client is active stays law (DECISIONS round 3).
- FGS start comes **only from the overlay** (foreground-context start; Android 10+ background-start rules never engage; the agent cannot start the bridge).

## 5. Foreground service type — `specialUse`

- Manifest declares `foregroundServiceType="specialUse"` with the honest property description: "debug-only developer bridge for agent testing of the host app". Debug-only; no Play review concern.
- `dataSync` rejected: Android 15 caps it at 6h — long sessions would die mid-flow. Notification channel: `texure-bridge`.

## 6. Port policy — fixed 8765, fail loud

- **Single fixed default port `8765`** (freeze row in bridge-contract.md). Bind with `SO_REUSEADDR` so a rapid restart is never punished by `TIME_WAIT`.
- Bind conflict ⇒ service **stops immediately**; overlay + notification surface `PORT_IN_USE` with occupier hint. Agent-side it is an honest connection failure. **No ascending fallback range** — a fallback makes overlay URL ≠ config URL, which breaks the MCP setup silently (loud beats silent).
- v1 binds on Wi-Fi with token auth; off-LAN transport honesty stays contract wording (G11). mDNS hint/QR remain ROADMAP polish; mDNS was never a v1 dependency.
- Ownership note: "port policy" was also listed on ticket #7 (LLD E); it is RESOLVED HERE and removed from #7's grill list. The port table row lands in the contract doc with this decision.

## 7. Pairing token storage — plain app-private prefs

- Token lives in a `SharedPreferences` file owned by the bridge module (`texure_bridge_pairing`), **plaintext, unencrypted**, app-private. Attacker model: another app on the phone (blocked by app-private storage) and LAN peers (blocked by the token itself); not "/data stolen".
- `EncryptedSharedPreferences` rejected: androidx security-crypto is deprecated, MIUI Keystore is quirk-heavy, and the threat model gains nothing.
- `allowBackup=false` on the bridge's manifest contribution so the token never silently rides a debug backup — **rotation is the only way the token changes** (never per-install regeneration on rebuild; DECISIONS round 3 flavor preserved: stable per install).
- Rotation: overlay button ⇒ regenerate ⇒ overlay refreshes URL+token display; old holders get `401 AUTH_FAILED` immediately (contract §Auth).
- No baked-in default token ever (leaks via logs/screenshots; defeats route-auth honesty).

## 8. Shutdown paths — stop = abort

- **User Stop ⇒ hard teardown**: server stops accepting, sockets close immediately, serial worker's in-flight job is cancelled at its next step boundary. No drain-wait: the button exists because the human takes the device back NOW (G9 mirrors this as behavioral law for touch).
- Aborted agent request ⇒ error with candidate code **`BRIDGE_STOPPED`**, handed to ticket #7's error-table freeze (do not overload `APP_BUSY`, which means "another client"). Single-flight lock releases with the teardown; MCP-server reconnect behavior belongs to ticket #8 (LLD C) — the bridge owes only an honest abrupt close.
- Uninstall / force-stop / process death: **zero code**. Process death IS the teardown; the token file dies with app data.
- The bridge is never running unless the user started it (no autostart-on-boot; parked by design).

## Cross-references

- Contract rows this doc freezes: `bridge-contract.md` §Conventions (port + SO_REUSEADDR + `PORT_IN_USE`), §Auth (rotation semantics).
- Behavior rows already law (DECISIONS round 3 + docs/behavior-spec.md): foreground refusal, screen-state refusals, single-flight lock, human wins, staleness rev guard.
- Feeds: ticket #1 (Spike 1 implements this design hand-wired), ticket #4 (crash capture hooks bridge init same pass), ticket #7 (error table incl. `BRIDGE_STOPPED`), ticket #8 (server reconnect), ticket #9 (plugin LLD wires the service shape).
