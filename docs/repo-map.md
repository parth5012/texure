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
| Blackbox full run | `cargo run -p texure-blackbox -- --target all --report out.json` | planned (#3) |
| Blackbox, CI only | `cargo run -p texure-blackbox -- --target mcp-fake` | planned (#3) |
| Server unit tests | `cargo test -p texure-server` | planned (#1/#2) |
| Contract fixture check | `cargo test -p texure-contract` | planned (#3) |
| Bridge unit tests | `./gradlew :android:bridge:testDebug` | planned (#1) |
| Sample app build | `./gradlew :android:sample:assembleDebug` | planned (#1) |

Keep this table honest: flip status to "live" only when the command actually runs in this repo.
