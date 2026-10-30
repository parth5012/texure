# Decisions Log

Append-only. One entry per decision: what, why, alternatives, what triggered it.

---

## 2026-10-30 (behavior spec, grilling round 3)

**Tree source: own view-hierarchy traversal only.** No accessibility service. Verified VelaVoice is Expo/React Native, which renders real native views, so generic traversal covers the first target. Compose = first parked framework adapter. Weak RN ids accepted; spike 1 judges them live.

**Foreground contract: refuse with `APP_NOT_FOREGROUND` + hint.** Bridge never auto-launches its own Activity (Android 10+ background-start rules + MIUI). Agent must bring the app forward as an explicit step.

**Screen state + human steering.** Screen-safe `FLAG_KEEP_SCREEN_ON` debug overlay while a client is active; screen-off and device-lock are hard refusals (`SCREEN_OFF`/`DEVICE_LOCKED`). Human physical touch during agent session ALWAYS wins: cancels injection, bumps rev, surfaces `HUMAN_STEERING` so the agent re-plans. Synthetic injections carry a marker deviceId to make this detectable.

**Tree shaping: interesting nodes + depth cap 12.** clickable/editable/scrollable/text/bearing containers only. Full trees rejected to protect agent token budgets.

**Error surface: one-line `CODE: message` + hint, with opt-in full JSON via `verbose:true` tool param.** Server daemon never dies from tool errors.

**Token lifecycle: stable per install.** Generated on first boot, stored, survives reboots/rebuilds. Manual rotation only (overlay button). Per-install and daily rotation rejected.

**Server config: TOML at standard config dir + `TEXURE_*` env overrides.** JSON-only and env-only rejected.

**Concurrency: single-flight lock.** Second MCP client gets `BUSY` + owner identity. Lock released on disconnect or 60s idle.

Full observable behavior table + shared state machine: [docs/behavior-spec.md](docs/behavior-spec.md).

## 2026-10-30 (spike placement correction)

**Spike code lives only in texure.** VelaVoice never receives test scaffolding. Spike 1 (mechanism proof) runs in `texure/android/bridge` + `texure/android/sample`, app on-device. Spike 2 (integration proof) publishes the bridge AAR to `mavenLocal` and adds one dependency line to VelaVoice's debug build, manually simulating the plugin. Rationale: throwaway code inside a product repo is future deletion work, and the product's core bet IS the one-line integration, so it should be exercised first, not last.

## 2026-10-30 (LLD grilling, round 2)

**Staleness guard: revision counter.** `/ui-tree` returns `rev`; `/tap` and `/type` must echo the rev they acted on. Bridge refuses on mismatch with `NODE_OUTDATED`; the agent re-fetches. Alternatives cut: resolve-late (silent wrong taps), re-fetch-on-tap (still racy), semantic hash (extra identity machinery for v1).

**Sync semantics: explicit `wait_for` MCP tool backed by WebSocket push.** 6 tools total now. Transport is WS (`tree_changed {rev}` pushed from bridge); webhook rejected for v1 because the PC is a NAT'd listener. `wait_for` timeout returns `ok:false` + one fresh `/ui-tree` snapshot so the agent can re-plan. Agent-layer polling is banned by design.

**Bridge process shape: debug-only foreground service.** Survives agent-driven backgrounding mid-flow. Autostart-on-boot rejected (MIUI autostart pain, parked).

**Pairing: in-app debug overlay, QR code deferred.** Dev copies URL + token once into MCP server config. QR is future polish.

**First milestone flow: login or simple text flow on VelaVoice.** Chosen over voice round-trip on past experience; voice path has more async to chew.

---

## 2026-10-30 (device runbook, LLD round 6)

**Form: executable checklist doc.** docs/runbook-device.md; every step carries probe command + expected output; pure-script verification rejected (MIUI battery dialogs are GUI-only).

**Structure: one-time vs per-session split.** A = MIUI hardening (battery No restrictions, autostart, recents lock, dev options, stays-alive), repeat trigger = MIUI update/app reinstall/reset. B = ordered ladder B0→B7 (device up → same network probe → app install → overlay probe → server connect → injection probe → flow). Straight-to-flow rejected: every red run would burn time rebuilding the ladder.

**Network doctrine: home WiFi + hotspot fallback.** Same-subnet ping is the network gate; no reply = phone hotspot with documented cost (PC loses internet). Home-WiFi-only rejected on AP-isolation debug risk.

**ADB position re-confirmed: setup-only inside the runbook** (install + logcat during sessions), never a runtime requirement; target is removal once pairing overlay covers install-less flows (product-claim guard, G4).

**App source: both paths documented, v1 = source path A** (assembleDebug + adb install from D:\work\projects\VelaVoice); plugin/mavenLocal path joins when #2 lands.

