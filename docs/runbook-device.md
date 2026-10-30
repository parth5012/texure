# Device session runbook (surface F)

Executable checklist. Run top-to-bottom when a session cannot start; jump to step N only if steps 0..N-1 are known-green. Every step has a verification command and expected output. MIUI/HyperOS on Xiaomi; v1 target app: VelaVoice debug build from local source.

Failure rule: any red run produces an evidence bundle (bottom of this doc) and a GitHub issue. Never continue debugging on a red run without the bundle.

## A. One-time setup (repeat only after MIUI update, app reinstall, or reset)

### A1. Developer options
1. Settings → About phone → tap MIUI/HyperOS version 7×.
2. Settings → Additional settings → Developer options → ON:
   - USB debugging (security settings) — required for injection testing probes.
   - USB debugging — required for installs during setup only.
   - Wireless debugging (Android 11+) — pair once; record the PC-side pairing command output in this file's notes section.
   - Stay awake while charging — ON.
   - Window/transition animation scale — 0.5× or 0 (fewer flaky renders during tree waits; optional).

### A2. Battery hardening (MIUI kills non-exempt background services)
1. Security app → Battery → App battery saver → VelaVoice → **No restrictions**.
2. Settings → App management → VelaVoice → Battery saver → No restrictions; + **Autostart ON**.
3. Recents screen: long-press VelaVoice card → **lock** (pin) it.
4. Settings → Always-on display & lock screen → **Screen timeout ≥ 10 min** (during sessions; charge-lid discipline applies).
5. Security app → Boost speed: verify VelaVoice is NOT in the auto-clean exception-less list (it must be exempt from memory clean + data clean).

### A3. Verify once
```
adb devices                       # device serial listed, "device" state
adb shell dumpsys battery | grep -E "powered|status"   # charging present during sessions
```

## B. Per-session checks (every session, in order = the ladder)

### B0. Device up
```
adb devices                      # exactly one device, state "device"
```
Red: reconnect USB/accept auth dialog. Even though runtime is ADB-free (G4), sessions use it setup-only for install + logcat.

### B1. Same network
```
# PC side: find own IPv4 + phone is on same /24
ipconfig | findstr IPv4
# expect 192.168.x.y and phone on 192.168.x.z; record phone IP from Settings → Wi-Fi → network details
ping <phone-ip>                   # replies (AP isolation OFF; no reply = try hotspot fallback)
```
**Hotspot fallback** (when home WiFi blocks ping): phone hotspot ON → PC connects to it → re-run ping. Sessions still work; PC loses external internet, acceptable.

### B2. App on phone (v1: build from local source)
```
cd D:\work\projects\VelaVoice
gradlew :app:assembleDebug        # green build
adb install -r app\build\outputs\apk\debug\app-debug.apk
```

### B3. Bridge visibility
Launch VelaVoice on phone. Debug overlay visible (URL `http://<phone-ip>:<port>` + token). Expected: overlay renders over VelaVoice's own window; URL IP matches B1's subnet.

### B4. PC probe
```
curl http://<phone-ip>:<port>/health    (named per contract; route list is the authority in docs/bridge-contract.md)
# expect ok:true or 200 JSON
```
Red: check state machine in docs/behavior-spec.md (bridge must be in connected-pairable state).

### B5. MCP server connect
Start `texure` MCP server with phone URL + token from B3 config. `device_tree` call returns a tree with `rev` N. Re-call returns same or N+1 rev (staleness guard proving liveliness).

### B6. Injection probe (issue #1 acceptance)
`tap` on a tappable root-owned node → verify touch effect on-screen. One success = the spike verdict.

### B7. VelaVoice flow (issue #2 acceptance)
Unattended: login or simple text flow over the 6 tools. Any red → evidence bundle at the failing step id.

## C. Evidence bundles (contract)

On any red run, one folder `tmp/evidence-<date>-<step>`:
1. `logcat.txt` — `adb logcat -d` tail ≥ 200 lines (fix window: keep tail bounded).
2. `screen.png` — phone screenshot (manual or via `adb exec-out screencap -p`).
3. `tree.json` — last `device_tree`/`ui-tree` output if bridge reachable.
4. `step.md` — runbook step id, probe command, expected vs actual.
Issue: `gh issue create` linking the folder path (push folder to repo `tmp/` branch if contents matter beyond the issue body).

## D. Guards re-confirmed

- **ADB is setup-only.** Runtime agent sessions (B5 onward) never require ADB; if a flow ever needs ADB to work, that's a bug against G4 and the product claim dies.
- **Nothing in this runbook belongs in product code.** ADB/wireless pairing live here + in the ADB bootstrap utility (ROADMAP.Next, optional), never in the bridge's runtime path.
- Runtime requirement after spike: one-time A2 only (token auth, same-network). ADB even for setup should target removal once the pairing overlay covers install-less flows.

## Notes
- (append dated notes here: pairing output, static IP for phone if home router has reservations, hotspot quirks discovered during sessions)
