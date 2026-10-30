# LLD outline / dig-in list

Things to nail down when we start LLD. Resolve in order.

1. **Injection mechanism (day-1 spike, blocks everything)**
   - Candidates: Instrumentation#sendPointerSync from app process, View.dispatchTouchEvent replay, dispatchGesture via in-app accessibility banner. Confirm which works under Xiaomi/MIUI, and whether instrumentation grants INJECT_EVENTS to the app being instrumented.
   - Acceptance: a tap lands on a button in VelaVoice debug build over LAN, no SecurityException, no settings toggles the dev must flip beyond enabling the plugin.

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

Each item: when resolved, write the answer into `docs/bridge-contract.md` or ARCHITECTURE.md, and log it in DECISIONS.md.
