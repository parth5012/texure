# LLD: CI/CD + distribution (surface D)

Decided via grilling 2026-10-30. Owner: cross-cutting; release recipe touches every issue.

## Pipeline shape

Trigger order in the PR matrix — later jobs skip when earlier fail:

1. `lint` — texure-lint (docs-gate P0 from issue #5; ast-grep rules join when code exists). Runs FIRST, nothing else runs on red lint.
2. `rust-test` — cargo test across `crates/*` + blackbox-fake suite (issue #3 F-tier targets).
3. `android-build` — assembleDebug + release-leak probe (Gradle-plugin check, issue #2). Job activates when plugin code lands.
4. `release` — tag-driven only (below).

### Merge gates (policy)

| Job | Gate on PR to main? |
|---|---|
| lint | gate |
| rust-test | gate |
| android-build | gate when code present; skipped before #2 lands |
| nightly device runs | never a gate; opens issues from FAIL findings |

## OS targets per PR

Two jobs: **ubuntu + windows**. Windows is the shipping platform for target devs (this PC is the dogfood host) — must run per PR, not at release. macOS runs only at release tag. PR feedback target: under ~5 min.

## Rust server distribution

- **Prebuilt binaries** per tagged release: x86_64-pc-windows-msvc + x86_64-unknown-linux-gnu + aarch64-apple-darwin, uploaded to `gh release` with SHA256 checksums.
- **`cargo install texure --locked`** as the secondary path for Rust-fluent devs.
- Quickstart shows the prebuilt download first.

## Android bridge distribution (v1)

- **mavenLocal + quickstart example in this repo**: `examples/` dir holds an example app + README (clone → publish plugin to mavenLocal via `./gradlew :plugin:publishToMavenLocal` → add classpath → run).
- NOT a separate quickstart repo in v1 (convenience for users, overhead for dev — parked below).
- JitPack / Plugin Portal / Maven Central: NOT-doing-listed until the bridge itself is post-spike.

## Release recipe (tag → shipped)

1. Tag `vX.Y.Z` → release workflow: cross-build the 3 targets, checksum, `gh release` upload.
2. Binaries verified → `cargo publish` (crate + binaries from the same tag).
3. Versioning: semver. **Breaking MCP-contract change = major bump**, contract doc update freezes in the same commit as the version bump.
4. Playbook doc: `docs/release-runbook.md` (authored with issue #2, since the AAR ship step needs the plugin to exist).

## Nightly device runs

- **Scheduler: T3 Code scheduled tasks** (not cron-system, not Windows Task Scheduler, not GitHub cron). Interval schedule invokes the run script on this PC: device check → run blackbox P0 against real bridge → emit `out.json` → FAILs opened as GitHub issues via `gh`.
- Runs only when PC + phone are on; missed runs are fine, no catch-up logic.
- GitHub Actions has no role in device runs (LAN-only device).

## Parked (roadmap Later)

- Separate `texure-quickstart` repo: revisit only if external adoption justifies the maintenance split ("secret repo" acceptable form).
- cargo-dist, package-manager manifests (winget/scoop/homebrew).
- Real Maven publishing.

## Open questions

- NIGHTLY-INVOCATION: exact T3 task shape (schedule interval + `desktop` vs headless) — decide when issue #3's P0 target actually runs on device; blocked by #1.
- CARGO-CRATE-NAME: confirm `texure` is free on crates.io before the first `cargo publish` (check belongs in release runbook).
