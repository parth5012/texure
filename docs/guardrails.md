# Guardrails

Hard limits for texure implementation. Breaking one is a design regression, not a shortcut. AGENTS.md carries the summary; updates land here first.

## Build & ship
1. Release variants NEVER contain bridge code. The Gradle plugin fails the build on leak (fail, not warn).
2. No code in target-app repos, ever. Target apps receive exactly ONE dependency line (issue #2 path only).

## Runtime dependency rules
3. Root device? Never. No su, no `/system` installs, no platform system signatures.
4. ADB: optional setup/bootstrap ONLY, never a runtime dependency. If a feature needs ADB at runtime, the design is wrong (locked-down phones like Xiaomi + 6GB PCs are the supported constraint).

## Injection
5. Accessibility-service-based injection: rejected permanently (MIUI kills the services). Own-window injection only (`sendPointerSync` primary, `dispatchTouchEvent` fallback).
6. No injection outside the bridge's own windows: no system UI, notification shade, nav bar, other apps. Scoped exception (Group R, maps 10–13): Shizuku gateway integration is permitted strictly for remote companion lifecycle (wake, keyguard dismiss, cold launch via `am start`, runtime permissions via `pm grant`) and fallback system tap for OS permission popups outside the app's windows under agent control.

## Agent surface
7. MCP tool list frozen at 6 tools. Additions/renames/shape changes require a DECISIONS.md entry first.
8. Errors: one-line `CODE: message` + hint always; full JSON only behind `verbose:true`. Server never dies from a tool error.
9. Human touch on the device ALWAYS beats in-flight agent actions (`human wins` is behavioral law; never try to out-prioritize a human).

## Privacy & safety
10. Screen/locked state = hard refusal for reads AND writes (`SCREEN_OFF`, `DEVICE_LOCKED`). Never read the screen contents while locked. (In-app bridge refuses reads/writes while locked; remote companion via Shizuku gateway in BB-R7 may execute guarded wake + keyguard dismiss prior to bridge session).
11. The bridge listens on LAN only; must refuse off-LAN/cloud transport attempts in v1 contract terms. Scoped architecture (Group R, maps 10–13): The in-app bridge itself maintains this LAN-only boundary. Remote unattended test devices utilize an external standalone companion service establishing an outbound, TLS-encrypted Reverse WebSocket tunnel (`wss://`) with bearer token authentication for NAT traversal, preserving in-app bridge transport isolation.

## Project hygiene
12. `DECISIONS.md` before behavior changes; `LOG.md` every session; `ROADMAP.md` moves tied to decisions.
13. Tests: blackbox red-and-green stays protocol-only; behaviors map to test IDs; evidence bundles on all red runs.
