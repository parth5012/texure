# LLD outline / dig-in list

Things to nail down when we start LLD. Resolve in order.

1. **Injection mechanism (day-1 spike, blocks everything)** — **RESOLVED BY RESEARCH** (see `docs/spike-injection.md`): the Xiaomi INJECT_EVENTS wall only blocks cross-window injection; own-window injection needs no permission. Spike bet: `Instrumentation.sendPointerSync` (own windows, non-main thread), fallback `dispatchTouchEvent` replay. AccessibilityService rejected (MIUI kills accessibility services). Spike experiment defined; not yet executed.
   - Acceptance: the 5 criteria in docs/spike-injection.md.
2. **Node model and ids** — **RESOLVED (2026-10-30, LLD round 8 / ticket #7)**: v1 identity = index-in-last-tree + rev guard + verify-at-execute identity-chain check (`test tag > view id > text > class+index`) at dispatch. Semantic hash ids stay ROADMAP (re-decided with the same rationale as round 2; mid-press refresh recovery = `NODE_OUTDATED` re-fetch, not new machinery). Frozen in bridge-contract.md (`POST /tap`).

3. **Targeting scheme** — **RESOLVED (round 8)**: array-index-from-last-tree confirmed as v1; the "verify it survives a real refresh flow" concern is covered structurally by verify-at-execute (contract) + blackbox B4/`stale_screen` scenarios; live spike still validates on-device.

4. **Bridge lifecycle inside the app** — **RESOLVED (2026-10-30, LLD round 7)**, see `docs/lld-bridge.md`:
    - Engine: NanoHTTPD + websocket module (route logic in bridge core, engine swappable).
    - Threads: serial bridge worker; sendPointerSync direct, dispatchTouchEvent/tree capture hop to main with latch + 1s timeout.
    - Overlay: activity-scoped debug view, zero user grants; FGS `specialUse`, foreground-context start only.
    - Port: fixed `8765`, `SO_REUSEADDR`, loud `PORT_IN_USE`. Token: app-private plaintext prefs, `allowBackup=false`, overlay-only rotation.
    - Shutdown: user Stop = hard abort (`BRIDGE_STOPPED` handed to surface E); uninstall/force-stop = zero code.

5. **Gradle plugin mechanics**
   - How to add a dependency + init hook to debug source sets only, from a plugin. Manifest injection vs Application subclass hook vs startup initializer.
   - Pairing token flow: part-resolved (in-app debug overlay shows URL + token; QR deferred).

6. **MCP server shape**
   - Tool list freeze: **6 tools** (`device_tree`, `tap`, `type`, `keypress`, `screenshot`, `wait_for`). Error taxonomy mapping bridge codes to MCP tool errors, screenshot plumbing (PNG to tool result image).
   - Rust crates: mcp microservice SDK choice, HTTP client, config file location.

7. **Crate/module split freeze**
   - Finalize tree: crates/texure-server, android/bridge, android/gradle-plugin, contract/.

8. **CI/CD + distribution (surface D)** — **RESOLVED (2026-10-30, grilling round 5)**, see `docs/lld-ci.md`:
    - Merge gates: lint → rust-test are PR gates; android-build becomes a gate once plugin code lands; nightly device runs never gate.
    - OS targets: ubuntu + windows per PR (windows = ship platform); mac at release tag only.
    - Rust dist: prebuilt gh-release binaries (windows-msvc/linux-gnu/apple-darwin) + checksums; `cargo install --locked` secondary.
    - Android dist: mavenLocal + in-repo `examples/` quickstart (separate repo parked, see ROADMAP.Later).
    - Release recipe: tag v* → cross-build → checksum → gh release → cargo publish; semver, breaking MCP contract change = major.
    - Nightly device runs: T3 Code scheduled tasks on this PC; FAILs open issues via gh. GitHub Actions has no device role.
    - Remaining open: NIGHTLY-INVOCATION exact task shape (blocked by #1); crates.io name check at first publish.

9. **Device session runbook (surface F)** — **RESOLVED (2026-10-30, grilling round 6)**, see `docs/runbook-device.md`:
    - Executable checklist doc (GUI-verified steps can't be scripted); one-time MIUI hardening vs per-session ladder B0..B7; home WiFi + hotspot fallback; ADB setup-only inside sessions; source-build path for VelaVoice v1; evidence bundle contract per red run.

Each item: when resolved, write the answer into `docs/bridge-contract.md` or ARCHITECTURE.md, and log it in DECISIONS.md.
