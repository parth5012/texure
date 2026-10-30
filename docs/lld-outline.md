# LLD outline / dig-in list

Things to nail down when we start LLD. Resolve in order.

1. **Injection mechanism (day-1 spike, blocks everything)** — **RESOLVED BY RESEARCH** (see `docs/spike-injection.md`): the Xiaomi INJECT_EVENTS wall only blocks cross-window injection; own-window injection needs no permission. Spike bet: `Instrumentation.sendPointerSync` (own windows, non-main thread), fallback `dispatchTouchEvent` replay. AccessibilityService rejected (MIUI kills accessibility services). Spike experiment defined; not yet executed.
   - Acceptance: the 5 criteria in docs/spike-injection.md.
2. **Node model and ids**
   - id source: test tag > view id name > text > class+index fallback? Stability contract across refreshes. Write it into contract doc.

3. **Targeting scheme**
   - Array-index-from-last-tree vs stable node id vs raw coords. Pick one for v1 (above doc says index; verify it survives a real refresh flow).

4. **Bridge lifecycle inside the app** — part-resolved: debug-only **foreground service**. Remaining: thread model for embedded HTTP server; shutdown path when user leaves debug build.

5. **Gradle plugin mechanics**
   - How to add a dependency + init hook to debug source sets only, from a plugin. Manifest injection vs Application subclass hook vs startup initializer.
   - Pairing token flow: part-resolved (in-app debug overlay shows URL + token; QR deferred).

6. **MCP server shape**
   - Tool list freeze: **6 tools** (`device_tree`, `tap`, `type`, `keypress`, `screenshot`, `wait_for`). Error taxonomy mapping bridge codes to MCP tool errors, screenshot plumbing (PNG to tool result image).
   - Rust crates: mcp microservice SDK choice, HTTP client, config file location.

7. **Crate/module split freeze**
   - Finalize tree: crates/texure-server, android/bridge, android/gradle-plugin, contract/.

8. **CI/CD + distribution (surface D)** — **RESOLVED (2026-10-30, grilling round 5)**, see `docs/lld-ci.md`:
    - Merge gates: lint → rust-test are PR gates; android-build becomes a gate once plugin code lands; nightly device runs never gate.
    - OS targets: ubuntu + windows per PR (windows = ship platform); mac at release tag only.
    - Rust dist: prebuilt gh-release binaries (windows-msvc/linux-gnu/apple-darwin) + checksums; `cargo install --locked` secondary.
    - Android dist: mavenLocal + in-repo `examples/` quickstart (separate repo parked, see ROADMAP.Later).
    - Release recipe: tag v* → cross-build → checksum → gh release → cargo publish; semver, breaking MCP contract change = major.
    - Nightly device runs: T3 Code scheduled tasks on this PC; FAILs open issues via gh. GitHub Actions has no device role.
    - Remaining open: NIGHTLY-INVOCATION exact task shape (blocked by #1); crates.io name check at first publish.

Each item: when resolved, write the answer into `docs/bridge-contract.md` or ARCHITECTURE.md, and log it in DECISIONS.md.
