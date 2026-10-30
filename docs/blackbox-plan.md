# Blackbox test suite — implementation plan

Executable form of [bridge-contract](bridge-contract.md) + [behavior-spec](behavior-spec.md). Blackbox only: through the two interfaces (HTTP routes on the bridge, MCP stdio tools on the server). Never internal APIs, never internals-by-log-grep (evidence bundles are the only artifact exception).

## Stance

The suite is written FIRST, while no implementation exists. Phase 0 ends with a runnable runner + authored scenarios that all fail. Each implementation milestone (issues #1, #2) flips groups from red to green. This makes the suite the progress gauge, not a post-hoc chore.

## Harness shape (one runner, three targets)

```
tests/blackbox/
  Cargo.toml                # texure-blackbox (bin + test lib)
  src/main.rs               # runner: --target <mode> --scenario <name>
  src/client_http.rs        # reqwest + tokio-tungstenite (bridge target)
  src/client_mcp.rs         # mini stdio MCP client (spawns texure-server)
  scenarios/*.json          # behavior sequences, shared with FakeBridge scenarios
  fixtures/                 # golden JSON (single source: also used by contract tests)
```

Targets (one suite, run against progressively more-real things):
1. `--target mcp` — spawns the REAL texure-server binary over stdio (full blackbox, the agent's actual path).
2. `--target http --url phone:port` — real bridge on the sample app (device needed).
3. `--target mcp-fake` — server configured against FakeBridge for CI-speed runs.

FakeBridge scenarios and blackbox scenarios are THE SAME JSON files; the runner can play a scenario server-side (FakeBridge mode) or assert against a real device session.

## Behavior-row to test mapping (from behavior-spec)

Test IDs: `BB-<group><n>`. Every row from behavior-spec is covered or explicitly marked device-only.

### Group A — contract shape (FakeBridge + real bridge)

| ID | Asserts |
|---|---|
| A1 | `/ui-tree` shape: `rev`, `window`, nodes with `id,text,cls,bounds,clickable,editable`; interesting-only filter respected; depth cap honored |
| A2 | `/tap` `/type` `/keypress` `/back` return `{ok:true}` with correct rev echo |
| A3 | Error envelope: every error has `code`, `message`, and a hint; codes match the taxonomy (`NODE_OUTDATED`, `AUTH_FAILED`, `OFFLINE`, `APP_NOT_FOREGROUND`, `SCREEN_OFF`, `DEVICE_LOCKED`, `APP_BUSY`) |
| A4 | `/screenshot`: PNG magic bytes; `scale=0.5` default; sizes differ across scales |
| A5 | `/ws`: mutating action emits `tree_changed {rev}`; rev strictly increases |

### Group B — state machine behaviors

| ID | Asserts | Runnable where |
|---|---|---|
| B1 | Unconfigured server → `NO_CONFIG` + exact fix hint | mcp |
| B2 | Unreachable device → `OFFLINE` + hint; WS reconnects 1s/5s/15s | mcp-fake |
| B3 | Token mismatch → `AUTH_FAILED` | http |
| B4 | Stale rev tap → `NODE_OUTDATED` (reproduce `stale_screen` timing) | http, FAKE retry |
| B5 | Backgrounded app mutations → `APP_NOT_FOREGROUND` + launch hint (reads still allowed, hint warns) | device |
| B6 | Screen off → `SCREEN_OFF`; device locked → `DEVICE_LOCKED` | device |
| B7 | `human_win`: real digitizer touch during active session cancels injection, rev bumps, state observable, agent re-fetch advised | device |
| B8 | Single-flight: second MCP session gets `BUSY` + owner identity | mcp |
| B9 | Lock release on disconnect and on 60s idle (test-mode timer via `TEXURE_TEST_*` env scaling — a documented affordance, not an impl change) | mcp |
| B10 | `verbose:true` returns full JSON error; default returns one-liner | mcp, http |

### Group C — goal flows (the milestone instrumented)

| ID | Asserts |
|---|---|
| C1 | `login_flow`: agent-driven login/text flow completes unattended on VelaVoice over LAN (issue #2's milestone, as a test) |
| C2 | `slow_screen`: `wait_for` timeout ≤ 30s → `{ok:false,last_rev}` + re-plan hint; WS-down path falls back to HTTP polling |
| C3 | `steer_flow`: human touch mid-flow → agent receives steering signal and completes after re-fetch (human+agent cooperative flow) |

## Phases and gates

| Phase | When | Contents | Gate to pass |
|---|---|---|---|
| **P0 now** | immediately | Runner skeleton (3 target modes), client_http + client_mcp, scenario JSON format, fixtures authored for A-group, all tests authored as failing/skipped | `cargo run -p texure-blackbox -- --target all` runs, reports every ID as red/skip with reasons |
| **P1 with FakeBridge** (issue #1 side-quest) | spike 1 second half | FakeBridge with scenarios `static_screen`, `stale_screen`, `slow_screen`, `auth_reject`, `busy_second_client` | A1-A5, B1/B2/B4(fake)/B8/B9/B10 green in CI, zero device |
| **P2 real bridge** (issue #1) | spike 1 end | Device target on `android/sample`; B5/B6/B7 device-gated | A-group green on REAL bridge; B5/B6/B7 green on Xiaomi |
| **P3 VelaVoice** (issue #2) | milestone | One-line integration target; C1/C3 | C1 green = the two-week milestone, measured |

## Scenario / assertion DSL v0 (the missing contract)

Every scenario and fixture is JSON with `schema_version: 1`. Without this grammar P0 cannot be authored; it IS part of the blackbox design, not an implementation detail.

```json
{
  "schema_version": 1,
  "name": "stale_screen",
  "kind": "scenario",              // scenario | fixture
  "steps": [
    { "action": "device_tree", "args": {}, "expect": { "rev": "> 0" } },
    { "action": "tap",         "args": { "node": 3, "rev": "$tree.rev" }, "expect": { "ok": true } },
    { "action": "wait_for",    "args": { "where": "text", "value": "Home", "timeout": 30 }, "expect": { "ok": true } }
  ],
  "assertions": [
    { "pred": "exists",  "target": "node[text=Home]" },
    { "pred": "count_lt","target": "all", "value": 200 },   // token budget guard
    { "pred": "state",   "target": "bridge", "value": "IDLE" }
  ]
}
```

- `$tree.rev` style refs bind to the last `device_tree` result in the same scenario (scenarios are sequential, bridge is stateful — parallel steps are a later feature and forbidden in v1).
- Predicates (`exists`, `missing`, `count_lt`, `state`, `bounds_of`) evaluate against the last tree; predicate vocabulary is small by design.
- The same files serve three roles: FakeBridge script, blackbox scenario, eval-harness input. One grammar, all three consumers.

## Report and CI contract

- `--report out.json`: machine-readable result per test ID (`pass|fail|skip|device-skip`, evidence bundle path on fail, duration).
- Exit code: 0 iff no unskipped-or-device-skipped FAIL; nonzero otherwise. CI and the nightly cron consume ONLY this shape.
- Nightly cron maps failures straight into GitHub issues; no human log-diving required.
Runner runs each target's tests strictly sequentially (stateful bridge under test); parallelism exists only across targets.

## Ownership boundaries (no orphan seams)

- Only bridge routes + MCP tool surface here. The **Gradle plugin's release-leak guard is NOT blackbox**: it is CI build-matrix + plugin unit tests, owned by issue #2.
- Scenario/fixture schema evolution is versioned via `schema_version`; breaking change = bump + migration note in this doc.

## Extra test IDs closing the audit gaps

| ID | Asserts |
|---|---|
| A0 | MCP handshake: server declares ONLY the tools capability; tool list is EXACTLY the frozen 6; unknown tool/param yields a proper MCP error |
| A6 | Token budget: on the deep gauntlet fixture screen, `device_tree` node count stays under the budget cap (protects agent context) |
| B11 | Optional device flow (rotate token via overlay): old token → `AUTH_FAILED`, new token → OK; server config must be re-pasted (documented friction, not a bug) |

## Ground rules

1. Runner never imports server/bridge internals; it talks protocol only.
2. Scenario = JSON (`name, setup, steps[], assertions[]`, predicates over `/ui-tree` state), so eval-harness later reuses it as-is.
3. Device-only tests are tagged `device` and skipped cleanly when no phone is configured (exit 0 with skip report, not build failure).
4. No sleeps: waits are predicate polls with deadlines; test affordances (`TEXURE_TEST_*` timers) are documented env vars, not magic.
5. Every red run produces an evidence bundle (screenshot + tree dump + trace) into `tmp/evidence/`.