**Evidence bundles mandatory per red run** (logcat tail + screenshot + tree dump + step.md + issue), folder contract named in the runbook.

Full runbook: [docs/runbook-device.md](docs/runbook-device.md).

---

## 2026-10-30 (custom linters, grilling round 4)

**Lint surface: docs + code.** texure's top silent-failure mode is doc/code drift, so lints cover both. Class D (docs/contract/scenarios) = Rust crate `texure-lint` (serde); Class C (code anti-patterns) = ast-grep YAML rules indexing guardrails.md G1/G4/G5/G6, except release-leak final say stays with the Gradle-plugin check.

**Executor: perfgym dogfood, time-boxed 2 weeks.** Rules are portable YAML so perfgym's linter and plain `sg scan` are interchangeable executors; adoption criteria: runs full ruleset, stable exit codes, sg-baseline parity. No hard dependency on perfgym's version. Semgrep rejected (overlaps ast-grep capability, extra toolchain), detekt customs rejected (heavier per-language authoring).

**Severity: FAIL/WARN tiers.** Guardrail + contract + scenario-schema breaks = FAIL (nonzero exit). Doc freshness = WARN. Single orchestrator entrypoint (`texure-lint`) merges both engines into `lint-report.json`, parsed nightly into issues.

**Scope freeze v1:** docs-gate + scenario DSL + hygiene + fixture checks + 3 guardrail rule families. Architecture/lint defers (A6 token budget already enforced by blackbox; no duplicate).

**Where it runs:** CI job first in matrix + local; nightly cron opens issues from `lint-report.json`.

**Rule home:** in-repo `lint/ast-grep/`; rules versioned with the codebase they guard.

Full plan: [docs/linting-plan.md](docs/linting-plan.md).

---

## 2026-10-30 (CI/CD + distribution, LLD round 5)

**Rust dist: prebuilt + cargo.** Release-tag cross-builds (windows-msvc/linux-gnu/apple-darwin) + checksums on gh release; `cargo install --locked` secondary; quickstart shows prebuilt first. Semver, breaking MCP contract change = major.

**Android dist: mavenLocal + in-repo examples/.** No separate quickstart repo v1 (user call: convenience vs overhead; parked to Later with "secret repo" fallback). JitPack/Plugin Portal/Central = not-doing until post-spike.

**Nightly device runs: T3 Code scheduled tasks** own the schedule, run on this PC, FAILs open issues via gh. GitHub Actions has no device role (LAN-only). Not cron-system, not Task Scheduler — user preference for T3-native scheduling.

**Merge gates: lint + rust-test are gates;** android-build becomes a gate when plugin code lands. ubuntu+windows per PR (windows = ship platform, must stay green); mac at release tag only. Nightly never a gate.

Full LLD for the bridge itself plus the 8 decisions behind it: [docs/lld-bridge.md](docs/lld-bridge.md).

---

## 2026-10-30 (LLD A: bridge internals, grilling round 7)

Eight decisions, full detail in [docs/lld-bridge.md](docs/lld-bridge.md):

1. **Engine: NanoHTTPD + websocket module.** Route logic lives in the bridge core; engine swappable. Ktor rejected (dep-tree weight for debug-only), hand-rolled rejected (HTTP + RFC 6455 correctness risk).
2. **Threads: serial bridge worker.** All mutation verbs FIFO on one background thread; `sendPointerSync` runs directly (non-main-thread research law), `dispatchTouchEvent` replay + tree capture hop to main via Handler + result latch with 1s timeout. One concurrency contract at behavior-spec level.
3. **Injection seam: coords-in adapter.** Tree layer resolves index→coords; adapter is window-physics only (`tap/swipe/text/key`); `sendPointerSync`→`dispatchTouchEvent` auto-fallback frozen inside; no agent-visible mode knob.
4. **Overlay: activity-scoped debug view.** Zero user grants (no `SYSTEM_ALERT_WINDOW`); service notification carries status while backgrounded; FGS started only from foreground context. MIUI settings-maze avoided by construction.
5. **FGS type `specialUse`** with honest debug-bridge description. `dataSync` rejected — Android 15's 6h cap would kill long sessions.
6. **Port fixed `8765`, fail loud.** `SO_REUSEADDR`; bind conflict stops the service with `PORT_IN_USE`; no fallback range (silent config drift worse than loud failure). Port bullet marked resolved here and removed from ticket #7's scope.
7. **Token: plain app-private SharedPreferences, `allowBackup=false`.** Attacker model: other apps + LAN peers only. `EncryptedSharedPreferences` rejected (deprecated androidx crypto, MIUI Keystore quirks, no new attacker covered). Rotation-only semantics preserved.
8. **Stop = abort.** User Stop closes sockets immediately, in-flight job cancelled at step boundary; candidate code `BRIDGE_STOPPED` handed to ticket #7's error-table freeze. Uninstall/force-stop cost zero code.

