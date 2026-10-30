# Custom linting plan

texure's custom linters, designed via grilling (2026-10-30). Complements docs/testing-architecture.md; lints guard the METAS — docs, contracts, scenarios — that the blackbox suite then enforces at runtime.

## Stance

Half of texure's truth lives in docs (contract, behavior rows, guardrails). Drift between docs and code is the top silent-failure mode. So: linters cover docs AND code, in two classes with different engines.

## Two lint classes

### Class D — docs/contract/scenarios → Rust crate `texure-lint`

Checks (all JSON/Markdown-structural, no AST):
1. **Docs-gate:** every behavior-spec row ID ↔ blackbox test ID bidirectional coverage; contract route list ↔ frozen MCCP tool list consistency.
2. **Scenario DSL:** `schema_version: 1` enforcement, unknown `action` values, forbidden `sleep` steps, predicate vocabulary, unresolvable `$tree.*` refs (per blackbox-plan DSL section).
3. **Doc hygiene:** AGENTS.md ≤ 60 lines, LOG.md has an entry per calendar session day in use, scenario fixture files exist for every FakeBridge scenario reference.
4. **Fixtures:** route examples stay parseable by the real serde structs (crate `texure-contract` shared with CI tests).

Engine: Rust + serde; same parsing code paths as `texure-contract` and the blackbox runner.

### Class C — code anti-patterns → ast-grep YAML rules in `lint/ast-grep/`

Rules map 1:1 to guardrails.md numbers:
| Rule intent | Guardrail | Pattern (target) |
|---|---|---|
| Forbidden input injection outside injection adapter | G5/G6 | `InputManager.injectInputEvent`, `AccessibilityService.*` subclasses, reflection `getDeclaredMethod("injectInputEvent")` — anywhere EXCEPT `android/bridge/injection/` |
| ADB/system.exec usage | G4 | `Runtime.exec`, `ProcessBuilder` with `su`/`adb` strings |
| Release-leak pattern hints | G1 | bridge imports appearing in non-debug source sets (Gradle-plugin check owns the final say; this is a code-level tripwire) |

Authoring: plain YAML, tree-sitter Kotlin + Rust grammars.

## Orchestration: one report, two engines

`texure-lint` is the single entrypoint:
- runs Class D checks natively,
- invokes the Class C executor (see below), normalizes findings,
- applies severity tiers, emits `lint-report.json` (same shape as blackbox `out.json`: `FAIL/WARN:severity, rule, file, line, message`).

| Severity | Tier on build result |
|---|---|
| Guardrail violation, contract/tool freeze break, scenario schema break | FAIL |
| Doc freshness (LOG staleness), AGENTS.md budget drift | WARN |
| Report-only, zero false-positive class | WARN initially, graduates on confidence |

Exit code nonzero iff any FAIL. The nightly cron parses `lint-report.json` into issues, same as blackbox reports.

## Class C executor policy: perfgym dogfood, time-boxed

- Rules portable as YAML; run **perfgym's linter when convenient**, plain `sg scan` otherwise. No hard dependency.
- **Experiment window: 2 weeks from first execution attempt.** Exit criteria for perfgym adoption: runs the full ruleset, stable exit codes, findings match sg baseline 1:1. Otherwise: `sg scan` fallback becomes permanent with zero rewrites (rules unchanged).
- Dogfooding outcomes logged in texure LOG.md +回馈 feedback filed to perfgym.

Repo-wide invariant (aligns with perfgym's Invariant Gate): the lint job runs FIRST in CI matrix; nothing else runs on a red lint.

## Scope freeze for v1

Included (issues #5):
- Class D docs-gate + scenario DSL + hygiene + fixtures checks
- Class C forbidden-API guardrail rules (the 3 rule families above)

Deferred (v1.1, ROADMAP Next):
- Architecture crate-boundary lints (cargo module graph checks)
- Token-budget lint over fixtures (A6 blackbox test already enforces; lint duplicate only if fixture authoring gets noisy)
- stdio/inject adapter allowlist maintenance UI/lint improvements
