# Framework Adapters Specification

> [!NOTE]
> **Milestone Status: LATER (Post-v1.1 Roadmap)**
> Framework adapters are documented architectural extensions parked for implementation after the core product pillars (Maps 4–8), primary observability harness (Map 7), and remote companion daemon (Maps 9–13) have landed.

Defines the architecture and implementation roadmap for supporting non-pure-native Android development ecosystems: **React Native / Expo**, **Flutter**, **Tauri Mobile**, **Capacitor**, and **WebView/Electron-style** hybrid containers.

---

## 1. Architectural Principle: Frozen Transport Seam

Texure's core transport contract (`docs/bridge-contract.md`) and agent tool list (`docs/behavior-spec.md`) remain **100% frozen and invariant** across all frameworks. 

Every framework adapter implements the exact same 6 HTTP/WS routes (`/ui-tree`, `/tap`, `/type`, `/keypress`, `/screenshot`, `/ws`) and enforces the single-flight rev guard (`verify-at-execute`).

What changes per framework is strictly:
1. **The Adoption Seam (Packaging):** How the dev imports texure in 1 line without touching release builds.
2. **The Extraction Seam:** How the framework's visual hierarchy is mapped to standard `UiTree` JSON.
3. **The Injection Seam:** How synthetic taps and text commits are delivered to the framework's render tree.

```
       [ Expo / React Native ]   [ Flutter ]   [ Tauri Mobile ]   [ Capacitor / Hybrid ]
                  │                   │                │                    │
                  ▼                   ▼                ▼                    ▼
          (Config Plugin)       (Pub Plugin)     (Tauri Plugin)       (npm Plugin)
                  │                   │                │                    │
                  └─────────────┬─────┴────────────────┴────────────────────┘
                                │
                                ▼
                   [ Unified Bridge HTTP/WS API ]  <-- Port 8765 (Frozen Contract)
                                │
                                ▼
                   [ texure-server (Rust MCP) ]    <-- 6 Frozen Tools
                                │
                                ▼
                   [ AI Agents / OpenCode / AFK ]
```

---

## 2. Target Framework Specifications

### 2.1 React Native & Expo (`@texure/react-native`)

* **Adoption Seam:**
  - Expo: Config plugin in `app.json`:
    ```json
    { "plugins": ["@texure/expo-plugin"] }
    ```
  - Bare React Native: Auto-linked Gradle dependency in `android/app/build.gradle`:
    ```groovy
    debugImplementation project(':texure-react-native')
    ```
* **Extraction Seam:** React Native renders real native Android views (`ViewGroup`, `ReactTextView`, `ReactViewGroup`). The core `BridgeTreeExtractor` traverses them natively.
  - **ID Resolution:** `testID` props in JSX map directly to native View `tag` or `contentDescription`.
* **Injection Seam:** Native `sendPointerSync` and `dispatchTouchEvent` directly target RN views.

---

### 2.2 Flutter (`texure_flutter`)

* **The Flutter Challenge:** Flutter renders via Impeller/Skia onto a single native `FlutterSurfaceView` / `FlutterView`. Standard Android View traversal sees only one giant canvas view with 0 child nodes.
* **Adoption Seam:**
  - `pubspec.yaml` dependency:
    ```yaml
    dev_dependencies:
      texure_flutter: ^0.1.0
    ```
  - Auto-initializes via Flutter `BindingBase` in debug builds only.
* **Extraction Seam (Semantics Tree Adapter):**
  - Traverses Flutter's internal `SemanticsOwner` / `SemanticsNode` tree.
  - Maps Flutter `SemanticsNode` (label, hint, value, actions: `tap`, `showOnScreen`, `scrollLeft`) into texure's `UiNode` schema (`tag`, `text`, `cd`, `bounds`, `clickable`).
* **Injection Seam:**
  - Dispatches actions through Flutter's native `GestureBinding.instance.handlePointerEvent` or invokes semantics actions (`SemanticsAction.tap`).

---

### 2.3 Tauri Mobile (`tauri-plugin-texure`)

* **Adoption Seam:**
  - In `Cargo.toml`:
    ```toml
    [dependencies]
    tauri-plugin-texure = { version = "0.1", optional = true }
    ```
  - In `src-tauri/src/main.rs`:
    ```rust
    #[cfg(debug_assertions)]
    builder.plugin(tauri_plugin_texure::init());
    ```
* **Extraction Seam (Hybrid Native + DOM Bridge):**
  - Android native container view is traversed by `BridgeTreeExtractor`.
  - Inside the WebView, a lightweight DOM accessibility script runs via `evaluateJavascript()` to extract interactive HTML elements (`<button>`, `<a>`, `<input>`, `[role]`), projecting viewport-relative pixel coordinates into `UiTree`.
* **Injection Seam:**
  - Synthetic input injected into the native `WebView` window via `dispatchTouchEvent` at the computed pixel coordinates.

---

### 2.4 Capacitor & Ionic (`@texure/capacitor`)

* **Adoption Seam:**
  - Single npm install: `npm install @texure/capacitor --save-dev`
  - Auto-registered into debug Android Capacitor builds via `capacitor.config.ts`.
* **Extraction & Injection:**
  - Bridges Capacitor's Android Bridge with the DOM accessibility tree inside Chromium WebView.
  - Synchronizes coordinate scaling between device density and CSS pixels.

---

### 2.5 Hybrid & Mobile Electron / Chrome DevTools Protocol (CDP)

* For apps running Chromium WebViews or embedded Electron/Node environments:
  - Connects to the local Android Chromium debugging socket (`/proc/net/unix` abstract socket `@webview_devtools_remote_<pid>`).
  - Queries `DOM.getDocument` and `Accessibility.getFullAXTree` to resolve accurate semantic identities.

---

## 3. Implementation Phasing

| Framework | Target Module | Phasing | Prerequisite |
|---|---|---|---|
| **Android Native / Gradle** | `android/bridge`, `android/gradle-plugin` | ✅ Built / Map 5 | Milestone v1 (Now) |
| **Tauri Mobile (Companion)** | `companion/` (Tauri v2 + React) | 📋 Maps 10–12 | Milestone v1.1 (Next) |
| **React Native & Expo** | `adapters/texure-react-native` | ⏳ **Later** | Post-v1.1 |
| **Flutter** | `adapters/texure-flutter` | ⏳ **Later** | Post-v1.1 |
| **Tauri Mobile (Target App)** | `adapters/tauri-plugin-texure` | ⏳ **Later** | Post-v1.1 |
| **Capacitor & Ionic** | `adapters/texure-capacitor` | ⏳ **Later** | Post-v1.1 |
| **WebView / CDP Adapter** | `crates/texure-cdp` | ⏳ **Later** | Post-v1.1 |
