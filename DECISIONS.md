# Decisions Log

Append-only. One entry per decision: what, why, alternatives, what triggered it.

---

## 2026-10-30

**Product scope: devs test their OWN apps.** In-app bridge model. Generic any-app control is explicitly cut.
*Why:* Kills the Xiaomi/INJECT_EVENTS problem by construction; defines a shippable product (one Gradle line) instead of a fragile universal controller. *Alternative cut:* accessibility APK sidecar / HID emulation; abandoned because vendor security makes them whack-a-mole.

**First target: VelaVoice** (Gradle Android project).
*Why:* Real first client for the seam; Gradle plugin story is natural. Expo/RN config plugin is a future adapter if needed.

**Framework support: Gradle (Android) v1; Rust-built apps on adapter roadmap.**
*Why:* Gradle covers VelaVoice now. Rust adapter (Tauri etc.) is a future implementation behind the same transport seam. Two plugin ecosystems in v1 was rejected (doubles surface, muddies interface).

**Server language: Rust** for the MCP server.
*Why:* Single static binary, ~15-20MB RAM, cross-platform. Fits the 6GB-RAM desktop constraint. Alternatives (TS/Node server) rejected on footprint and distribution friction. Note: OpenCode context briefly contradicted itself on this; resolved toward Rust.

**Agent seam: MCP over stdio.**
*Why:* OpenCode is the day-one client; stdio MCP is natively consumable by OpenCode, Claude Code, Cursor, etc. Streamable-HTTP parked.

**Transport: LAN HTTP/WS with token auth for v1.** Wireless via adb pairing, Tailscale, WireGuard: parked as later adapters at the same seam.
*Why:* v1 must not build infra. Phone and PC share a network is an acceptable documented constraint; error message explains failures.

**Adoption seam: Gradle plugin, debug-builds-only enforced.**
*Why:* One line for the dev is the difference between adoption and abandonment. Release leak would be a security incident, so the plugin fails builds, not just warns.

**First milestone: "one app, one flow."**
*Why:* Forced pick after "a mix of these" got rejected in grilling. Two weeks: spike injection mechanism, hand-wired bridge, MCP server + 4 tools, agent completes one real VelaVoice flow over LAN unattended. Gradle plugin built only after the spike proves injection.

**Product stance: OSS, MIT, free.**
*Why:* Adoption and portfolio/internship positioning now; monetization can be layered later without breaking an MIT core.

**Xiaomi any-app workarounds (HID/AOA, accessibility APK, scrcpy+xdotool): explicitly cut.**
*Why:* Not needed once the bridge runs in-process. Research report methods 2-4 parked; Method 1 (in-app bridge) IS the product.

**Naming: `devhand`.** Placeholder; renaming is cheap at this stage.
