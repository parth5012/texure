# LLD: Gradle plugin mechanics (surface B)

Authoritative design for `android/gradle-plugin` (module `com.texure.bridge`). Resolved 2026-11-01, LLD grilling round 10 (HITL ticket [LLD B: Gradle plugin mechanics](https://github.com/parth5012/texure/issues/9), map 2). Change the contract doc FIRST, then this doc, then code. status: RESOLVED (item 5 of lld-outline).

---

## 1. Goal & Architecture Seam

The plugin automates the single-dependency promise for target apps (Guardrail G2: exactly ONE line, zero source changes). It injects the bridge into debug variants only, binds activity lifecycle callbacks automatically, and enforces Guardrail G1 by failing the build if bridge code leaks into release variants.

### Core Seam
```
Host App build.gradle(.kts)
    │  apply plugin: "com.texure.bridge"  (or plugins { id("com.texure.bridge") })
    ▼
Gradle Plugin (com.texure.bridge)
    ├── 1. Injects `debugImplementation("com.texure:bridge:<version>")` into Android dependencies
    ├── 2. Configures `checkTexureReleaseLeak` task attached to release build pipeline
    └── 3. Binds debug ContentProvider (`TexureInitProvider`) in debug manifest for auto-init
```

---

## 2. Debug-Only Wiring Mechanics

### 2.1 Initialization Mechanism: Debug ContentProvider (`TexureInitProvider`)
To honor the zero-code-changes guardrail on the host application, the bridge uses an internal ContentProvider pattern (similar to Firebase Init Provider / AndroidX Startup).

- **Implementation:** `com.texure.bridge.init.TexureInitProvider : ContentProvider()` lives in `android/bridge`.
- **Manifest Placement:** Declared in `android/bridge/src/main/AndroidManifest.xml` (or debug manifest overlay).
  ```xml
  <provider
      android:name="com.texure.bridge.init.TexureInitProvider"
      android:authorities="${applicationId}.texure.init-provider"
      android:exported="false"
      android:multiprocess="true" />
  ```
- **Lifecycle Hook:**
  - In `onCreate()`, gets `context.applicationContext as Application`.
  - Registers `Application.ActivityLifecycleCallbacks`:
    - `onActivityCreated` / `onActivityResumed`: Checks `BridgeOverlay.install(activity)` on the active activity.
    - `onActivityDestroyed`: Detaches overlay cleanly.
    - Registers `BridgeCrashHandler` uncaught exception handler (ticket #4).
- **Release Safety:** Because the library is attached only to `debugImplementation`, this ContentProvider is absent from release variant manifests. Release builds do not execute a single instruction of init code.

#### Tradeoff Analysis & Alternatives Cut
| Mechanism | Evaluation | Verdict |
|---|---|---|
| **Debug ContentProvider** | Zero user code, automatic lifecycle hook, AGP stable, zero runtime footprint in release | **Selected** |
| **Bytecode Transform** | Injects calls directly into Activity bytecode; fragile across AGP versions, adds build latency | Cut |
| **Custom Application Subclass** | Requires user to edit `AndroidManifest.xml` or write Kotlin/Java code in target repo | Cut (violates G2) |

---

## 3. Pairing Token Plumbing

- Generated and managed on-device by `BridgeTokenManager` (`SharedPreferences` file `texure_bridge_pairing`, plaintext, app-private).
- Initialized on first boot, stable across rebuilds.
- Surfaced via:
  1. The in-app debug overlay (`BridgeOverlay`) pinned to the bottom of the active debug Activity.
  2. (Not shown in the foreground service notification; notification displays connection and lifecycle status text only).
  3. Logcat debug tag `TexureBridge: Pairing token: <token>`.
- Rotation: dev clicks "Rotate Token" on overlay → writes new token → immediately invalidates old holders with HTTP 401 `AUTH_FAILED`.

---

## 4. Release-Leak Enforcement (Guardrail G1)

### 4.1 Dedicated Gradle Task: `checkTexureReleaseLeak`
> **Status:** Planned design. This PR specifies the architecture; the task implementation will be delivered when `android/gradle-plugin` is authored.

The plugin registers a verification task for every release variant:
- **Task Name:** `checkTexureReleaseLeak<VariantName>` (e.g. `checkTexureReleaseLeakRelease`).
- **Hook Point:** Depends on `resolveReleaseRuntimeClasspath` and runs before `packageRelease` / `assembleRelease`.
- **Inspection Logic:**
  1. Scans the resolved `runtimeClasspath` and `compileClasspath` configurations for the release variant.
  2. Inspects component identifiers for group `com.texure` or artifact `bridge`.
  3. Scans merged release resources and manifest for `TexureInitProvider` or `BridgeService`.
- **Behavior on Leak:**
  - If any reference is found, **fails the build immediately** with a loud diagnostic error:
    ```
    BUILD FAILED: Guardrail G1 Violation!
    Bridge code detected in release variant '<variant>'!
    Found dependency: com.texure:bridge
    Bridge dependencies must only be added to debug configurations ('debugImplementation').
    ```
  - Never logs a warning; hard build failure is law (Guardrail G1).

---

## 5. Module Shapes & Target App DSL Examples

The plugin supports both Groovy DSL and Kotlin DSL without requiring target app adaptations.

### 5.1 Groovy DSL (e.g. VelaVoice `app/build.gradle`)
```groovy
plugins {
    id 'com.android.application'
    id 'com.texure.bridge' version '0.1.0'
}

// Or legacy buildscript apply:
apply plugin: 'com.texure.bridge'
```

### 5.2 Kotlin DSL (e.g. Modern Android `build.gradle.kts`)
```kotlin
plugins {
    id("com.android.application")
    id("com.texure.bridge") version "0.1.0"
}
```

---

## 6. Publication Recipe & Local Distribution

Per `docs/lld-ci.md` §3 (Android distribution):
1. **Module setup:**
   - `android/bridge`: Android library module, publishes AAR (`com.texure:bridge:0.1.0`).
   - `android/gradle-plugin`: Planned Gradle plugin module (JVM / Kotlin). When implemented, it will publish plugin marker and artifact (`com.texure.bridge:com.texure.bridge.gradle.plugin:0.1.0`).
2. **Local publication command:**
   ```bash
   ./gradlew publishToMavenLocal
   ```
3. **Consumer requirement:**
   - Target app's `settings.gradle` includes `mavenLocal()` in `pluginManagement { repositories { } }` to resolve the plugin marker (once plugin module is present).
   - Target app's project `repositories { }` (or `dependencyResolutionManagement`) includes `mavenLocal()` to resolve library `com.texure:bridge:0.1.0`.

---

## 7. Blackbox & Verification Mapping

Plugin crash and leak paths mapped to blackbox test IDs:
- **BB-A0 / BB-B1:** Verifies debug variant successfully starts bridge and responds to `device_tree`.
- **BB-G1 (Leak Check):** Verifies `./gradlew :sample:assembleRelease` fails if bridge is forcibly forced into release, and passes cleanly with 0 bridge classes when configured normally.
- **BB-B3:** Verifies pairing token plumbing surfaces in overlay and accepts requests.
