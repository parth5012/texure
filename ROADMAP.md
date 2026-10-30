# Roadmap

Explicitly parked ideas live here so ARCHITECTURE.md stays about v1. Cadence: review when current milestone lands; never pull a "later" item into "now" without a DECISIONS.md entry.

## Now (current 2-week slice)

- Injection spike live-proof on real Xiaomi device (`docs/spike-injection.md`)
- Bridge SDK: 6 routes + `/ws`, hand-wired, no plugin
- Rust MCP server: 6 tools over stdio
- One agent-driven flow on VelaVoice over LAN (login/text)
- Gradle plugin (debug-only) — only after spike passes

## Next (v1.1-ish)

- **Custom linters** (see [docs/linting-plan.md](docs/linting-plan.md)): `texure-lint` docs-gate + scenario DSL crate FIRST (P0, no code needed), forbidden-API ast-grep YAML rules when code exists. perfgym dogfood, time-boxed; `sg scan` fallback. Parked as issue #5.

- **Agent eval harness** (see [docs/testing-architecture.md](docs/testing-architecture.md)): scenario runner CLI, deterministic replay + goal-based pass-rate modes, evidence bundles on failure. First pass rides issue #2.
- **Record & replay human workflows.** Bridge records its own window's real MotionEvents during a debug-overlay "record" mode, exports as scenario JSON (the shared DSL with the blackbox suite/eval harness), agents replay via tools with the rev guard. Also exports a **codemode script** form: a plain tool-call sequence agents can execute/reuse directly in their harness (OpenCode etc.), so agents get programmatic control with or without the scenario runner. Concept synergy: "human wins" steering doubles as a recording input.
- **Crash/ANR capture — ASAP tier (user-prioritized).** Debug-only `UncaughtExceptionHandler` in the bridge: on uncaught exception/ANR signal, export a bundle (crash stack, last known ui-tree, last actions, build info) and surface it to the next connected agent/dev session. Cheap; hooks bridge init in issue #1 scope; implement in the same pass as the spike. See issue #4.
- **ADB bootstrap utility — OPTIONAL only (user-confirmed, never required).** Install/launch/screen-wake/bootstrap via USB or wireless ADB as SETUP-ONLY, opt-in tooling. Runtime agent interaction NEVER requires ADB (product honesty rule: if ADB becomes required, the locked-down-phone claim dies).
- **Trace viewer.** Evidence bundles evolve into a Playwright-style HTML timeline of steps + screenshots + trees.
- **Regression tracking.** Deterministic C-group scenario runs per build, results stored (JSONL/sqlite keyed by versionName + build hash), HTML trend report; failures keep evidence bundles. A "no new concept" feature: scenario runner + a store.
- **Gradle plugin hardening.** Release-leak guardrails (build fail, not warn), both Groovy and Kotlin-DSL module shapes, pairing token generation.
- **QR pairing.** In-app debug overlay renders QR encoding URL + token; MCP server camera-side is out of scope, so QR is for phone-to-phone or local viewer use.
- **Stable node ids** (beyond rev + index): semantic hash identity so agent memory of a node survives refreshes.
- **Screenshot scale presets + partial-region capture** to cut vision tokens.
- **`set_tag` route.** Dev can tag views from agent feedback, without rebuilding.
- **mDNS discovery** so no URL pasting at all.
- **Multi-device registration** in server config (phone profile list, select by profile name).

## Later (roadmap, deliberate separates)

- **Framework adapters** (same bridge HTTP contract, different injectors/packaging):
  - Expo/React Native config plugin
  - Flutter module
  - Rust-built apps (Tauri etc.) — the "rust app plugins" idea from the original plan
- **Screen mirroring for agents.** scrcpy-source MCP frame provider (requires the ADB bootstrap utility first; scrcpy needs ADB). Vision grounding on top of the JSON tree when pixel truth matters.
- **Config matrix.** Same scenario replayed under rotation, dark mode, or a locale change; scenario runner scripts the config change before each replay.
- **Deep-link/entry matrix.** Same flow via deep link / notification / cold start.
- **Performance probes.** Choreographer frame time + cold start per flow, persisted into the regression trend UI.
- **Assertion SDK for devs.** Kotlin DSL over the bridge for non-agent tests in plain instrumented suites.
- **Multi-device runs.** Two+ phones concurrently for cross-device regressions (single-flight lock rework required).
- **Wireless beyond LAN:** adb wireless debugging pairing flow; Tailscale/WireGuard transport adapters.
- **CI mode:** USB runner suite, device persists across runs, junit-style results out of flow scripts.
- **Emulator + scrcpy support:** window/keyboard mirroring as an observation channel, not as injection.
- **Web-build testing hooks** (same flows replayed against an Expo web build for cross-engine checks).
- **Battery killer auto-detection** on MIUI/HyperOS: detect restrictions and show a guided fix page instead of failing silently.

## Not doing (revisit only with real user pain)

- Any-app control (accessibility tablet, HID emulation, scrcpy+xdotool)
- Cloud relay / hosted bridge (infra + security surface, NAT'd PC listener)
- Monetization layer (MIT stays; product stance decides when a paid tier borders ever justified)
- Raw coordinate tap mode as the primary target scheme
