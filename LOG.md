# Log

## 2026-11-12 (session) — Wayfinder map 7 (#28): PR #92 babysit + ticket #27 AFK ship
- Map 7 pre-flight: gh auth ok, git ok. OCR LLM gateway (100.87.250.35:20128, debian host) OFFLINE 15h → OCR lane ran degraded (`ocr delegate preview/rule`, rule-based, no LLM); findings applied by host agent. Both worktrees: wf-92-fix + wf-27-trace.
- PR #92 (ticket #26 history store) was OPEN + CONFLICTING + lint-gate red. Rebased onto main (4f9e252): resolved 3 conflicts keeping recording+history modules and Record/Replay CLI. Applied 6 CodeRabbit findings: unique sub-second run IDs, plain INSERT (no row clobber), git rev-parse status/non-empty checks, ast-grep extension-neutral-first + .cmd retry, npx.cmd→npx NotFound retry, cargo fmt. Separated unrelated texure-server fmt drift into its own style commit. L0 lint 0 + fmt clean · L1 blackbox 12/12 + workspace green · L2 mcp-fake 14/14. Pushed 2d6b336+90c3638.
- Babysit round: CI clippy 1.99 map_identity flagged 2 pre-existing texure-server sites (came from map 8 merges) → removed identity map_err (50c754a), server tests 13/13. Watching PR #92.
- Ticket #27 (trace viewer + trend) implemented by delegate agent on wf/28-27-trace-viewer (base 90c3638): trace.rs (707L, hand-rolled templates, embed_json script-safe escaping), runner StepTrace capture across mcp/mcp-real/http walks, step_records now persisted (was &[]), --trace/--trend/--out flags, get_scenario_stats flaky aggregation. TDD red: tmp/evidence/27/red-run.txt (7 failing).
- Self-review 2 findings applied (f000c01): detail pane + step list built via DOM textContent instead of innerHTML (embedded-JSON XSS hardening), new no-innerHTML unit test; clippy too_many_arguments allow on recorder, checked_div, needless deref. OCR delegate rule lane: nothing new.
- VERIFY: L0 fmt clean, texure-lint 0, clippy clean (1.99) · L1 blackbox 22/22, workspace 65/65 · L2 mcp-fake 14/14 · --trace run_20261112_044347.412 → tmp/evidence/27/trace.html (23KB, self-contained, 0 external URLs), --trend → tmp/evidence/27/trend.html.
- Status: #27 Done pending PR. NEXT: file stacked PR (base wf/28-26-history-store), babysit both PRs.

## 2026-11-10 (session) — Wayfinder map 8 (#36) ticket #33 QR overlay
- Branch wf/36-33-qr-overlay off wf/36-32-mdns 4ef4616 (stack leg 5/7). Implemented pure-Kotlin QR generator (versions 1-4, byte mode, ECC L) with Reed-Solomon over GF(256), plus "Show QR" toggle in BridgeOverlay rendering QR bitmap. Updated behavior-spec overlay row + ROADMAP.
- TDD red: QrCodeTest (square matrix, finder patterns, deterministic, too-large null, version growth) captured red-run at tmp/evidence/33/red-run.txt (4 failures before impl).
- VERIFY: L0 cargo run -p texure-lint 0 findings; cargo clippy 0; gradle lintDebug pass · L1 cargo test --workspace 12 passed; :bridge:testDebugUnitTest (QrCodeTest + existing) pass · L2 cargo run -p texure-blackbox --target mcp-fake 14/14 · G1 :sample:assembleRelease pass (0 bridge strings). L3 parked (no device).
- EVIDENCE: tmp/evidence/33/red-run.txt. FLIPS: 5 Robolectric (QrCodeTest) red→green. NEXT: orchestrator self-review; #34/#35 remain.

