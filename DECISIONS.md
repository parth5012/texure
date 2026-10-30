# Decisions Log

Append-only. One entry per decision: what, why, alternatives, what triggered it.

---

## 2026-10-30 (spike placement correction)

**Spike code lives only in texure.** VelaVoice never receives test scaffolding. Spike 1 (mechanism proof) runs in `texure/android/bridge` + `texure/android/sample`, app on-device. Spike 2 (integration proof) publishes the bridge AAR to `mavenLocal` and adds one dependency line to VelaVoice's debug build, manually simulating the plugin. Rationale: throwaway code inside a product repo is future deletion work, and the product's core bet IS the one-line integration, so it should be exercised first, not last.

## 2026-10-30 (LLD grilling, round 2)

**Staleness guard: revision counter.** `/ui-tree` returns `rev`; `/tap` and `/type` must echo the rev they acted on. Bridge refuses on mismatch with `NODE_OUTDATED`; the agent re-fetches. Alternatives cut: resolve-late (silent wrong taps), re-fetch-on-tap (still racy), semantic hash (extra identity machinery for v1).

**Sync semantics: explicit `wait_for` MCP tool backed by WebSocket push.** 6 tools total now. Transport is WS (`tree_changed {rev}` pushed from bridge); webhook rejected for v1 because the PC is a NAT'd listener. `wait_for` timeout returns `ok:false` + one fresh `/ui-tree` snapshot so the agent can re-plan. Agent-layer polling is banned by design.

**Bridge process shape: debug-only foreground service.** Survives agent-driven backgrounding mid-flow. Autostart-on-boot rejected (MIUI autostart pain, parked).

**Pairing: in-app debug overlay, QR code deferred.** Dev copies URL + token once into MCP server config. QR is future polish.

**First milestone flow: login or simple text flow on VelaVoice.** Chosen over voice round-trip on past experience; voice path has more async to chew.

---

## 2026-10-30 (earlier)

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

**Naming: `texure`.** First working name was `devhand`, scrapped in discussion. `texure` is a coined word and we own that story. Renaming later is still cheap at this stage.
