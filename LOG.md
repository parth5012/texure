# Log

## 2026-11-19 (session) — Wayfinder map 14 (#69) In-app network interception & dynamic API mocking
- Scope: 4 AFK tickets (#65, #66, #67, #68) in single-PR mode on branch `wf/69-network-mocking`.
- Ticket #65: AGP ASM bytecode weaver for OkHttpClient in debug builds with build-time opt-out toggle (`texure { networkMocking = false }` or `networkMocking(false)`). ASM visitor targets `OkHttpClient.Builder.build()` with idempotency guard. Closed #65.
- Ticket #66: Core `TexureNetworkInterceptor` and dynamic regex mock rule engine in `android/bridge`. Passthrough by default, synthetic responses with delay and times expiration, 100-request synchronized ring buffer. Closed #66.
- Ticket #67: Bridge HTTP endpoints `POST /network/mocks`, `DELETE /network/mocks`, `GET /network/history`, `POST /network/mode`. Closed #67.
- Ticket #68: Blackbox scenario suite & FakeBridge mocks for network mocking. Added Group N behavior rows BB-N1, BB-N2, BB-N3 in `behavior-spec.md`, documented contract in `bridge-contract.md`, created scenarios `network_mock_response.json`, `network_history.json`, `network_mode_passthrough.json`. Closed #68.
- Council Review (Round 1): Convened 3-member Council (Gemini 3.8 Flash, Hy4 Flash, DeepSeek V4.1 Flash):
  - Round 1 Scores: Gemini 3.8 Flash (7/10), Hy4 Flash (6.5/10), DeepSeek V4.1 Flash (6.5/10) — Verdict: REQUESTING CHANGES.
  - Concrete fixes applied:
    - Fixed full blackbox suite runner skip filter in `runner.rs` for `network_*` actions on `mcp-real` target (`cargo run -p texure-blackbox -- --target all` now passes 55/55 non-skipped tests).
    - Hardened request body capture in `TexureNetworkInterceptor`: stream safety for `isOneShot()`/`isDuplex()`, capped capture at 64 KB via BoundedSink without memory blowup, immediate omission for bodies exceeding 64 KB.
    - Added classpath safety in `BridgeServer.kt`: `isOkHttpAvailable()` check preventing unhandled `NoClassDefFoundError` when OkHttp is absent.
    - Added validation for `status` (100..599), `delay_ms` (0..30000ms), and `times` (> 0 integer) returning 400 `BAD_REQUEST`.
    - Added reset of mock rules and request history on `BridgeServer.stop()`.
    - Aligned FakeBridge error codes to `BAD_REQUEST`, supported wildcard `*` method matching, added payload flat/nested body parsing, and echoed headers in mock responses.
    - Enhanced BB-N1 and BB-N2 scenarios to strictly assert headers and `history.0.request_body`.
    - Supported Kotlin DSL `texure { networkMocking(false) }` and added visitor tests for dedupe guard instructions.
    - Updated `gradle-plugin` to declare ASM `implementation`.
- Council Review (Round 2):
  - Round 2 Scores: Gemini 3.8 Flash (10/10 SATISFIED), Hy4 Flash (8.5/10 SATISFIED), DeepSeek V4.1 Flash (Round 2 feedback fully resolved):
    - Added in-tree JVM integration test in `TexureOkHttpVisitorTest` weaving real `okhttp3.OkHttpClient$Builder` from `okhttp-4.12.0.jar` with `COMPUTE_FRAMES` StackMap verification, asserting interceptor attachment and deduplication on `newBuilder().build()`.
    - Added OkHttp dependency to `sample` debug builds, validating AGP ASM class transformation and Guardrail G1 release-leak check.
    - Added diagnostic `POST /network/execute` and `DELETE /network/history` endpoints to both `BridgeServer` and `FakeBridge`, with `X-Mock-Source: Texure` header parity and percent-decoded query params.
    - Hardened body capture: zero-overhead immediate passthrough when `!mockingEnabled`, one-shot/duplex placeholder guards, 64KB cap via `ForwardingSink`, and zero false truncation on exactly 64KB bodies.
    - Enforced strict integer validation for `status` (200..599), `times` (> 0), `delay_ms` (0..30000ms), and 200 mock rules capacity cap across both `BridgeServer` and `FakeBridge`.
- Verification: `cargo run -p texure-lint` (0 findings), `cargo run -p texure-blackbox -- --target all` (55 passed, 0 failed, 8 skipped, 6 device-skipped), `cargo test --workspace` (all passed), Android `./gradlew testDebugUnitTest :gradle-plugin:test` (all 99 tasks passed), Guardrail G1 `:sample:checkTexureReleaseLeakRelease` (passed).
- Status: Map #14 tickets #65..#68 complete; Council satisfied (Gemini 10/10, Hy4 8.5/10, DeepSeek Round 2 feedback resolved); ready for PR.

## 2026-11-18 (session) — Council round 3 close-out (map #64 stack)
- Round 3 respawn: DeepSeek V4.1 Flash 9/10 SATISFIED — independently reproduced 150 workspace tests, clippy/fmt/lint, 13/13 mcp-tunnel, and two out-of-tree repros proving (a) silent tunnel → LAN fallback tree rev:42 and (b) gateway-only order → OFFLINE with no rev, so BB-R4/R5 FAIL. Round-2 F1 confirmed genuinely fixed.
- GLM 5.3 Flash unavailable (model no longer advertised by any provider); Gemini 3.8 Flash unavailable (Antigravity quota + provider-session open failure). No substitution or invented scores.
- Final state: 3 of 4 members satisfied (Hy4 9/10, DeepSeek 9/10, GLM 8.5/10 in round 2). All blocking findings across 3 rounds fixed. Non-blocking follow-ups recorded: no blackbox scenario for fallback-on-failure, bare-reply stale-reply risk for non-conforming companions, heartbeat cadence vs fixture clarification, decorative transport_resolve args.
- Doc drift fixed: blackbox-plan R12 scope, repo-map router test count (13). VERIFY: texure-lint 0 · workspace 150 tests · clippy clean · fmt clean · mcp-tunnel 13/13. Council section added to all 5 PR descriptions via REST API.

## 2026-11-17 (session) — Council round 2 fixes (map #64 stack)
- DeepSeek 7/10 NO: round-1 B2 fix was still exploitable — with lan_direct in every tunnel scenario's transport_order, a silent tunnel timed out and the FakeBridge tree (same rev:42) satisfied `rev > 0`, so BB-R4 passed while the gateway proxied nothing. Gateway scenarios now get `transport_order = ["reverse_ws"]` (only BB-R12 transport_resolve keeps lan_direct), so a silent tunnel yields OFFLINE and fails the oracle; B1 fallback remains covered by BB-R12 + the repro.
- Hy4 9/10 YES (blockers RESOLVED). Non-blocking replay risk addressed: `keypress` (no rev guard) is excluded from fallback retry to avoid double execution; rev-guarded tap/type and reads still fall over.
- Docs traceability: behavior-spec §3R R3 and bridge-contract item 1 now annotate that exponential backoff is companion/L3 scope while BB-R3 L2 asserts re-registration only.
- VERIFY: texure-lint 0 · workspace 148 tests · clippy clean · fmt clean · mcp-tunnel 13/13 · B1 repro still returns LAN rev:42.

## 2026-11-17 (session) — Council round 1 fixes (map #64 stack)
- Council: Hy4 Free 8/10 YES, GLM 5.3 Flash 8/10 YES, DeepSeek V4.1 Flash 6/10 NO; Gemini 3.8 Flash unavailable (Antigravity individual quota, resets ~86h). Pre-audit: applied CodeRabbit's 4 #119 findings (query token encoding, authority delimiters, port 0, unknown selected profile) and restacked all 5 branches.
- BLOCK B1 (DeepSeek): fallback fired only on tunnel *absence*, not failure. Added `RouteError::{Transport,Tool}` in `router.rs` + `route_transport`, `resolve_transport_excluding` in `transport.rs`, and server fallback on transport failure. Repro (`/tmp/opencode/bb-review/repro_fallback.py`) now returns LAN tree `rev:42` after a silent-tunnel timeout instead of OFFLINE.
- BLOCK B2 (all three): BB-R4/R5 first-step `expect` checked a harness-injected `rpc` key. Removed injection; scenarios now assert `rev: > 0` from the real tree payload.
- BLOCK B3 (DeepSeek/Hy4): BB-R3 overclaimed exponential backoff. Scenario/TestDef/blackbox-plan reworded to "reconnect after drop", with backoff scheduling declared companion L3 scope.
- Also: `TEXURE_PROFILE` added to blackbox env sanitization; `route()` restricted to cfg(test). VERIFY after fixes: texure-lint 0 · workspace 148 tests · clippy clean · fmt clean · mcp-tunnel 13/13 · B1 repro green.

## 2026-11-17 (session) — Wayfinder #64/#63: blackbox end-to-end tunnel test (BB-R1..R13 -> GREEN)
- Branch `wf/64-63-blackbox-tunnel-e2e` (stacked on `wf/64-62-transport-fallback`). Scope: flip all 13 authored-red Group R scenarios green via a real-server + in-process fake-companion blackbox target.
- RED baseline captured first: `cargo run -p texure-blackbox -- --target mcp` showed BB-R1..R13 all XFAIL with `Unsupported action in MCP test: <action>` (13 xfail, 6 pass) — saved to `tmp/evidence/63/red-run.txt`.
- New `crates/texure-blackbox/src/fake_companion.rs`: real WS client of the server's reverse tunnel (`ws://127.0.0.1:<port>/tunnel`, `Authorization: Bearer <token>` upgrade header with 401 → `AUTH_FAILED`, first TEXT frame `{"action":"register","token":...}`), fixture-driven canned gateway replies (echoes `seq`, `device_tree` from `ui_tree.json`, stale-revision `NODE_OUTDATED` with `relayed:true`), `tunnel.ping`/`tunnel.pong` heartbeat echo, real drop/reconnect, and `shizuku_cmd` outcomes from `shizuku_command_schema.json`. `CompanionError` keeps failures explicit; 6 unit tests.
- `runner.rs`: new `mcp-tunnel` target. Spawns the REAL `texure-server` binary over stdio with `--config <tmp> --enable-tunnel --tunnel-port <ephemeral>`, starts FakeBridge for the `lan_direct` fallback, then drives all 7 Group R actions over the real MCP stdio path: `tunnel_connect`/`tunnel_drop`/`tunnel_heartbeat` drive real WS lifecycle; `gateway_proxy` calls real MCP tools routed via `TunnelRouter` (enriches observable `verify_at_execute` from the recorded envelope); `shizuku_cmd` is fixture-driven; `pairing_qr` runs the real `texure-server pair` subprocess and parses its `wss://…?token=` output; `transport_resolve` proves fallback by calling `device_tree` with the tunnel disconnected. No server internals imported; no fabricated passes.
- `main.rs`: all 13 BB-R `TestDef.target` retargeted `mcp` -> `mcp-tunnel`; `--target` doc updated. A/B/C and `mcp-fake`/`mcp-real` stay green (mcp-fake 17 pass 0 xfail; mcp-real 17 pass / 5 documented skips).
- Docs: `docs/repo-map.md` command row + `docs/blackbox-plan.md` Group R target column updated to `mcp-tunnel`.
- Self-review: independent reviewer APPROVE after auditing all 7 Group R actions for fabricated passes. 12 of 13 are genuine real-path exercises; BB-R7..R10 (`shizuku_cmd`) use the authored fixture `mock_result` oracle from `shizuku_command_schema.json` — there is no MCP tool or tunnel RPC for wake/keyguard/am-start/pm-grant/input-tap under the frozen 6-tool set (G7), so L2 can only assert the fixture outcome; real execution is L3/device scope. Documented as a coverage limit, not a tunnel pass. OCR re-review was attempted twice but each run was lost to an opencode server restart; no OCR findings were received. Duplicate-agent incident from the first lost dispatch was reconciled: single `fake_companion` module/harness, no residue.
- VERIFY: L0 pass (`texure-lint` 0 findings; clippy `--workspace --all-targets -D warnings` clean; `fmt --check` clean) · L1 pass (`cargo test --workspace`: 43 blackbox + 61 server + 26 contract + 9 companion + 4 lint, 0 failures) · L2 pass(13 BB-R flipped): `cargo run -p texure-blackbox -- --target mcp-tunnel` = 13 passed / 0 failed / 0 xfail · L3 n/a. FLIPS: BB-R1..BB-R13 -> GREEN. NOT COMMITTED — orchestrator handles commit/PR/issue.

## 2026-11-17 (session) — Wayfinder #64/#62: configurable transport order fallback resolution
- Branch `wf/64-62-transport-fallback` (stacked on `wf/64-61-tunnel-tool-routing`). Scope: make `call_tool` transport selection config-driven instead of hardcoded "tunnel if connected else LAN".
- Contract schema (`crates/texure-contract/src/config.rs`): added optional `[network]` table (`NetworkConfig`) with `transport_order` defaulting via `#[serde(default = "default_transport_order")]` to `["reverse_ws","lan_direct","tailscale"]` and optional `tailscale_ip`. Existing configs with no `[network]` parse unchanged; struct literals in contract/pair tests use `..Default::default()`.
- New `crates/texure-server/src/transport.rs`: deep seam `resolve_with(order, probe)` evaluates transports in strict declared order with transparent fallover; `NO_CONFIG` failures win over `OFFLINE` so "no config + no tunnel" keeps the behavior-spec result; all-fail is frozen `OFFLINE` with one-line `CODE: message; hint` (G8). `parse_transport_order` rejects unknown/empty names as `NO_CONFIG` config errors. `tailscale_config` builds a dial config only when a tailnet destination is configured (reuses BridgeClient/reqwest + resolved token/timeout). `resolve_transport` probes reverse_ws via `TunnelHandle::is_connected`, lan_direct via `load_resolved_config_async`, tailscale via direct config resolution (skips mDNS auto-URL discovery).
- `server.rs`: `call_tool` now calls `resolve_transport` and routes `ReverseWs` via `TunnelRouter` or `Direct` via a URL-keyed cached `(BridgeClient, WsSupervisor)`. Verbose/arg parsing/error conversion unchanged; 6-tool freeze (G7), frozen 16 codes, no wire/schema_version change. No DECISIONS.md entry needed — Map 9 decision 4 already fixed this exact order.
- TDD: RED captured in `tmp/evidence/62/red-run.txt` (49 compile failures against the missing API); GREEN: 13 new transport tests + 3 config-parsing tests pass, 61 server + 26 contract tests total green.
- Docs: `docs/lld-server.md` §2.2 config example + `[network]` semantics; `docs/bridge-contract.md` Group R item 4; new `docs/repo-map.md` row. LOG entry added.
- Self-review (2 iterations): reviewer BLOCK — changing the resolved target overwrote the cached `(BridgeClient, WsSupervisor)` but left the old `WsSupervisor` reconnect task running forever; fixed by storing its `JoinHandle` in the cache (`CachedBridge` alias) and aborting on replacement + `type_complexity` alias. OCR tailscale-embedded-port validation gap fixed (`100.64.0.5:9999` now rejected); 5 OCR findings discarded as false positives/intentional design (runtime name validation, cache key, empty-order error, NO_CONFIG priority). Re-verified green.
- VERIFY: L0 pass (`cargo run -p texure-lint`: 0 findings; clippy `--workspace --all-targets -D warnings`: 0; fmt clean) · L1 pass (`cargo test --workspace`: 61 server + 37 blackbox + 26 contract + 9 companion + 4 lint, 0 failures) · L2 n/a (BB-R12 stays authored-red until #63) · L3 n/a. FLIPS: none. NOT COMMITTED — orchestrator handles commit/PR/issue. BB-R12 remains xfail until #63.

## 2026-11-17 (session) — Wayfinder #61: transparent tool routing over the reverse tunnel
- Branch wf/64-61-tunnel-tool-routing (stacked on wf/64-60-reverse-ws-listener). Scope: route all 6 frozen MCP tools through a connected companion tunnel; keep the direct LAN bridge path untouched when no tunnel is connected.
- New `crates/texure-server/src/router.rs` (`TunnelRouter`): builds the gateway envelope `{v:1, seq:<unique>, payload:{type:"rpc", rpc, args, verify_at_execute}}`, sends via `TunnelHandle::request`, correlates the `{v,seq,payload}` / JSON-RPC `{jsonrpc,id,result|error}` reply, and translates into the direct path's exact `CallToolResult` (success_text/image + verbose meta; one-line `CODE: message; frozen hint`; full relayed error JSON). Mutation args reuse the same contract request structs as the direct path; verify_at_execute = true for tap/type/keypress. `wait_for` polls device_tree over the tunnel (500ms, 30s cap) since the tunnel has no WS tree_changed channel. Relayed NODE_OUTDATED keeps code/message/hint (G8); tunnel NotConnected/Disconnected/Timeout map to frozen OFFLINE, never panic.
- `McpServer` gains `tunnel: Option<TunnelHandle>` + `with_tunnel()`; `call_tool` selects tunnel when `handle.is_connected()`, else the unchanged direct path (BB-R12 fallback). `main.rs` passes the handle from the opt-in TunnelServer. Default stdio (no flags) is unchanged.
- Lifecycle: opt-in `--lifecycle-probe` (McpServer::with_lifecycle_probe -> TunnelRouter::with_lifecycle_probe) sends a best-effort `device_state` probe and triggers `shizuku_start_app` when state is STOPPED. Off by default: the exact state-reporting RPC is not in the frozen gateway contract; DECISIONS.md entry + follow-up #121 track that dependency.
- TDD: RED stub captured 7 failing router tests in `tmp/evidence/61/red-run.txt`; GREEN implementation flips all 11 router tests; added MCP-boundary tests for tunnel selection and disconnected fallback.
- Self-review: reviewer BLOCK — lifecycle probe was unreachable in production; wired as opt-in `--lifecycle-probe` flag (McpServer -> TunnelRouter), DECISIONS.md entry + follow-up issue #121. Fixed `wait_for` deadline parity (final fresh tree fetch), strict base64/screenshot and serialization error handling (OCR HIGH). Re-verified green.
- VERIFY: L0 pass (`texure-lint` 0 findings, clippy --workspace --all-targets -D warnings 0, fmt clean) · L1 pass (`cargo test --workspace`: 44 server + 37 blackbox + 26 contract + 9 companion + 4 lint) · L2 n/a (BB-R4/R5/R6 stay authored-red until #63) · L3 n/a. FLIPS: none. NOT COMMITTED (orchestrator review).

## 2026-11-17 (session) — Wayfinder map 13 (#64) ticket #60: Reverse WebSocket tunnel listener
- Ticket #60 (`wf/64-60-reverse-ws-listener`, stacked on `wf/64-59-pair-qr`): new module `crates/texure-server/src/tunnel_server.rs` implements the inbound `/tunnel` listener: raw `TcpListener` + `tokio_tungstenite::accept_hdr_async` (no axum/hyper), path + bearer-header validation at upgrade (401 `AUTH_FAILED`), first-frame `{"action":"register","token":...}` validation (post-upgrade `AUTH_FAILED` frame + close), native Ping/Pong + text `tunnel.ping`/`tunnel.pong` heartbeat (30s default timeout = 2x 15s interval, floor 10s), single-flight lock with `BUSY` + owner identity and release on teardown, and a cloneable `TunnelHandle` seam (`is_connected`, `owner`, `request(&str)` with `seq`/`id` correlation) for #61/#63.
- G11 note: production endpoint is `wss://` (TLS terminated externally); this module runs plain `ws://` on localhost for in-process tests/dev — no TLS deps added. 6-tool frozen list unchanged (G7); no new error codes.
- Startup wiring: opt-in `--enable-tunnel` / `--tunnel-port <u16>` (default `pair::DEFAULT_TUNNEL_PORT` 8766), binds `0.0.0.0`, token from `pair::resolve_token_from_config`. No flags ⇒ unchanged plain stdio MCP server (mcp-real blackbox preserved).
- TDD: 7 focused unit tests red on stubs (captured `tmp/evidence/60/red-run.txt`) → green (valid register, AUTH_FAILED first-frame, AUTH_FAILED header 401, heartbeat timeout, teardown release, BUSY + owner, enum `request` correlation).
- Self-review (2 iterations): reviewer found one BLOCK — `release()` mutated `connected` and `pending` unconditionally so a stale teardown could evict a newer owner; fixed by holding the `active` mutex across all three mutations + regression test `stale_release_does_not_disconnect_newer_owner_or_clear_pending`. OCR iteration-1 HIGH `std::sync::Mutex` deadlock claim disproved (no guard held across await), Relaxed `next_id`/best-effort error-frame-send findings parked; OCR iteration-2 run timed out (600s) after reviewer APPROVE. Re-verified green: L0 0 findings · L1 109 tests · clippy clean · fmt clean.
- NOT committed — orchestrator handles commit/PR/issue. BB-R1/R2/R3/R13 remain xfail until #63.

## 2026-11-17 (session) — Wayfinder map 13 (#64) ticket #59: `texure-server pair` ASCII QR + pairing secret
- Ticket #59 (`wf/64-59-pair-qr`): added `pair` subcommand to `texure-server` (crate `crates/texure-server`) rendering an ASCII QR (`#`/space, quiet zone) plus the raw `wss://<ip>:<port>/tunnel?token=<token>` connection string.
- New module `src/pair.rs`: `DEFAULT_TUNNEL_PORT=8766`, `DEFAULT_TUNNEL_PATH="/tunnel"` pub consts for #60 reuse; `--tunnel-url` normalization (full `wss://host[:port]/path` or bare domain, scheme forced to wss, default port 443); token reuse from config via `resolve_token_from_config` with an OS-CSPRNG (`getrandom`) 32-byte base64url fallback that errors rather than downgrading; JSON payload matches `contract/fixtures/pairing_qr_payload.json`.
- CLI keeps global `--config`/`--profile` (clap `global = true`) and defaults to the stdio MCP server with no subcommand; 6-tool frozen list unchanged (G7). Added `qrcode = "0.14"` dependency.
- RED: 10 focused unit tests failed on stubs (captured to `tmp/evidence/59/red-run.txt`). GREEN: 24/24 `texure-server` tests pass. Stdio smoke test with `--config`/`--profile` returned initialize + 6 tools. CLI smoke test rendered a readable QR.
- Self-review (2 iterations): reviewer APPROVE; OCR HIGH CSPRNG finding fixed (getrandom 0.3 + Result propagation), MEDIUM host-whitespace + localhost-warn findings fixed; 2 OCR findings discarded as invalid (read_config is not Result, profile lookup is not recursive); IDN/percent-encoding and CSPRNG-mock findings parked as non-blocking. Re-verified green: L0 0 findings · L1 101 tests · clippy clean · fmt clean.
- CodeRabbit review (PR #119) applied: percent-encode configured tokens in the URL query (JSON QR keeps raw token); stop authority parsing at `/`, `?`, `#`; reject port 0; return an error for an explicitly selected but unknown profile (signature now `Result<Option<String>, String>`). 4 new tests; `cargo test -p texure-server` 29 passed.
- NOT committed — orchestrator handles commit/PR/issue. BB-R11 remains xfail until #63.

## 2026-11-17 (session) — Wayfinder map 12 (#58): Android Shizuku native integration & lifecycle daemon
- Full-cycle execution of Map #58 in stacked-PR mode (5 AFK tickets #53–#57).
- Ticket #53 (`wf/58-53-android-gradle-shizuku`): Android Gradle configuration in `companion/src-tauri/gen/android` with Gradle 8.8, AGP 8.4.0, compileSdk 35, minSdk 26, targetSdk 35, Shizuku dependencies (`dev.rikka.shizuku:api:13.1.5`, `dev.rikka.shizuku:provider:13.1.5`). ShizukuProvider and permissions registered in AndroidManifest.xml. Automated tests: `ShizukuSetupTest.kt`. PR #114 filed into `main`. Linked via `link_pull_request`. All checks green. Issue #53 closed.
- Ticket #54 (`wf/58-54-shizuku-permission`): Shizuku permission check & request native Tauri plugin commands in `ShizukuPlugin.kt`. Implemented `shizuku_is_installed()`, `shizuku_is_running()`, `shizuku_has_permission()`, `shizuku_request_permission()`, and event emission `shizuku://binder-state` with binder listeners. Addressed OCR self-review findings (optimized listener checking). PR #115 filed into `wf/58-53-android-gradle-shizuku`. Linked via `link_pull_request`. All checks green. Issue #54 closed.
- Ticket #55 (`wf/58-55-lifecycle-supervisor`): Lifecycle supervisor commands (`shizuku_wake_screen`, `shizuku_dismiss_keyguard`, `shizuku_start_app`, `shizuku_stop_app`) wrapping IPC shell executions via Shizuku binder process runner reflection. Timeout and stream capture. Addressed OCR self-review findings (reflection try/catch containment, stream thread interruption, runner exception trapping). PR #116 filed into `wf/58-54-shizuku-permission`. Linked via `link_pull_request`. All checks green. Issue #55 closed.
- Ticket #56 (`wf/58-56-elevated-fallback`): Elevated system fallback commands (`shizuku_grant_permission`, `shizuku_system_tap`, `shizuku_press_key`) with strict input validation regexes (`PACKAGE_REGEX`, `PERMISSION_REGEX`, `KEYCODE_REGEX`) against command injection. Addressed OCR self-review findings (namespace checking, extracted safe execution helper `executeSafely`, bounds documentation). PR #117 filed into `wf/58-55-lifecycle-supervisor`. Linked via `link_pull_request`. All checks green. Issue #56 closed.
- Ticket #57 (`wf/58-57-foreground-service-daemon`): Android Foreground Service `connectedDevice` daemon (`CompanionDaemonService.kt`) with ongoing notification ("Texure Relay: Connected to PC"), action buttons ("Stop Relay", "Restart App"), and partial wake lock management with 1-hour safety timeout. Addressed OCR self-review findings (AtomicReference/AtomicBoolean state, START_NOT_STICKY policy, default wake lock safety timeout). PR #118 filed into `wf/58-56-elevated-fallback`. Linked via `link_pull_request`. All checks green. Issue #57 closed.
- Council Review (5 Members: Gemini 3.8 Flash, GLM 5.3 Flash, DeepSeek V4.1 Flash, Hy4, Claude Sonnet 4.6):
  - Round 1 Scores: Gemini 3.8 Flash (8/10), GLM 5.3 Flash (6/10), DeepSeek V4.1 Flash (5/10), Hy4 (6/10), Claude Sonnet 4.6 (7/10). Verdict: REQUESTING CHANGES across all members.
  - Concrete improvements applied:
    1. Added `<queries>` block in `AndroidManifest.xml` for `moe.shizuku.privileged.api` and `rikka.shizuku` ensuring package visibility on Android 11+ (API 30–35).
    2. Added `CHANGE_NETWORK_STATE` and `CHANGE_WIFI_STATE` to AndroidManifest.xml satisfying Android 14+ (API 34/35) `FOREGROUND_SERVICE_CONNECTED_DEVICE` runtime prerequisites.
    3. Added `ACTIVITY_REGEX` validation and package matching in `buildStartAppCommand` to eliminate command flag injection via `activityName`.
    4. Guarded `CompanionDaemonService.stop()` and `updateStatus()` against inactive execution; moved `isRunning` state mutation to post-`startForeground` success; added `ACTION_RESTART_APP` intent handler.
    5. Cleaned up process execution stream handling: closed stdin output stream immediately; closed streams on timeout; made reader threads daemon; bounded output capture buffer (256 KiB); mapped non-zero exit codes to `error = "NON_ZERO_EXIT"` with `errorHint`.
    6. Synchronized listener registrations via `AtomicBoolean` and wrapped listener removals in `runCatching`; dispatched `emitBinderState` to main looper.
    7. Hardened app security with `android:allowBackup="false"`.
    8. Created `companion/src-tauri/gen/android/app/proguard-rules.pro` keeping Shizuku reflection methods and daemon classes.
    9. Wired `ShizukuPlugin` into `MainActivity` lifecycle.
  - Round 2 Scores: Gemini 3.8 Flash (10/10), GLM 5.3 Flash (9/10), DeepSeek V4.1 Flash (8/10), Hy4 (8/10), Claude Sonnet 4.6 (8.5/10). Verdict: SATISFIED across all 5 council members.
  - Additional polish applied in commit `c14c129`: normalized fully-qualified activity names without slash prefix, aligned `errorHint` on `NON_ZERO_EXIT` result envelopes, and expanded regression test coverage.
- Verification: 47/47 Android unit tests passing across `ShizukuSetupTest` (3/3), `ShizukuPluginTest` (35/35), `CompanionDaemonServiceTest` (9/9). Workspace cargo check clean (89/89 tests passing, texure-lint 0 findings, clippy clean with 0 warnings).
- Status: Map #58 complete. Council unanimously SATISFIED (Rounds: 2). Stacked PRs #114..#118 ready for handoff to `merge-pr`.

## 2026-11-15 (session) — Wayfinder map 11 (#52): Tauri Rust backend & reverse WebSocket tunnel engine
- Full-cycle execution of Map #52 in Single-PR mode (4 AFK tickets #48–#51).
- Ticket #48: Tauri Rust workspace setup in `companion/src-tauri`, path dependency to `crates/texure-contract`, core wire type integration and test. Issue #48 closed.
- Ticket #49: Outbound reverse WebSocket client with exponential reconnect backoff (1s -> 3s -> 10s), 15s heartbeat ping/pong, and initial registration handshake (`{"action":"register","token":"..."}`). Mock server unit tests passing. Issue #49 closed.
- Ticket #50: Local loopback HTTP proxy client (`ProxyRouter`) forwarding reverse tunnel tool execution requests (`device_tree`, `tap`, `type`, `screenshot`) to target bridge on `127.0.0.1:8765`, with NODE_OUTDATED error envelope relay and clean OFFLINE status detection when target bridge is unreachable. Integration tests against mock HTTP server passing. Issue #50 closed.
- Ticket #51: Tauri IPC commands and state management (`start_tunnel`, `stop_tunnel`, `get_tunnel_status`, `get_target_status`) and streaming event dispatch to frontend. Unit tests verifying complete command lifecycle passing. Issue #51 closed.
- Verification: L0 texure-lint 0 findings, cargo clippy clean (0 warnings) · L1 9/9 Rust companion unit tests passing, 37/37 companion Vitest frontend tests passing · Workspace cargo check clean (88/88 workspace tests passing).
- Council Review Loop: Convened 3-member Council (Gemini 3.8 Flash, GLM 5.3 Flash, DeepSeek V4.1 Flash) across 3 review rounds:
  - Round 1 Scores: Gemini 3.8 Flash (8/10), GLM 5.3 Flash (7/10), DeepSeek V4.1 Flash (6/10).
  - Concrete improvements applied in commit `b90b26d`: Enabled `rustls-tls-native-roots` on `tokio-tungstenite` for `wss://` TLS support; decoupled WebSocket read/heartbeat from inbound RPC handling via outbound mpsc channel and spawned worker tasks (preventing false pong timeouts and HOL blocking); added `Authorization: Bearer <token>` to initial HTTP dial request; awaited join handle on tunnel stop to eliminate restart races; tracked real-time ping/pong RTT latency; added unknown RPC path traversal sanitization and structured error responses.
  - Round 2 Scores: Gemini 3.8 Flash (10/10), GLM 5.3 Flash (8.5/10), DeepSeek V4.1 Flash (8/10).
  - Concrete improvements applied in commit `a080225`: Enforced 5-second bounded grace period with abort fallback in `TunnelHandle::stop().await`; handled dropped handle/channel termination across all tokio::select loops; exposed live target bridge URL and authenticated target status probing; isolated client-initiated RTT telemetry.
  - Round 3 Scores: Gemini 3.8 Flash (10/10 - SATISFIED), GLM 5.3 Flash (9.5/10 - SATISFIED), DeepSeek V4.1 Flash (no score - rate limited; all Round 2 blocking requests verified fixed). Verdict: SATISFIED across council.
- Status: Map #52 tickets completed. All child issues #48-#51 resolved. PR #113 CI checks green (5/5). Ready for handoff to `merge-pr`.


## 2026-11-15 (session) — Wayfinder map 10 (#47): Companion Web UI & browser-testable shell
- Full-cycle execution of Map #47 in stacked-PR mode (5 AFK tickets #42–#46).
- Ticket #42 (`wf/47-42-companion-scaffold`): Tauri v2 + React/Vite app scaffolding, Tailwind CSS, TypeScript strict setup, CompanionContext with browser mock backend provider. PR #107 filed into `main`. Linked via `link_pull_request`. All checks green. Issue #42 closed.
- Ticket #43 (`wf/47-43-qr-scanner`): QR pairing code parser (BB-R11 golden pairing spec), QrScanner component with camera feed, BarcodeDetector API, ZXing fallback, and image drag-and-drop. PR #108 filed into `wf/47-42-companion-scaffold`. Linked via `link_pull_request`. All checks green. Issue #43 closed.
- Ticket #44 (`wf/47-44-status-dashboard`): Real-time StatusDashboard component with Shizuku status chips (AUTHORIZED/UNAUTHORIZED/SERVICE_DEAD), WebSocket tunnel states, target app status, and hardware vitals (battery gauge with thresholds, screen on/off, network type). Addressed OCR review findings (safe sanitization, unmount timeout cleanup, extracted subcomponents). PR #109 filed into `wf/47-43-qr-scanner`. Linked via `link_pull_request`. All checks green. Issue #44 closed.
- Ticket #45 (`wf/47-45-control-panel`): Manual device ControlPanel component for developer overrides: Screen Wake (`input keyevent 26/82`), Keyguard Dismiss (`wm dismiss-keyguard`), Cold Start Target App (`am start`), Force Stop Target App (`am force-stop`), and Reconnect Tunnel. Loading indicators on executing actions and accessible toast notification overlay. Addressed OCR review findings (toast sanitization, package name validation, memoized callbacks). PR #110 filed into `wf/47-44-status-dashboard`. Linked via `link_pull_request`. All checks green. Issue #45 closed.
- Ticket #46 (`wf/47-46-log-viewer`): Real-time streaming LogViewer component with monospace terminal view capped to 500 lines, auto-scroll toggle with scroll-detection lock, severity filter (ALL/INFO/WARN/ERROR/DEBUG), search query filter across messages and subsystem sources, Copy All / copy line, Export to formatted log file, and Clear buffer. Addressed OCR review findings (exhaustive level badge handling, clipboard error feedback, container scroll guard). PR #111 filed into `wf/47-45-control-panel`. Linked via `link_pull_request`. All checks green. Issue #46 closed.
- Council Review Loop: Convened 3-member Council (Gemini 3.8 Flash, GLM 5.3 Flash, DeepSeek V4.1 Flash) across 2 full review rounds:
  - Round 1 Scores: Gemini 3.8 Flash (8/10), GLM 5.3 Flash (7/10), DeepSeek V4.1 Flash (5/10) — Verdict: REQUESTING CHANGES.
  - Concrete improvements applied in commit `fdb634f`: QR parser URL token TypeError fix & IPv6 support, Tauri bridge listener disposal race fix & native error surfacing, QrScanner camera mount / rAF lifecycle decoupling, pair failure error state handling, Android package regex validation in ControlPanel, LogViewer scoped scroll with programmatic guard, CompanionContext memoization, lazy mock ticker, top-level ErrorBoundary, and StatusDashboard URL copy affordance.
  - Round 2 Scores: Gemini 3.8 Flash (10/10 - SATISFIED), GLM 5.3 Flash (8/10 - SATISFIED), DeepSeek V4.1 Flash (8/10 - SATISFIED) — Verdict: SATISFIED across all 3 delegates.
- Verification & Babysitting: 37/37 companion unit tests passing across 7 test suites (`App.test.tsx`, `mock-backend.test.ts`, `qr-scanner.test.tsx`, `status-dashboard.test.tsx`, `control-panel.test.tsx`, `log-viewer.test.tsx`, `error-boundary.test.tsx`). Clean TypeScript checks (`tsc --noEmit`) and Vite production bundle. All CI checks across PRs #107, #108, #109, #110, #111 are passing green.
- Status: Map #47 complete. Stacked PRs #107..#111 ready for handoff to `merge-pr`.

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

## 2026-11-13 - map 28 restack complete (PR #104)
- Rebased wf/28-27-trace-viewer onto wf/28-26-history-store (5a920ab, CodeRabbit round-2 fixes). Test-region conflict in history.rs resolved by keeping both PR suites (HEAD target-filter/format/migration tests + #104 stats tests, #104 literals adapted with target: None). One stray brace + duplicate #[test] cleaned after splice.
- Restack adaptations: StepRecord.target from results[i].target (zip results/step_traces), get_scenario_steps calls pass s.target.as_deref(), --trace/--trend default DB path now args.root-relative (deferred #92 fix applied), trace.rs fixtures gain target field.
- VERIFY: L1 pass (25 blackbox + 26 server + 4 lint + 13 contract) · clippy 0 · fmt clean · L2 pass (mcp-fake 14/14) · artifacts regenerated tmp/evidence/27/{trace.html,trend.html}, trace shows mcp-fake target labels, zero external URLs.
- Pushed 9d518ff --force-with-lease. PR #104 CI 2/2 green; CodeRabbit auto-review skipped (non-default base) -> manual @coderabbitai review triggered (comment 6843369206), watching for round-1 findings. Stale trace-db-path.html artifact removed.
- PR #92: CodeRabbit link-artifact comment only, CI green on 5a920ab, no action needed.

## 2026-11-13 - map 7 CodeRabbit round-1 findings on #104 (bd906e5)
- Applied all 9 findings: flaky requires actual failure; stats+trace keyed/grouped by (scenario_id, target); error_message shown without code; skipped-only verdict; --trace accepts any existing DB file; MCP/HTTP validation returns record failing action first; HTTP tap/type record resolved params; --trend writes per-run trace-<run_id>.html artifacts and links each row to its own trace (was shared trace.html?run= which never selected another run).
- Tests: 28 blackbox (3 new: flaky-requires-failure, stats group-by-target, skipped-only verdict), 26 server, 4 lint, 13 contract all green; clippy/fmt clean; L2 mcp-fake 14/14; artifacts regenerated, per-run links verified, 0 external URLs.

## 2026-11-13 (session) — Wayfinder map 9 (#41) shipped via wayfinder-ship
- Status: Done (tickets #37-#40). Branch wf/41-blackbox-map9 off origin/main, single-PR mode (4 AFK < 5).
- Scope: write-first RED blackbox suite BB-R1..R13 (reverse tunnel handshake/auth/reconnect, gateway proxy + error propagation, Shizuku lifecycle, pairing QR + transport fallback), Group R spec rows in behavior-spec.md §3R + blackbox-plan.md, DSL actions tunnel_connect/tunnel_drop/tunnel_heartbeat/gateway_proxy/shizuku_cmd/pairing_qr/transport_resolve (runner + lint), docs-gate R-group support (regex [A-C]→[A-R] + expected-ID list), 5 contract fixtures.
- TDD: the red suite IS the deliverable; BB-R1..R13 verified RED via --target mcp ("Unsupported action in MCP test: <action>") — clean fail-until-implemented per docs/agent-test-loop.md rule 2.
- VERIFY: L0 pass (texure-lint 0 findings) · L1 pass (28 blackbox + 4 lint unit tests green) · L2 pass-red (13/13 R scenarios authored red; existing A/B/C suites untouched) · L3 n/a (no device surface; maps 10-13 implement the real targets) · FLIPS: none (write-first; flip happens when maps 10-13 land).
- Review: independent reviewer subagent + OCR both DOWN (LLM endpoint conn refused / HTTP-to-HTTPS 400). Degraded mode: mechanical self-review checklist in-context — action lists identical, 13/13 registry/plan/spec/file consistency, no false-green paths (every R scenario starts on an unknown action), no duplicate IDs, no real credentials. GREEN, zero findings, no parked items.

## 2026-11-13 (session) — map 9 PR #105 babysit, CodeRabbit round-1
- CI 7/7 green on 95da3ef; CodeRabbit review triggered manually (repo <10 stars, no auto-review).
- Round-1: 3 Minor findings. Applied 2 concrete fixes (commit fix(#41)): stale-rev refusal path in gateway_proxy_tap (BB-R5); evaluate_assertion state-predicate no longer silently passes for unhandled targets (generic targets compare last-response `state` field, fail loudly otherwise) + 3 new assertion tests (31 total). Parked 1 verbatim on ticket #40: owner-identity field for tunnel BUSY refusal — needs bridge-contract field definition, map 10 decision.
- VERIFY after fixes: L0 lint 0 · L1 31 blackbox + 4 lint green · clippy 0 · fmt clean · L2 BB-R suite still authored red.

## 2026-11-14 - map 9 PR #105 merged
- Rebase-merged into main (1ba2361 tip); all 8 commits landed. CI 7/7 green on d97f720 pre-merge.
- Branch cleanup: remote deleted by gh, local wf/41-blackbox-map9 deleted. T3 session worktree kept (session cwd, platform-managed).
- PR watcher released (handback). Maps 10-13 unblocked.

## 2026-11-14 (session) — Council Reviews Round 1 & Round 2 (Map 9 Issue #41)
- Council review convened with 3 delegates: Gemini 3.8 Flash, GLM 5.3 Flash, DeepSeek V4.1 Flash.
- Round 1 scores: Gemini 7.5/10, GLM 7/10, DeepSeek 4/10.
- Applied Round 1 fixes (commit 08d3800):
  - Guardrail scoped exceptions G6/G10 documented in docs/guardrails.md and DECISIONS.md.
  - docs/behavior-spec.md §4 and §5 updated for Group R mappings.
  - Protocol fixtures updated with reply envelopes, verify_at_execute: true, and flat arg schemas.
  - Blackbox runner: write_first flag and xfail tracking so authored-red suite does not break L2 gate.
  - docs_gate regex tightened to BB-(?:[A-C]|R)[0-9]+ with bidirectional validation.
- Round 2 scores: Gemini 10/10, GLM 8.5/10, DeepSeek 6/10.
- Applied Round 2 fixes:
  - Guardrail G11 architectural boundary and threat model documented in docs/guardrails.md and DECISIONS.md item 4.
  - Runner xfail conversion strictly restricted to feature-absent reasons so scenario I/O and syntax errors fail loudly.
  - Extracted unified record_step_context function covering gateway_proxy device_tree and tunnel_heartbeat.
  - Generic verify_expect checks nested reply objects and token_present; evaluate_assertion supports reply.state.
  - Reconciled fixtures: pairing_qr_payload.json, reverse_ws_envelope.json, transport_order.json, and shizuku_command_schema.json.
  - docs_gate reports accurate line numbers for unknown blackbox IDs.
  - Added unit tests: test_all_scenario_files_exist, test_write_first_xfail_predicate, and runner context/assertion tests.
- VERIFY: L0 pass (texure-lint 0) · L1 pass (81 unit tests passing) · clippy clean · fmt clean · L2 pass (mcp-fake 14 passed / 13 xfail / 0 failed, exit 0).

## 2026-11-14 (session) — Council Review Round 3 & Target Parity Fixes (Map 9 Issue #41)
- Council Round 3 convened: Gemini 3.8 Flash scored 8.5/10. GLM 5.3 Flash and DeepSeek V4.1 Flash hit OpenCode free usage rate limits (`Free usage limit reached`).
- Applied Round 3 fixes (commit b96c7e1):
  - Extracted `is_feature_absent(reason: Option<&str>)` in `crates/texure-blackbox/src/main.rs` matching `"Unsupported action in"` prefix (handling both `mcp-fake` and `mcp-real` runner diagnostics), `"FEATURE_ABSENT"`, and `"not yet implemented"`.
  - Added test case for `mcp-real` runner prefix in `test_write_first_xfail_predicate`.
  - Explicitly aliased `state:pairing` for `pairing_qr` and `state:transport` for `transport_resolve` in `record_step_context` in `crates/texure-blackbox/src/runner.rs`.
- VERIFY: L0 pass (`cargo run -p texure-lint` 0 findings) · L1 pass (`cargo test --workspace` 80/80 passed: 37 blackbox, 26 contract, 4 lint, 13 server) · clippy clean (`cargo clippy --workspace --all-targets` 0 warnings) · fmt clean (`cargo fmt --check`) · L2 pass (`mcp-fake` 14 pass / 13 xfail / 0 fail; `mcp-real` 15 pass / 13 xfail / 4 skip / 0 fail).

## 2026-11-14 (session) — Map 8 / Map 9 Integration & Council Fixes
- Restored Map 9 blackbox runner capabilities alongside Map 8 additions (`POST /tag`, region screenshot, hash targeting).
- Applied Council review rounds 1-3 fixes: xfail write-first tracking, threat model documentation in DECISIONS.md/docs/guardrails.md, target parity, and test suite verification.
- VERIFY: L0 pass (`cargo run -p texure-lint` 0 findings) · L1 pass (`cargo test --workspace` 80/80 passed) · clippy clean (0 warnings) · fmt clean · L2 pass (`mcp-fake` 17 pass / 13 xfail / 0 fail; `mcp-real` 17 pass / 13 xfail / 5 skip / 0 fail).
