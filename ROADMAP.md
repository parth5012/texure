# Roadmap

Explicitly parked ideas live here so ARCHITECTURE.md stays about v1. Cadence: review when current milestone lands; never pull a "later" item into "now" without a DECISIONS.md entry.

## Now (current 2-week slice)

- Injection spike live-proof on real Xiaomi device (`docs/spike-injection.md`)
- Bridge SDK: 6 routes + `/ws`, hand-wired, no plugin
- Rust MCP server: 6 tools over stdio
- One agent-driven flow on VelaVoice over LAN (login/text)
- Gradle plugin (debug-only) — only after spike passes

## Next (v1.1-ish)

- **Gradle plugin hardening.** Release-leak guardrails (build fail, not warn), both Groovy and Kotlin-DSL module shapes, pairing token generation.
- **QR pairing.** In-app debug overlay renders QR encoding URL + token; MCP server camera-side is out of scope, so QR is for phone-to-phone or local viewer use.
- **Stable node ids** (beyond rev + index): semantic hash identity so agent memory of a node survives refreshes.
- **Screenshot scale presets + partial-region capture** to cut vision tokens.
- **`set_tag` route.** Dev can tag views from agent feedback, without rebuilding.
- **mDNS discovery** so no URL pasting at all.
- **Multi-device registration** in server config (phone profile list, select by profile name).

## Later (roadmap, deliberate separates)

- **Framework adapters** (same bridge HTTP contract, different injectors/packaging):
  - Expo/React Native config plugin
  - Flutter module
  - Rust-built apps (Tauri etc.) — the "rust app plugins" idea from the original plan
- **Wireless beyond LAN:** adb wireless debugging pairing flow; Tailscale/WireGuard transport adapters.
- **CI mode:** USB runner suite, device persists across runs, junit-style results out of flow scripts.
- **Emulator + scrcpy support:** window/keyboard mirroring as an observation channel, not as injection.
- **Web-build testing hooks** (same flows replayed against an Expo web build for cross-engine checks).
- **Battery killer auto-detection** on MIUI/HyperOS: detect restrictions and show a guided fix page instead of failing silently.

## Not doing (revisit only with real user pain)

- Any-app control (accessibility tablet, HID emulation, scrcpy+xdotool)
- Cloud relay / hosted bridge (infra + security surface, NAT'd PC listener)
- Monetization layer (MIT stays; product stance decides when a paid tier borders ever justified)
- Raw coordinate tap mode as the primary target scheme
