# LLD outline / dig-in list

Things to nail down when we start LLD. Resolve in order.

1. **Injection mechanism (day-1 spike, blocks everything)**
   - Candidates: Instrumentation#sendPointerSync from app process, View.dispatchTouchEvent replay, dispatchGesture via in-app accessibility banner. Confirm which works under Xiaomi/MIUI, and whether instrumentation grants INJECT_EVENTS to the app being instrumented.
   - Acceptance: a tap lands on a button in VelaVoice debug build over LAN, no SecurityException, no settings toggles the dev must flip beyond enabling the plugin.

2. **Node model and ids**
   - id source: test tag > view id name > text > class+index fallback? Stability contract across refreshes. Write it into contract doc.

3. **Targeting scheme**
   - Array-index-from-last-tree vs stable node id vs raw coords. Pick one for v1 (above doc says index; verify it survives a real refresh flow).

4. **Bridge lifecycle inside the app**
   - Thread model for the HTTP server; foreground-service vs in-process; how to keep it alive while screen is on but app is backgrounded (common when agent backgrounds the app).
   - Shutdown path when user leaves debug build.

5. **Gradle plugin mechanics**
   - How to add a dependency + init hook to debug source sets only, from a plugin. Manifest injection vs Application subclass hook vs startup initializer.
   - Where the pairing token comes from and how it reaches both app and MCP server config.

6. **MCP server shape**
   - Tool list freeze (5 tools), error taxonomy mapping bridge codes to MCP tool errors, screenshot plumbing (PNG to tool result image).
   - Rust crates: mcp microservice SDK choice, HTTP client, config file location.

7. **Wait/sync semantics**
   - `/ui-tree` after tap needs client-side polling strategy? Or a `/wait` route with conditions. Decide who owns waits.

8. **Crate/module split freeze**
   - Finalize tree: crates/texure-server, android/bridge, android/gradle-plugin, contract/.

Each item: when resolved, write the answer into `docs/bridge-contract.md` or ARCHITECTURE.md, and log it in DECISIONS.md.
