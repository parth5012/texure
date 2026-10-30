# Agent test loop — how implementation agents verify their work

The playbook for ANY agent executing a texure ticket (wayfinder maps #11-#13, or later). Other docs define what is BUILT to be testable ([testing-architecture.md](testing-architecture.md)); this doc defines how an agent VERIFIES, in order, without a human present.

## The four verification tiers

| Tier | Runs on | When allowed | Path |
|---|---|---|---|
| L0 static | PC only, always | Always, self-serve | `cargo clippy/check`, `gradlew lint`, texure-lint |
| L1 unit | PC only, always | Always, self-serve | `cargo test`, `gradlew test` (Robolectric) |
| L2 fake | PC, no device | Always, self-serve | blackbox `--target mcp-fake` + FakeBridge scenarios + contract fixtures |
| L3 device | Phone + PC + runbook | Only via device ladder; STOP on red | blackbox `--target http`; device harness on android/sample |

## Rules

1. **A ticket is DONE only at the highest tier its surface exposes.** Bridge injection work must pass L3; server tool mapping may be L2-complete; docs-gate lint work is L0-complete. Tier mapping per ticket comes from blackbox-plan's group table.
2. **Never skip down a tier to claim green.** Turning a red L2 into an acknowledged XFAIL via label/comment is legitimate progress (write-first suite); silently deleting a failing test is a bug, not a decision.
3. **Flipping the suite is the progress gauge.** Implementation milestones flip blackbox groups from red/skipped to green — that IS the ticket outcome, not an afterthought.
4. **L3 device work follows docs/runbook-device.md ladder B0..B7 in order.** Jump to step N only if 0..N-1 are known-green. Any red run: evidence bundle (logcat tail + screenshot + tree dump + step.md) into `tmp/evidence-<date>-<step>/`, then comment BLOCKED on the ticket with the bundle path. Do NOT improvise device debugging.
5. **Device unavailable = blocked, not failed.** If `adb devices` empty or network probe fails, the agent parks the ticket's L3 portion and finishes everything up to L2, stating exactly what L3 remains.
6. **Scenario authoring is code.** New scenarios go into `tests/blackbox/scenarios/*.json` (same files as FakeBridge), validated by texure-lint DSL checks once #5 P0 lands. No scenario file, no new behavior accepted.
7. **Dogfood loop (when bridge + server exist).** The agent's OWN session becomes the test: run texure-server locally, connect the MCP tools, and drive the flow as the agent itself. This is the product being tested by its intended user. Point of the product; treat it as the L2.5 dev-loop, available any time PC + phone allow.

## Self-check a ticket must print at the end

```
VERIFY: L0 pass · L1 pass · L2 pass(<n> scenario groups or n/a) · L3 <pass|blocked:<reason>|n/a>
EVIDENCE: <bundle path or none>
FLIPS: <test IDs flipped red->green, or none>
```

This block goes in the ticket resolution comment; it is what the map runner reviews against the blackbox ID matrix.

## Command surface (planned -> live as code lands)

| Command | Live when | Purpose |
|---|---|---|
| `cargo run -p texure-lint` | #5 P0 | docs-gate green check (L0) |
| `cargo test` | first crate | L1 |
| `cargo run -p texure-blackbox -- --target mcp-fake --scenario <name>` | #3 P0 | L2 |
| `cargo run -p texure-blackbox -- --target http --url <phone-ip>:<port>` | #1 | L3 |
| `gradlew :app:connectedDebugAndroidTest` | #1 device harness | L3 mechanism regression |

Status column in docs/repo-map.md mirrors this table's live state.
