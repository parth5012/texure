# texure

**Put an agent-controllable bridge inside your debug app. Drive it from any MCP host over your LAN.**

One Gradle plugin line injects a bridge service into a debug (never release) build of an Android app. The bridge serves the app's live UI tree, taps, typing, key events, and screenshots over LAN HTTP. A tiny Rust MCP server turns those endpoints into ~5 tools any agent harness can drive (OpenCode, Claude Code, Cursor, ...).

No ADB dependency for touching the device. No emulator. No INJECT_EVENTS hacks. A phone with 6GB-RAM desktop is a supported constraint.

## Why

Agent-driven mobile testing today either boots emulators (kills small dev machines) or fights vendor security (Xiaomi blocks `INJECT_EVENTS` for `shell`). texure skips both: the app under test serves its own UI state from inside its own process, and the agent's host just connects over the network.

## Status

Docs and decisions first. See [ARCHITECTURE.md](ARCHITECTURE.md), [ROADMAP.md](ROADMAP.md), and [docs/](docs/). Code lands after the day-1 injection spike is proven.

- Target: 2-week slice = one app (VelaVoice), one real flow, driven unattended by an agent over LAN.
- Roadmap: framework adapters (Expo/RN config plugin, Rust-built apps via Tauri etc.), wireless pairing, Tailscale/WireGuard transports, CI USB runner.

## License

MIT (planned).
