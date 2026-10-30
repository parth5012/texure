# texure Architecture

## One-liner

A Gradle plugin injects a tiny bridge into a debug build of a dev's app. The bridge serves its own UI tree, taps, typing, and screenshots over LAN HTTP. An MCP server on the dev's machine turns those endpoints into a handful of tools any agent harness can drive.

This kills the Xiaomi problem by construction: no INJECT_EVENTS tricks, no HID emulation, no accessibility APK. Phone and PC share a network; the app serves its own state. ADB is not required for v1 interaction.

## Three seams, each deep

(adapted from codebase-design vocabulary: interface = everything a user must know; depth = behavior behind a small interface; seam = where the interface lives)

```
 dev's build.gradle ──(1 line: plugin id)──▶ [ Gradle Plugin ]  <- adoption seam
      | hides: deps, debug-only wiring, bridge init, pairing token
      v
 app debug build ──(HTTP/WS ~6 routes)──▶ [ Bridge SDK ]        <- transport seam
      | hides: UI-tree serialization, gesture injection, waits
      v
 agent harness ──(MCP stdio, ~5 tools)────▶ [ MCP Server ]      <- agent seam
        hides: device discovery, retries, token auth, PNG plumbing
```

### 1. Bridge SDK (in-app, Android, Kotlin)

- **Interface:** one initializer + 6 routes: `/ui-tree`, `/tap`, `/type`, `/keypress`, `/screenshot`, `/back`.
- **Implementation (deep):** compact node JSON (`id`, `text`, `bounds`, `clickable`), gesture injection from inside its own process, tree diffing, waits.
- **Internal adapter slot:** the injection mechanism. The day-1 spike decides which mechanism holds.

### 2. Gradle Plugin (adoption seam)

The entire adoption story is one line for the dev. It:
- adds the bridge dependency to **debug** source sets only,
- wires the initializer (manifest or Application hook),
- generates/embeds a pairing token,
- fails the build if the bridge leaks into release variants (a LAN listener in a release build is a remote exploit).

### 3. MCP Server (agent seam, Rust)

- **Interface:** 5 tools, no more: `device_tree`, `tap`, `type`, `keypress`, `screenshot`.
- **Implementation:** explicit device URL + token for v1 (paste once, cached). mDNS discovery, Tailscale/WireGuard transports: parked. Retries, backoff, PNG plumbing, tool-schema plumbing live here, hidden.
- Rust choice: single static binary, tiny RAM (~15-20MB), cross-platform, fits the 6GB-RAM constraint.

## Connection contract (v1)

- Phone and dev machine share a LAN (Tailscale/WireGuard explicitly parked as later adapters at this seam).
- Dev configures the MCP server once with the phone URL + token.
- Anything else is an error message telling the dev exactly what failed.

## Module / repo layout (target shape)

```
texure/
  crates/texure-server/         # Rust MCP stdio server
  android/bridge/               # Bridge SDK (Kotlin, publishes to maven local)
  android/gradle-plugin/        # Gradle plugin (the 1-line adoption seam)
  contract/                     # shared HTTP + tool contract (schemas, examples)
  docs/
```

`contract/` is the single source of truth both implementations build against. No implementations without the contract doc being right first.

## Risks (ranked)

1. **Injection mechanism feasibility (day-1 spike).** Research says the Xiaomi wall only blocks cross-window injection; own-window injection needs no permission. Spike must still prove it live on a real MIUI device before we trust it.
2. **UI-tree fidelity.** Views without test tags get poor node ids. Mitigation: fall back to text/class, provide `set_tag` route, document tag recommendation for devs.
3. **Token-efficient tree size.** Deep view hierarchies can flood the agent's context. Mitigation: compact node schema, subtree filters, maybe depth caps in the agent-facing tool.
4. **MIUI battery killer** (foreground service lifetime). Documented dev checklist: battery "No restrictions" + autostart; detection libs exist (MIUI-autostart) to warn instead of silently dying.
5. **Release-build leak.** Gradle plugin must actively protect against it; it's also the product's trust story.
6. **LAN-only adoption friction** (dev on different networks, hotspot quirks, AP isolation). Document as error message in v1; Tailscale is the roadmapped cure.

## First target app

VelaVoice (Gradle Android project) at `D:\work\projects\VelaVoice`. Structure: monorepo with `sdk/` (Kotlin DSL, vela-core/common/voice-ui/whisper/cleaner), `velaboard/` (app, Kotlin DSL, minSdk 24 / target 35), and `velavoice app/android/` (Groovy DSL, minSdk 23 / target 34). The spike targets the `velavoice app/android` module first since that is the app build the Gradle plugin will inject into; plugin design must accommodate both Groovy and Kotlin-DSL module shapes across the monorepo.

## Injection mechanism (researched)

The Xiaomi INJECT_EVENTS wall exists only for cross-window/system-wide injection. In-app injection into the app's own windows needs no extra permission: `Instrumentation.sendPointerSync` (own windows, non-main thread) and/or `view.dispatchTouchEvent` replay. AccessibilityService-based injection is rejected (MIUI kills accessibility services). Details + spike spec: [docs/spike-injection.md](docs/spike-injection.md).

## Framework adapter roadmap (parked, deliberate)

Same bridge HTTP contract, different injectors per framework: Expo/RN config plugin, Flutter, Rust-built apps (Tauri etc.). Each adapter is an implementation behind the SAME transport seam. None of this is v1 ambition. Any-app control and vendor-security workarounds are explicitly cut things we do not build.
