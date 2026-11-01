# Log

## 2026-11-01 (Wayfinder map 2 Phase 2 Spike 2 on-device verification & resolution)
- Map 2 Phase 2 AFK Spike 2 (issue #2: VelaVoice integration proof + first agent flow) successfully executed and verified on physical Xiaomi test device (`IBFUSOZHCYJJ99JZ`, Android 15 / SDK 35) connected to Windows host `DESKTOP-IJO3ROS` via T3 Code linked environment:
  - Bridge Publication:
    - Added `maven-publish` plugin and configuration to `android/bridge/build.gradle.kts` with `minSdk 24`.
    - Added `TexureInitProvider` (`ContentProvider`) in `android/bridge/src/main/kotlin/com/texure/bridge/init/TexureInitProvider.kt` and `AndroidManifest.xml` registering `Application.ActivityLifecycleCallbacks` for zero-code automatic initialization (satisfying Guardrail G2).
    - Executed `./gradlew :bridge:publishToMavenLocal --no-daemon`, successfully producing `com.texure:bridge:0.1.0` in `~/.m2`.
  - Target App Integration (`velaboard` / VelaVoice):
    - Added single dependency line `debugImplementation("com.texure:bridge:0.1.0")` in `velaboard/app/build.gradle.kts` and whitelisted `com.texure` group in root mavenLocal repository filter (honoring Guardrail G1 & G2).
    - Assembled debug APK via `./gradlew :app:assembleDebug --no-daemon` and installed via `adb install -r`.
  - On-Device Execution & Lifecycle:
    - Launched `helium314.keyboard.debug/helium314.keyboard.settings.SettingsActivity`.
    - `TexureInitProvider` auto-started `BridgeService` and initialized `BridgeServer` on port `8765`, logging pairing token and binding to LAN (`10.151.242.208:8765`).
  - Bridge Wire Protocol & Mutation Verification:
    - Auth: Unauthenticated `curl http://127.0.0.1:8765/ui-tree` returned HTTP error `AUTH_FAILED` with token mismatch hint.
    - Extraction: Authenticated `curl -H "Authorization: Bearer <token>" http://127.0.0.1:8765/ui-tree` returned structured `AndroidComposeView` hierarchy + pinned debug overlay with bounds, classes, clickable states, and revision `rev: 1`.
    - Own-Window Tap Mutation: Injected `POST /tap` on node 4 ("Rotate Token"). Successfully invoked button click listener, rotated token to `2607648a9e494b26bbfb3be4d21b1a68`, and bumped tree revision to `rev: 2`. Second tap rotated token again to bump revision to `rev: 3`. No `SecurityException` thrown.
    - Screenshot: `GET /screenshot` successfully captured 360x800 PNG (56 KB).
  - Release Leak Verification: `:sample:assembleRelease` produced release APK with 0 bridge classes, server references, or overlay code.
- Verification Matrix:
  - L0: `texure-lint` pass (0 findings).
  - L1: `cargo test --workspace` pass (6 unit tests), `./gradlew testDebugUnitTest` pass (15 unit tests).
  - L2: FakeBridge n/a.
  - L3: Physical device ladder PASS on Xiaomi `IBFUSOZHCYJJ99JZ` running VelaVoice (`velaboard`).
- Status: Phase 2 Done. VERIFY: L0 pass · L1 pass · L3 pass on physical device. FLIPS: None. Map 2 fully completed!

## 2026-11-01 (Wayfinder map 2 Phase 1 HITL tickets #8 & #9 grilling & resolution)
- Map 2 Phase 1 grilling completed with human on branch `t3/wayfinder-map2-agent-flow`:
  - Ticket #8 (LLD C: MCP server internals):
    - MCP SDK choice: Hand-rolled stdio JSON-RPC 2.0 loop via `tokio` + `serde_json` (lean, ~4-6MB binary, fast compile, fits 6GB RAM; `rmcp` cut).
    - Config schema: `~/.config/texure/config.toml`, hybrid top-level keys (`url`, `token`, `timeout_secs`) + `[profiles.<name>]` table foundation for multi-device; env var override `TEXURE_*`.
    - Reconnect: Background WS supervisor with 1s->5s->15s backoff; lazy HTTP dial (3s timeout) returning `OFFLINE` if unreachable; `wait_for` falls back to 500ms HTTP polling.
    - Error map: Single-line formatted text with `isError: true` matching bridge-contract table; full JSON with `verbose: true`.
    - Screenshot: PNG bytes fetched and Base64 encoded into MCP image content block.
    - Crate layout: Workspace split into `crates/texure-contract` (shared wire types/errors) consumed by `crates/texure-server`, `crates/texure-blackbox`, and `crates/texure-lint`.
    - Deliverables: Authored `docs/lld-server.md`, updated `DECISIONS.md`, marked `docs/lld-outline.md` items 6 & 7 resolved, posted resolution comment on issue #8, closed issue #8, appended line to map #12 `Decisions so far`.
  - Ticket #9 (LLD B: Gradle plugin mechanics):
    - Debug-only wiring: Debug ContentProvider (`TexureInitProvider`) in `android/bridge` registering `Application.ActivityLifecycleCallbacks`. Zero user code required in target apps (honoring Guardrail G2), automatic overlay attachment on debug Activities, and 100% stripped in release builds.
    - Release-leak check: Dedicated `checkTexureReleaseLeak` Gradle task hooked into release build pipeline, scanning resolved compile/runtime configurations and merged manifest for `com.texure:bridge`. Hard build failure on leak (Guardrail G1).
    - Plugin distribution: Planned Gradle plugin ID `com.texure.bridge` (design in `docs/lld-plugin.md`). Not implemented: no `android/gradle-plugin` module or plugin marker exists. Only the library `com.texure:bridge:0.1.0` is published to `mavenLocal`.
    - Deliverables: Authored `docs/lld-plugin.md`, updated `DECISIONS.md`, marked `docs/lld-outline.md` item 5 resolved, posted resolution comment on issue #9, closed issue #9, appended line to map #12 `Decisions so far`.
- Phase 2 AFK unblocked: Ticket #2 (Spike 2: VelaVoice integration proof + first agent flow) is now unblocked.
- Verification: L0 (`cargo run -p texure-lint`) pass (0 findings), L1 (`cargo test --workspace` and `./gradlew testDebugUnitTest`) pass, release build `:sample:assembleRelease` pass (0 leak).
- Status: Phase 1 Done. VERIFY: L0 pass · L1 pass. NEXT: Ticket #2 AFK implementation (device required per runbook).


## 2026-10-31 (Wayfinder map 1 AFK tickets #1 & #4 implementation)
- Map 1 AFK tickets claimed and implemented on branch `wf/11-injection-spike`:
  - Ticket #1 (Spike 1 injection mechanism proof): Android library `android/bridge`, sample host app `android/sample`, root `gradlew` wrapper, embedded NanoHTTPD/NanoWSD server on fixed port 8765, serial single-thread `BridgeWorker` with verify-at-execute check, `DefaultInjectionAdapter` with `sendPointerSync` primary and `dispatchTouchEvent` fallback, `BridgeTreeExtractor` with depth cap 12 and interesting-node filtering, `BridgeOverlay` activity-scoped debug controls.
  - Ticket #4 (Crash/ANR capture): `BridgeCrashHandler` registering `UncaughtExceptionHandler`, app-private bundle export (timestamp, stack trace, last known ui-tree, last N actions, build info), `GET /last-crash` route documented in `docs/bridge-contract.md` and implemented with 200/404 semantics.
- Guardrail G1 verified: `:sample:assembleRelease` produces APK with 0 bridge classes, server references, or overlay code.
- Verification:
  - L0: `cargo run -p texure-lint` pass (0 findings), `cargo clippy --workspace` pass (0 warnings), Gradle lint vital pass.
  - L1: 15 Robolectric unit tests in `bridge` and `sample` pass, 6 cargo unit tests pass.
  - L2: n/a (FakeBridge runner target pending #3 P0).
  - L3: Blocked: device-disconnected (no adb device attached; full acceptance suite ready in `android/sample/src/androidTest/kotlin/com/texure/sample/SpikeInjectionDeviceTest.kt`).
- OCR review: completed with 21 findings, all addressed (MessageDigest constant-time token comparison, MotionEvent recycling, @Synchronized locks on token manager and single-flight lock, cleartext HTTP flag in AndroidManifest, lifecycle cleanup in MainActivity onDestroy).
- Status: Done. VERIFY: L0 pass · L1 pass · L2 n/a · L3 blocked:device-disconnected. EVIDENCE: tmp/evidence/1/red-run.txt, tmp/evidence/4/red-run.txt. FLIPS: none.

## 2026-10-30 (LLD E grilling session, orchestrator thread — same-day continuation)
- LLD E (ticket #7) claimed and grilled HITL, 5 questions, human confirmed. Decisions: verify-at-execute identity-chain recovery (mid-press refresh ⇒ NODE_OUTDATED, no semantic hashes); wait_for grammar frozen {text|tag|visible|hidden, int timeout default 10 cap 30, hidden=negative, no composition}; 16-code error table frozen + WAIT_TIMEOUT ok:false row + APP_BUSY deleted (BUSY wins) + BRIDGE_STOPPED/PORT_IN_USE/CAPTURE_FAILED/HUMAN_STEERING/NODE_NOT_FOUND given behavior rows; contract table = sole lint-parseable authority (A3 taxonomy retargeted); /ui-tree gains additive cd field (schema stays v1), security annotations rejected, scale default 0.5.
- bridge-contract.md: open questions ALL closed (port + node-id + tree-shape + scale bullets resolved). behavior-spec rows updated (1.1 shutdown+bind, 1.4 rev guard/human wins/node checks, 1.5 capture failure). lld-outline items 2+3 RESOLVED. DECISIONS.md round-8 entry.
- Tracker: #7 resolution comment + closed completed; map #11 Decisions-so-far second line. ERROR fixed again mid-flight: repeated the stray-PATCH body clobber on #7 (this time caught and restored immediately from the pre-patch body). Procedure fix adopted for next tickets: comment via POST only, PATCH never for bodies during close-out.
- Doc changes still UNCOMMITTED (attribution rule) — batch now spans LLD A + LLD E on orchestrator checkout.
- Map 1 Phase 1 CLEAR: both grilling tickets closed; Spike 1 (#1) + crash capture (#4) are frontier for the map-1 AFK worker (device required per runbook).
- Status: Done (ticket #7 closed) — VERIFY: docs-only, L0–L3 n/a; FLIPS: none. NEXT: map-1 AFK worker DISPATCH HELD by user (2026-10-30) — starts only when they say go.

## 2026-10-30 (LLD A grilling session, orchestrator thread)
- LLD A (ticket #6) claimed (`assignee: parth5012`) and grilled HITL, 8 questions, human confirmed. Decisions: NanoHTTPD+ws engine; serial bridge worker (sendPointerSync direct / dispatchTouchEvent+tree main-hop latch 1s); coords-in injection adapter, auto-fallback inside, no agent knob; activity-scoped overlay, zero grants; FGS specialUse; port fixed 8765 + SO_REUSEADDR + loud PORT_IN_USE (port bullet removed from #7 scope); token plain app-private prefs allowBackup=false; user Stop = hard abort, candidate code BRIDGE_STOPPED handed to #7 error table.
- Authored docs/lld-bridge.md; freeze rows + port open-question closure in docs/bridge-contract.md; lld-outline item 4 RESOLVED; DECISIONS.md round-7 entry.
- Tracker: #6 resolution comment posted + closed completed; map #11 Decisions-so-far first line appended; #7 scope updated (port resolved). ERROR fixed: a stray PATCH briefly overwrote ticket #6 body with the resolution text — restored from the original body I'd fetched at grill start.
- Doc changes UNCOMMITTED on orchestrator checkout (attribution rule: no commit without explicit ask) — batch after LLD E closes or as docs PR.
- Status: Done (ticket #6 closed) — VERIFY: docs-only, L0–L3 n/a; FLIPS: none. NEXT: LLD E (#7) grilling, then map-1 AFK spike.

## 2026-10-30 (orchestrator session, wayfinder ship)

- Data repair (docs-only, no issue): map bodies #11/#12/#13 were EMPTY (creation bug from the wayfinder-conversion session) — restored Destination/Notes/children/blocking/decisions/fog from LOG.md + docs. Inverted `blocked by:` footers on grilling tickets #6/#7 fixed (they had claimed to be blocked by the implementation tickets they feed; body Sequencing is canon). Map restored + footer fix noted in map bodies.
- Frontier computed: map 3 (#13) is the only pure-AFK map — #3 P0, #5 P0, #10 workflow-shell unblocked NOW; P1s + nightly gated on code from maps 1–2. Maps 1–2 open with HITL grilling tickets (LLD A #6, LLD E #7, then LLD C #8, LLD B #9).
- Worker thread #1 dispatched on user instruction (gemini flash child model: opencode/omniroute/gemini-flash, fresh worktree off main): wayfinder-ship map 3 end-to-end, single-PR mode, tickets one at a time with texure-implementation-loop. Thread link: <t3-thread://v2/1b2b1840-8750-47a4-a5cc-9ad9f5873c5b/mcp:1cef90f0-ad8b-4a49-a33e-82a0c4c6986b>. One thread at a time per orchestrator policy; next queue: map 1 Phase-1 grilling with the human.
- Status: Review (awaiting human).

## 2026-10-30
- Repo created from grilling session. Docs only: README, ARCHITECTURE, docs/bridge-contract.md, docs/lld-outline.md, DECISIONS.md.
- Status: Ready. Next: LLD dig-in starting with injection mechanism spike decision; code after contract freeze.
- Renamed to texure; GitHub repo created (parth5012/texure, public).
- LLD round 2 decisions baked into contract (rev guard, /ws + wait_for, fg service, overlay pairing, login/text flow).
- Injection research done inline (subagent infra failed twice): own-window injection needs no INJECT_EVENTS; AccessibilityService rejected; spike spec written.
- Spike placement corrected: throwaway code lives in texure/android/sample, never in VelaVoice.
- ROADMAP.md added (now/next/later/not-doing).
- Spike scaffold request PARKED by user as GitHub issues: #1 spike 1 (bridge + sample + 5 criteria), #2 spike 2 (VelaVoice one-liner + agent flow). Implementation deferred.
- AGENTS.md designed per agents-md skill with user merges. docs/repo-map.md (layout + commands with honest status column) and docs/guardrails.md (13 hard limits) split out per user call. Attribution omitted until toolchain setup completes.
- Custom linting designed via grilling round 4: two classes (docs-gate Rust crate + ast-grep YAML guardrails), perfgym dogfood time-boxed with sg fallback, FAIL/WARN tiers, in-repo lint/ rules. Plan in docs/linting-plan.md, parked as issue #5, ROADMAP.Next entry added. Skills section added to AGENTS.md (CLAUDE.md re-synced).
- NOTE: CLAUDE.md is a plain COPY of AGENTS.md (Windows symlink needs elevated privilege). Re-copy whenever AGENTS.md changes.
- docs/testing-architecture.md added: test plan through the three seams, FakeBridge second adapter, scenario DSL, agent eval harness; build order mapped to issues #1/#2.
- docs/blackbox-plan.md added + implementation parked as issue #3 (BB runner P0/P1). Suite is write-first: runs red before implementation, flips green across #1/#2 milestones.
- Blackbox design audited for completeness: 6 gaps found and closed (scenario/assertion DSL v0 schema, tool-freeze + token-budget + token-rotation test IDs A0/A6/B11, report/CI contract, sequential-per-target rule, plugin-guard ownership boundary). Issue #3 updated with the additions.- Behavior spec authored (docs/behavior-spec.md) via LLD round 3 grilling; DECISIONS carries 10+ entries; docs now: architecture, contract, behavior, spike, testing, blackbox plan.
- ROADMAP extended with user brainstorm: record+replay, regression tracking, crash capture, ADB bootstrap, screen mirroring (scrcpy MCP), config/deep-link matrices, perf probes, assertion SDK, trace viewer.
- User refinements applied: record+replay also exports codemode scripts for agent self-scripting; crash/ANR capture promoted to ASAP tier (issue #4); ADB confirmed optional-only, never a runtime dependency; deep-link matrix explained and kept Later.
- CI/CD + distribution and device runbook planned via grilling rounds 5+6. docs/lld-ci.md (gates: lint+rust-test PR gates, ubuntu+windows per PR, mac at tag, prebuilt+cargo Rust dist, mavenLocal+in-repo examples Android dist, tag release recipe, T3 scheduled tasks nightly opens issues) and docs/runbook-device.md (executable checklist, one-time MIUI hardening vs ladder B0..B7, WiFi+hotspot fallback, ADB setup-only, evidence bundles per red run). lld-outline items 8+9 resolved; ROADMAP.Later gains quickstart-repo + JitPack/Central + cargo-dist deferrals.
- Remaining planning surfaces parked as issues: #6 bridge internals LLD (A), #7 contract freeze leftovers (E), #8 MCP server internals LLD (C), #9 Gradle plugin LLD (B), #10 CI matrix + nightly implementation. All planning-only, gates + sequencing noted; do not run until user says go.
- Wayfinder conversion (session 2026-10-30): issues #1-#10 annotated as wayfinder tickets (labels wayfinder:task / wayfinder:grilling + type/mode/blocked-by metadata appended to bodies). Three maps created: #11 map 1 bridge+contract->spike proof, #12 map 2 server+plugin->first agent flow, #13 map 3 quality rails (pure AFK handover target). Maps encode the docs+skills each runner must load; maps run only when the user is present for HITL phases.
- Agent verification protocol defined: docs/agent-test-loop.md (L0 static -> L1 unit -> L2 fake blackbox self-serve; L3 device only via runbook ladder; red = evidence bundle + BLOCKED; verify block REQUIRED in ticket resolution comments). Wired into AGENTS.md/CLAUDE.md testing rules, repo-map doc list + commands table (device regression + lint commands added), and all three map bodies (#11-#13) as a Notes bullet.
- project-implementation-loop skill extracted (session 2026-10-30): full ticket-level implement+verify+review loop (red blackbox/unit -> minimum impl -> bounded L0-L3 gauntlet -> adversarial review lanes incl. ocr-review + openqodex + attack-brief subagents -> CodeRabbit/parthpr bot + babysit-pr -> close-out artifacts). Installed at C:/Users/DELL/.agents/skills/ and pushed to personal-skills (cda05be). Maps #11-#13 AFK runners should load it as the per-ticket execution playbook.
- implementation-loop-builder (meta skill) created: scans a project + grills forks + drafts/validates/installs a tailored <project>-implementation-loop skill. Retro-applied: texure-implementation-loop generated (L0-L3 ladder, blackbox write-first contract, texure-lint docs-gate, evidence bundles, wayfinder stacking) at .agents/skills/ + user-level + personal-skills. Generic project-implementation-loop replaced by builder in personal-skills. AGENTS.md Skills section points at the loop skill.
- Implementation run for Wayfinder map 3 (quality rails) via wayfinder-ship skill:
  - Branch wf/13-quality-rails created off main.
  - Ticket #3 P0 landed: crates/texure-blackbox CLI runner, scenario DSL v0 parser/validator, 21 BB-* scenario definitions (15 FAIL, 6 SKIP on unstarted bridge/device, exit code 1 as write-first red contract), golden fixtures under contract/fixtures/, machine-readable JSON report verified.
  - Ticket #5 P0 landed: crates/texure-lint with bidirectional docs-gate against docs/behavior-spec.md (all 21 BB test IDs mapped) and docs/bridge-contract.md routes, scenario DSL validation, hygiene checks. Clean 0 findings report verified.
  - Ticket #10 landed: .github/workflows/ci.yml matrix workflow shell with lint, rust-test (Ubuntu + Windows), android-build clean skip probe, and release tag targets. Issue #10 closed.
  - Clippy, rustfmt, cargo test, and texure-lint gauntlet green across workspace.
  - PR opened to main, bot reviews requested, babysit active.
