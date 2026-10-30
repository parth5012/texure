# Repo map

Updated as the codebase updates. AGENTS.md points here instead of duplicating.

## Current layout (docs phase)

```
texure/
  AGENTS.md               # agent-facing rules
  README.md               # product one-liner + status
  ARCHITECTURE.md         # three-seam design, risks, target layout
  ROADMAP.md              # now / next / later / not-doing tiers
  DECISIONS.md            # append-only decision log
  LOG.md                  # session-by-session log
  docs/
    bridge-contract.md    # transport seam: HTTP routes + MCP tool note
    behavior-spec.md      # observable behaviors + shared state machine
    spike-injection.md    # injection research verdict + spike spec
    testing-architecture.md
    blackbox-plan.md
    linting-plan.md       # texure-lint + ast-grep rule design (issue #5)
    lld-ci.md             # CI gates, OS matrix, dist recipes, release flow
    lld-outline.md        # resolved/open LLD tracker with item status
    runbook-device.md     # per-session ladder B0..B7 (phone prep + probes)
    agent-test-loop.md    # verification protocol for implementation agents (L0..L3)
    repo-map.md           # this file
    guardrails.md
  contract/               # fixtures land here with issue #3
```

## Target layout (builds land via issues #1–#3)

```
texure/
  crates/
    texure-server/        # Rust MCP stdio server
    texure-blackbox/      # blackbox runner + clients
    texure-contract/      # fixture loader / schema checks
    texure-fakebridge/    # in-memory bridge adapter (scenario player)
  android/
    bridge/               # Bridge SDK (Kotlin AAR -> mavenLocal)
    gradle-plugin/        # 1-line adoption seam
    sample/               # gauntlet sample app (also the spike target)
  contract/
    fixtures/             # golden JSON, shared Kotlin<->Rust
  tests/blackbox/         # scenario JSONs (schema_version 1)
```

## Commands

| Task | Command | Status |
|------|---------|--------|
| Docs-gate lint (L0) | `cargo run -p texure-lint` | planned (#5 P0) |
| Lint report only | `cargo run -p texure-lint -- --report lint-report.json` | planned (#5 P0) |
| Blackbox full run | `cargo run -p texure-blackbox -- --target all --report out.json` | planned (#3) |
| Blackbox, CI only (L2) | `cargo run -p texure-blackbox -- --target mcp-fake` | planned (#3) |
| Blackbox vs device (L3) | `cargo run -p texure-blackbox -- --target http --url <phone-ip>:<port>` | planned (#1) |
| Server unit tests | `cargo test -p texure-server` | planned (#1/#2) |
| Contract fixture check | `cargo test -p texure-contract` | planned (#3) |
| Bridge unit tests | `./gradlew :android:bridge:testDebug` | planned (#1) |
| Sample app build | `./gradlew :android:sample:assembleDebug` | planned (#1) |
| Device mechanism regression | `./gradlew :android:sample:connectedDebugAndroidTest` | planned (#1) |

Keep this table honest: flip status to "live" only when the command actually runs in this repo.
