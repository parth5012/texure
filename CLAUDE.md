# texure — Agent Instructions

Agent-controllable Android bridge (one Gradle line into debug builds) + Rust MCP server. Agents test YOUR OWN app over LAN.

## Skills (load before working, not after)
- Always load the best-practices skill for the language/layer in play BEFORE writing or reviewing non-trivial code: Rust crates → Rust patterns skill; Android/Kotlin/Gradle → Android patterns skill; MCP protocol → the closest protocol/agent-harness skill available.
- Module/API shaping → `codebase-design` (deep modules, seams). TDD work → `tdd` skill workflows.
- If no matching skill is installed for a layer, proceed with documented styles from that layer's docs, and note the missing skill in `LOG.md` so the gap can be filled.

## Source of truth
1. `docs/bridge-contract.md` → 2. `docs/behavior-spec.md` → 3. code
- Change contract/behavior docs BEFORE code. Breaking change = bump `schema_version` + `DECISIONS.md` entry
- Every behavior-spec row is law; violating one is a bug, not a choice
- Repo layout + commands: `docs/repo-map.md` (update it whenever the layout changes)
- Hard limits: `docs/guardrails.md` (full list; summary below)

## Work discipline
- ALL work starts as a GitHub issue, planning included. Wayfinder maps: work follows their tickets. Docs-only edits are the exception and need no issue
- Build order per behavior: 1) blackbox test design + implementation (red) → 2) standard/unit test design + implementation → 3) feature code
- Update `LOG.md` every session. Move a `ROADMAP.md` item only with a `DECISIONS.md` entry
- Never report done without running the phase-gate command for that issue

## Guardrails (summary — full list in `docs/guardrails.md`)
- Release builds never contain bridge code
- ADB is optional setup-only bootstrap; never a runtime dependency
- No accessibility-service or system-UI injection; own windows only
- MCP tool list frozen at 6; changes need a `DECISIONS.md` entry
- Experiment/throwaway code lives ONLY in `android/sample`; target-app repos receive exactly one dependency line (via issue #2 only)

## Testing rules
- Blackbox suite is protocol-only (MCP stdio / HTTP contract), never internals
- Each behavior-spec row maps to a blackbox test ID; a missing mapping is a suite bug — fix the suite first
- Device-only tests tagged `device`; skipped cleanly without a phone (never a build failure)
- Every red run writes an evidence bundle to `tmp/evidence/`
