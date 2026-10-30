# Testing architecture for texure

Plan for how texure tests itself, and what gets BUILT so agents can test the system better. Principle borrowed from codebase-design: **test through the interfaces, not past them.** texure has three seams, so there are three groups of tests plus one eval layer.

```
 agent seam tests        MCP tool contract, error taxonomy
        |
 transport seam tests   HTTP contract fixtures + FakeBridge (second adapter)
        |
 adoption seam tests    plugin one-line integration, release-leak guard
        |
 eval layer (agents)    scenario DSL, evidence bundles, pass-rate metrics
```

## 1. Contract fixtures (cheapest, first)

`contract/fixtures/*.json`: golden examples of every route's request/response, edge cases (empty tree, missing ids, deep nesting), and error codes. Both the Kotlin bridge and the Rust server validate against the same fixtures in CI. Zero device needed, runs in seconds, catches contract drift before it bites cross-language.

Build:
- A tiny Rust test-utility crate `texure-contract` that loads/serves fixtures.
- Kotlin side: unit tests that assert serialized node output matches fixture JSON (Robolectric, no device).

## 2. FakeBridge — the second adapter (decision maker)

One adapter means a hypothetical seam; two make it real. Build an **in-memory fake bridge** (`texure-fakebridge`, Rust, behind a cargo feature or separate crate) that satisfies the exact HTTP contract `/ui-tree`, `/tap`, `/type`, `/keypress`, `/screenshot`, `/back`, `/ws`.

It can serve **scripted scenarios**:
- `static_screen` — fixed tree, perfect for happy paths.
- `mutating_screen` — tree changes after N taps (simulates navigation).
- `stale_screen` — mutation happens BETWEEN ui-tree and tap; tests rev-guard and NODE_OUTDATED.
- `auth_reject` — token mismatch; tests error taxonomy.
- `slow_screen` — tree arrives late; tests wait_for timeout path.
- `crash_loop` — WS flap; tests reconnect logic.

Everything above the fake runs with ZERO hardware: the Rust MCP server, all tool logic, error mapping, retry/reconnect code AND agent harnesses (OpenCode etc.) can drive the same tools against fake screens at CI speed.

This directly answers "how do agents test the system better": give agents a deterministic fabric that reproduces the failure modes of the real bridge on demand.

## 3. Rust server tests

- Unit: tool schema mapping, tool name freeze (6 tools), error translation table (bridge codes -> MCP tool errors), PNG plumbing (junk/scaled PNG handling).
- Integration over FakeBridge scenarios, exercised through the MCP tool surface (not internal functions): the MCP interface is also the test surface.

## 4. Android unit tests (Robolectric, no device)

- Tree serializer: view hierarchy -> compact node JSON (ids: test tag > view id > text > class+index fallback), `/ui-tree` output structure, node bounds.
- Rev counter logic + guard behavior.
- MotionEvent builders (tap/scroll/key) — structurally validated only; physics needs real device.

## 5. Device harness (sample app graduates into a regression gauntlet)

`android/sample` stops being a throwaway and becomes THE test fixture: a screen full of deliberate test surface (button, editable, scroll list, async loader with delay, error dialog, background-transition trigger). Instrumented tests run the 5 spike criteria plus scripted flows against it on the real Xiaomi.

- Runs on-device via Gradle (`connectedDebugAndroidTest`) using USB for install/bootstrap; runtime interaction then goes through the bridge itself over LAN (dogfooding).
- The 5 spike criteria become part of this suite, so mechanism regressions (e.g., a MIUI update) fail loudly instead of silently in the field.

## 6. Eval harness for agents (the differentiator)

Since agents are the real users, "does it work" must be measured where it matters: **can an agent complete the flow reliably?**

Build a **scenario runner** (CLI, Rust, in `crates/` or `tools/`):
- Scenario = JSON: `{ "name": "login_flow", "steps": [...], "assertions": [...] }` where steps reference bridge actions and `wait_for`, and assertions are predicates over `/ui-tree` state (e.g., `node_exists(text="Home")`), not vision-only.
- **Deterministic regression mode**: replay EXACT tool-call scripts against FakeBridge or a real bridge. Same commands, bit-for-bit, catches regressions without an agent.
- **Goal-based eval mode**: hand an agent a goal, run N times, collect pass rate per scenario, tokens spent, wall time. This is the metric that matters for the product claim.
- **Evidence bundles on failure**: `/tmp-style` bundle per failure containing fresh screenshot, `/ui-tree` dump at failure moment, tool error trace. Agents debug OTHER runs using them, so it is both a test artifact and a future agent feature.

Handles the hard CI truth: the real device is flaky by nature (network, MIUI battery killer); regression flakiness lives in the device harness, eval retries live in the eval harness, and neither blocks CI (nightly USB cron opens issues on failure instead of failing build).

## 7. What this buys the broader system

- Agents get `wait_for`, rev-guard, cheap trees from day 1 → their behavioral noise drops.
- FakeBridge scenarios double as agent-training playbooks and smoke-test rigs.
- The scenario JSON + evidence bundle becomes eventual agent-facing surface (a 7th/8th tool later: `run_scenario`, `last_evidence`) — noted as a ROADMAP-Next candidate ONLY when a second real scenario exists.

## Build order mapped to implementation milestones

| When | What tests get built |
|---|---|
| With issue #1 (spike 1) | contract fixtures + FakeBridge (2 scenarios minimum: static, stale) + Android unit tests |
| With issue #2 (spike 2) | device harness wired to sample gauntlet + deterministic replay mode |
| After milestone | goal-based eval mode + evidence bundles + nightly USB cron |

## Open decision point (grill me when ready)

Whether the sample gauntlet should be authored as scenario JSON (framework-neutral, doubles as agent packaging) or as Kotlin instrumented tests (more idiomatic Android; fixture and harness fuse). Recommendation: scenario JSON as the source of truth, instrumented tests as the executor — but this is a v-next decision, not a spike blocker.