Feeds: #1 (spike implements this), #4, #7, #8, #9.

---

## 2026-10-30 (LLD E: contract freeze leftovers, grilling round 8)

Five decisions, detail in bridge-contract.md:

1. **Mid-press recovery: verify-at-execute.** Mutations re-traverse the tree at dispatch and check the identity chain at the referenced index; mismatch ⇒ `NODE_OUTDATED`. Blind-tap hole closed without new id machinery; semantic-hash ids stay ROADMAP (round 2 rationale unchanged: same recovery UX, extra machinery).
2. **`wait_for` grammar frozen: `{where: text|tag|visible|hidden, value, timeout}`.** Case-insensitive contains for text, exact for tag, `hidden` = the negative predicate. Int seconds, default 10, cap 30. No and/or composition, no subtree scope, no `enabled` (agents already see per-node state; agents chain calls).
3. **Error table frozen: 16 codes + `WAIT_TIMEOUT` non-error row.** `APP_BUSY` deleted — `BUSY` wins (contract draft said both; one authority, no aliases). Statuses: 401/409/400 + conn-reset for `BRIDGE_STOPPED`. Hints frozen in-table.
4. **Authority: the table lives only in bridge-contract.md §Error shape.** Ground contract rule: behavior rows cite codes but never re-list them; `texure-lint` regex-parses the markdown rows (uppercase-snake, orphan check, every code referenced by ≥1 behavior row/scenario). A3's taxonomy updated to match.
5. **Node shape gains `cd` (contentDescription, nullable, additive — schema_version stays 1).** RN icon-only buttons become addressable. Security annotations: NOT exposed (own-app tree only). `scale` default `0.5` frozen into /screenshot. All four contract open questions closed.

Feeds: #1 (spike implements against frozen contract), #3 (A3 taxonomy + scenario grammar), #5 (docs-gate parses the table), #8 (error-map cross-references this table).

---

## 2026-10-30 (earlier planning rounds)

**Product scope: devs test their OWN apps.** In-app bridge model. Generic any-app control is explicitly cut.
*Why:* Kills the Xiaomi/INJECT_EVENTS problem by construction; defines a shippable product (one Gradle line) instead of a fragile universal controller. *Alternative cut:* accessibility APK sidecar / HID emulation; abandoned because vendor security makes them whack-a-mole.

**First target: VelaVoice** (Gradle Android project).
*Why:* Real first client for the seam; Gradle plugin story is natural. Expo/RN config plugin is a future adapter if needed.

**Framework support: Gradle (Android) v1; Rust-built apps on adapter roadmap.**
*Why:* Gradle covers VelaVoice now. Rust adapter (Tauri etc.) is a future implementation behind the same transport seam. Two plugin ecosystems in v1 was rejected (doubles surface, muddies interface).

**Server language: Rust** for the MCP server.
*Why:* Single static binary, ~15-20MB RAM, cross-platform. Fits the 6GB-RAM desktop constraint. Alternatives (TS/Node server) rejected on footprint and distribution friction. Note: OpenCode context briefly contradicted itself on this; resolved toward Rust.

**Agent seam: MCP over stdio.**
*Why:* OpenCode is the day-one client; stdio MCP is natively consumable by OpenCode, Claude Code, Cursor, etc. Streamable-HTTP parked.

**Transport: LAN HTTP/WS with token auth for v1.** Wireless via adb pairing, Tailscale, WireGuard: parked as later adapters at the same seam.
*Why:* v1 must not build infra. Phone and PC share a network is an acceptable documented constraint; error message explains failures.

**Adoption seam: Gradle plugin, debug-builds-only enforced.**
*Why:* One line for the dev is the difference between adoption and abandonment. Release leak would be a security incident, so the plugin fails builds, not just warns.

**First milestone: "one app, one flow."**
*Why:* Forced pick after "a mix of these" got rejected in grilling. Two weeks: spike injection mechanism, hand-wired bridge, MCP server + 4 tools, agent completes one real VelaVoice flow over LAN unattended. Gradle plugin built only after the spike proves injection.

**Product stance: OSS, MIT, free.**
*Why:* Adoption and portfolio/internship positioning now; monetization can be layered later without breaking an MIT core.

**Xiaomi any-app workarounds (HID/AOA, accessibility APK, scrcpy+xdotool): explicitly cut.**
*Why:* Not needed once the bridge runs in-process. Research report methods 2-4 parked; Method 1 (in-app bridge) IS the product.

**Naming: `texure`.** First working name was `devhand`, scrapped in discussion. `texure` is a coined word and we own that story. Renaming later is still cheap at this stage.