## 2026-11-09 (session) — Close Map 5 (#22) + Wayfinder map 8 (#36) ship start
- Issue #22 (Map 5 Gradle plugin) closed as completed: ticket #21 CLOSED, PR #95 MERGED 2026-11-08 (wf/22-gradle-plugin → main). Resolution comment posted.
- Map 8 (#36) pre-flight: gh auth ok, git inside work tree ok (HEAD db3ced1 == origin/main). OCR gateway omniroute.service was inactive → started; OCR config url fixed localhost:20128 → 100.87.250.35:20128 (gateway binds Tailscale IP only); model gemini-3.8-flash-tiered lacks antigravity creds → switched to auto/best-coding (command-a-reasoning). `ocr llm test` passes.
- Map 8 is pure AFK (7 code-changing tickets #29–#35) → stacked-PR mode (≥5). Frontier: #29–#34 unblocked, #35 blocked by #29,#30,#31. Topo order: #29,#30,#31,#32,#33,#34,#35.
- Stack branches: wf/36-29-semantic-hashes → wf/36-30-screenshot-region → wf/36-31-post-tag → wf/36-32-mdns → wf/36-33-qr-overlay → wf/36-34-multidevice-profiles → wf/36-35-blackbox-map8. No Rust/Android best-practice skill installed for this layer — proceeded with layer docs; gap noted per AGENTS.md.
- Ticket #29 (semantic hashes) implemented on wf/36-29-semantic-hashes (base origin/main db3ced1). FNV-1a 64 hex over ancestor path/id/text/cls; UiNode.hash additive nullable (schema 1); /tap|/type hash-or-index verify rev→index-on-hash→hash-resolve→NODE_OUTDATED; subtree ancestry preserved (ancestorPathTo). Self-review 2 iterations: B1 DECISIONS gate satisfied, B2 subtree resolve fixed + subtree-invariance test added (after fixing tag-ordering test bug + direct-child prefix edge). OCR 3 findings parked (perf/map suggestion, docs already satisfied, class-fallback collision tradeoff). VERIFY: L0 lint 0, clippy 0, gradle lint pass · L1 cargo 39 (22 contract) + bridge 39 (6 SemanticHash incl subtree, 6 HashTargeting) · L2 mcp-fake 14/14 · G1 release 0 bridge strings · L3 parked (no device). EVIDENCE: tmp/evidence/29/red-run.txt. FLIPS: 7 Rust + 11 Robolectric red→green.
- Status: Map 5 Done. Map 8 Phase 1 (HITL) none — Phase 2 AFK: #29 Done (PR filed, babysit), #30–#34 frontier, #35 blocked by #29/#30/#31 (now unblocked for #29 leg only — still blocked by #30/#31). NEXT: file stacked PR for #29 → babysit → branch #30.
- Ticket #30 (screenshot region) done on wf/36-30-screenshot-region (base wf/36-29). Crop-then-scale, clamp, 400 on malformed, recycle. Self-review code-clean iter1; OCR LLM failed (gateway timeout) → delegate preview (degraded mode, documented). VERIFY: L0 lint 0 · L1 cargo 39 + BridgeServerTest 13 · L2 mcp-fake 14/14 · G1 0 strings · L3 parked. EVIDENCE: tmp/evidence/30/. Issues #29/#30 closed, map #36 Decisions updated (2/7).
- Merge (stack 1-2/7): PR #97 (Stable semantic node hashes) + PR #98 (Partial-region screenshot) rebased bottom-up, merged into main (2821142). Branches deleted. Map #36 briefly closed per user (partial 2/7 note), then reopened per user — tickets #31–#35 remain open.
- Status: Map 8 Phase 2 in progress (2/7 merged). NEXT: ticket #31 Dynamic view tagging (POST /tag) on branch wf/36-31-post-tag off origin/main; then #32/#33/#34/#35 in topo order.

## 2026-11-07 (Wayfinder map 5 (#22) — Gradle plugin ticket #21, single-PR mode)
- branch `wf/22-gradle-plugin` off origin/main (b35619a); OCR note: local OCR LLM gateway (localhost:20128) unreachable, so the OCR lane ran in delegate mode (`ocr delegate preview/rule` — no LLM), findings applied by host agent; CodeRabbit/parthpr still requested on the PR.
- Ticket #21 (gradle-plugin) TDD: red detector tests → minimal impl → green. Module `android/gradle-plugin` standalone build (compose includeBuild): plugin ID com.texure.bridge, debugImplementation injection (G2), `checkTexureReleaseLeak<Variant>` per release variant + aggregate, scanning release runtime/compile classpaths + merged manifest, GradleException on hit/missing input, never upToDate-skip. Published marker+artifact to mavenLocal.
- Sample app adopted via one plugins-block line; direct project(":bridge") dep removed (G2 one-liner path).
- Self-review iterations: 2 (code-review Standards+Spec agents; concrete fixes applied: package/bundle hook point, missing-input loud failure, LLD-verbatim "Found dependency:" line, doc §refs). Parked (quoted on ticket): aggregate task + extra manifest token beyond LLD (both justified by acceptance command + stricter-than-LLD scan), stringly-typed config names, app/lib onVariants duplication.
- Gauntlet: L0 texure-lint 0 findings; L1 plugin tests green, :bridge:testDebugUnitTest green; L2 (command-surface equivalent for this surface): :gradle-plugin:publishToMavenLocal ok, :sample:assembleDebug includes bridge (82 dex markers), clean :sample:assembleRelease 0 bridge classes, leak-demo forced releaseImplementation → hard G1 failure with exact LLD diagnostics. L3 n/a (no device surface; blackbox has no Gradle scenario kind — BB-G1 mapping parked to blackbox suite scope, issue #3).
- Evidence: tmp/evidence/21/red-run.txt, leak-demo-fail.txt.
- Babysit round 1: Android CI job red — sample resolves the bridge AAR from mavenLocal but CI never published; added mavenLocal publish step (both modules, per lld-ci.md §3 recipe) before assembleDebug. Parked: adding assembleRelease + G1 leak-check to the CI job's surface (CI matrix contract, issue #10 scope).
- Babysit round 2: CodeRabbit 5 findings — applied project-component scan (Major: `project(":bridge")` in release now fails G1) + exclusiveContent mavenLocal filter (Minor); parked verbatim on ticket #21: config-cache input rewiring (Major, own follow-up ticket), shipping-buildtype DSL (Minor, LLD extension), CI release-leak test lane (Minor, issue #10 scope). Gauntlet re-run green (tests/publish/assembleDebug/assembleRelease 0 markers/bridge tests/lint).
- Merge: PR #95 merged into main (rebase, all 7 checks green on c6e5f23; mergeState CLEAN; user-ordered "merge"). Remote branch deleted by merge. Map 5 (#22) Destination reached — adopted-one-line plugin is distributed via mavenLocal; remaining parked items: config-cache rewiring + shipping-buildtype DSL + CI release-leak lane (candidates for follow-up tickets).
- Status: Done (ticket #21 close-out per full loop) — VERIFY: L0 pass · L1 pass · L2 pass(command surface) · L3 n/a. EVIDENCE: tmp/evidence/21/. FLIPS: none. NEXT: PR + babysit.

## 2026-11-07 (Standalone MCP probe + map 4 merge)
- Added `tools/test_mcp_client.py` (issue #94): stdlib-only test MCP client — in-process FakeBridge (auth, rev-guard, PNG, polling fallback) → real `texure-server` over stdio JSON-RPC; 19 checks covering handshake/tools-list, all 6 tools, NO_CONFIG/OFFLINE/AUTH_FAILED, verbose:true, -32602/-32700, daemon-survives-errors. Recorded run 19/19 PASS (2.1s); report at https://cron-system.vercel.app/mcp-probe/ (tmp/mcp-probe/ gitignored).
- Verified pre-commit: probe 19/19, `cargo run -p texure-lint` 0 findings, `cargo test --workspace` green (28 passed).
- Committed probe + LOG onto PR #93 branch for merge into main (map 4 #18+#19+#94).

## 2026-11-02 (Wayfinder Map 7 Ticket #26 AFK implementation & verification)
- Executed and resolved Map 7 Ticket #26 (Regression tracking datastore: embedded SQLite run history & query CLI) on branch `wf/28-26-history-store`:
  - SQLite storage engine in `crates/texure-blackbox/src/history.rs`:
    - Auto-initialized `.texure/history.db` with WAL mode, normal synchronous, foreign keys, and busy timeout.
    - Schema: `runs`, `scenario_results`, and `step_records` tables with query indexes.
    - Added `record_run`, `list_runs`, `get_run`, `get_scenario_steps`, and CLI table/JSON renderer.
  - Blackbox Runner Integration (`crates/texure-blackbox/src/main.rs`):
    - Added `--history`, `--limit`, `--format`, and `--history-db` CLI flags.
    - Auto-recorded run summary, test outcomes, error codes, and evidence paths into SQLite on suite execution.
  - Cross-platform Windows batch execution fix in `crates/texure-lint/src/ast_grep.rs` for `npx.cmd` and `ast-grep.cmd`.
  - Added `.texure/` to `.gitignore`.
  - Verification Matrix:
    - L0: `cargo run -p texure-lint` pass (0 findings).
    - L1: `cargo test -p texure-blackbox` pass (8 unit tests passed).
    - L2: `cargo run -p texure-blackbox -- --target mcp-fake` pass (14/14 passed) + `--history` verified.
- Status: Ticket #26 Done. Ready for PR.

## 2026-11-02 (Wayfinder map 3 tickets #3 & #5 AFK resolution)
- Executed and resolved remaining map 3 P1 tickets on branch `t3/wayfinder-map3-leftovers`:
  - Ticket #3 (BB-P0/P1: blackbox runner + FakeBridge scenario suite):
    - Implemented in-process HTTP and WebSocket FakeBridge in `crates/texure-blackbox/src/fake_bridge.rs` supporting all contract endpoints and scenario states.
    - Implemented `FakeMcpServer` in `crates/texure-blackbox/src/fake_mcp.rs` for stdio JSON-RPC 2.0 conformance testing with frozen 6-tool schema and scenario error simulation.
    - Implemented blackbox scenario runner in `crates/texure-blackbox/src/runner.rs`.
    - Applied concrete OCR review findings (bounded buffers, WebSocket peek consumption fix, notification handling).
    - Verified: `cargo run -p texure-blackbox -- --target mcp-fake`: 14/14 PASS (39ms); `cargo run -p texure-blackbox -- --target all`: 15 PASS, 6 DEVICE-SKIP, 0 FAIL.
    - Ticket closed and committed.
  - Ticket #5 (Custom linters: texure-lint P0 + ast-grep rules P1):
    - Authored YAML AST pattern rules in `lint/ast-grep/` (`g1-release-leak.yml`, `g4-no-adb-system-exec.yml`, `g5-g6-forbidden-injection.yml`) mapped to Guardrails G1, G4, G5/G6.
    - Created root `sgconfig.yml` pointing to `lint/ast-grep`.
    - Integrated Class C executor into `crates/texure-lint/src/ast_grep.rs` with perfgym dogfood attempt and CLI/npx `ast-grep scan --json` fallback.
    - Logged perfgym dogfood experiment evaluation in `DECISIONS.md`.
    - Verified: `cargo run -p texure-lint -- --report tmp/evidence/5/lint-report.json`: 0 findings (0 FAIL, 0 WARN).
- Verification: L0 (`texure-lint`, clippy 0 warnings), L1 (`cargo test --workspace` 9 passed), L2 (14/14 blackbox fake passed).
- Status: Map 3 Phase 2 AFK Done. Single PR ready to open.

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
- Wayfinder map 4 (#20) run via wayfinder-ship: ticket #18 (texure-contract, PR merged e837129) + ticket #19 (texure-server) on branch t3/fbaa7cb6.
  - Ticket #19 landed: crates/texure-server MCP stdio server (6 frozen tools, TOML config --config/TEXURE_CONFIG/env, lazy 3s dial, WS supervisor 1s/5s/15s pushing tree_changed into wait_for with 500ms HTTP poll fallback, single-flight daemon); blackbox --target mcp-real spawns the real binary against FakeBridge with temp-config isolation (NO_CONFIG via absent path, auth_reject via wrong token); workspace member added.
  - Gauntlet green: L0 texure-lint 0 findings + clippy clean; L1 26 unit tests; L2 mcp-real 15/15 (~2.1s), --target all 36 rows: 30 passed, 6 device-skipped, 0 failed. Evidence: tmp/evidence/19/.
  - Self-review (code-review Standards+Spec + OCR): fixed contract one-liner hint (CODE: msg; hint), verbose payload = bridge envelope verbatim + bridge_url/ws_connected/http_status-only-when-real, unknown-tool/bad-args -> JSON-RPC -32602 (no fabricated bridge codes), OFFLINE/BRIDGE_STOPPED split + 401/409/5xx fallback mapping, screenshot Content-Type PNG validation, malformed-TOML tracing::warn, lld-server.md config surface + deps. Parked findings quoted on ticket #19.

## 2026-11-08 (session) — Wayfinder map 6 (#25), ticket #23 via wayfinder-ship
- Status: Done (ticket #23). Branch wf/25-map6 (single-PR mode: 2 code-changing AFK tickets < 5).
- Pre-flight: gh auth ok; OCR LLM backend DOWN (localhost:20128 refused, no API keys) => documented degraded mode: ocr delegate (rules/preview, no LLM) + CodeReviewer lane. Delegated subagent execution also unavailable (provider connection refused) => implemented in-session instead of dispatching `general` coder agents.
- Commits: bc5e8a4 docs gate (contract /recording/* rows, behavior-spec 1.7 + BB-B12..BB-B15 mapping, DECISIONS entry — schema_version stays 1, additive); 9926c2b implementation.
- Design notes: capture = Window.Callback wrapper (dispatchTouchEvent + dispatchKeyEvent + KeyEvent.Callback fallback) — the pass-through capture-view design was abandoned after discovery that nested re-entrant decorView dispatch corrupts ViewGroup touch-target state (NPE, had failing evidence). Recording routes exempt from single-flight lock (control-plane). NanoHTTPD keep-alive requires draining POST bodies or the next request line breaks ("HTTP verb {}POST unhandled").
- VERIFY: L0 pass · L1 pass (27/27 bridge, 13 new) · L2 pass (mcp-fake 14/14) · L3 parked (no device) · G1 release leak check pass.
- Lint found BB-R group naming invalid (docs-gate regex only accepts BB-[A-C]) — renumbered to BB-B12..15 rather than extending the lint surface.
- OCR unused-found: SYNTHETIC_DEVICE_ID constant existed but was never stamped onto injections; stamped at craft time this session (completes behavior-spec 1.4 marker row).
- Next in session: ticket #24 (CLI record/replay + codemode export, FakeBridge mocks, blackbox scenarios).

## 2026-11-09 (session) — Wayfinder map 6 (#25), ticket #24 via wayfinder-ship
- Status: Done (ticket #24). Same branch `wf/25-map6` (completes single PR).
- Commits: b203c1c (recording.rs exporter+replay core, record/replay CLI, FakeBridge /recording mocks, DSL actions, BB-B12..B15 scenarios+TestDefs, lint allowed-actions) + one cosmetic amend.
- TDD: red evidence tmp/evidence/red-run-24.txt (25 compile errors + fake 404 test). Replay verify-at-execute caught node-2-out-of-range in draft record_replay.json (fixture has 2 nodes) — fixed scenario, then 7/7 pass-rate 100%.
- L2 notes: FakeBridge records mutations while recording as synthetic actions (deterministic replay source); real bridge excludes agent events (verified L1/L3 in #23). mcp-real skips recording tests with reason (HTTP-only routes).
- VERIFY: L0 pass (texure-lint 0) · L1 pass (9/9 blackbox + 4/4 lint unit) · L2 pass (--target all 34/0/4-skip/6-device-skip; --target http 5/5) · L3 parked (no device) · CLI record/replay smoke pass.
- Review: self-review (OCR backend down, documented degraded mode). Remaining: checkpoint PR.

## 2026-11-12 (session) — Wayfinder map 8, ticket #27 via wayfinder subagent
- Status: Done (ticket #27). Branch wf/28-27-trace-viewer stacked on 90c3638 (tip of wf/28-26-history-store). No push, no PR (per orchestrator instruction).
- Scope: HTML trace viewer CLI + regression trend dashboard + suite-runner per-step persistence into history step_records.
- TDD: red first — 7 failing tests (trace HTML scrubber/error/self-contained/escaping, trend flaky rows, scenario-stats aggregation) captured in tmp/evidence/27/red-run.txt; then GREEN.
- Files: crates/texure-blackbox/src/trace.rs (new, HTML templates + tests), history.rs (ScenarioTrend + get_scenario_stats + tests), runner.rs (StepTrace capture through all three walk targets), main.rs (--trace/--trend/--out flags + step record wiring).
- CLI decision (DECISIONS.md): --trend (not ticket's literal --report, which was taken by the JSON report flag).
- VERIFY: L0 pass (texure-lint 0 findings, cargo fmt clean, cargo check 0 warnings; clippy unavailable on this toolchain — not installed for stable-x86_64-pc-windows-gnu) · L1 pass (21 blackbox + 26 server + 4 lint + 13 contract) · L2 pass (--target mcp-fake 14/14, step records persisted: 19 rows verified in .texure/history.db with tool names/rev/offsets) · L3 n/a (no device surface touched) · artifacts tmp/evidence/27/{trace.html,trend.html,trace-db-path.html} with zero external URLs.
