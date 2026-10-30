# Injection spike (day-1) — research verdict + experiment spec

## Where the spike code lives (corrected 2026-10-30)

Throwaway test code must NOT go into VelaVoice. Spike work happens inside `texure/`:

- **Spike 1 — mechanism proof** lives in `texure/android/bridge` (this becomes the real Bridge SDK module) + `texure/android/sample` (minimal app it injects into). Runs against the sample app on the Xiaomi. App-agnostic: own-window injection is not VelaVoice-specific.
- **Spike 2 — integration proof**: publish the bridge AAR to `mavenLocal`, add one dependency line to VelaVoice's debug build. Manual simulation of what the Gradle plugin will automate. If a one-line integration hurts here, the plugin design is wrong. Then run the real login/text flow on VelaVoice.

## Research verdict (2026-10-30, sourced)

Claims verified against Stack Overflow + TouchInjector (emanuele-f) docs + Android Instrumentation reference + dontkillmyapp/xiaomi:

1. **`Instrumentation.sendPointerSync` into OWN app: works without INJECT_EVENTS.**
   - TouchInjector doc: "successfully injects events into the same app but requires the INJECT_EVENTS permission to inject to other apps."
   - Confirmed independently by SO (android INJECT_EVENTS permission, 2011): injection into an Activity of one's own application succeeds; cross-app throws SecurityException.
   - Cross-app/system-wide injection is permanently blocked for normal apps (signature permission; scrcpy needs shell uid; MIUI examples in scrcpy issues #5201 confirm the block fires even for wireless-ADB setups).
   - Gotchas: must run on a NON-main thread; Android 13+ `setInTouchMode` needs extra perms (irrelevant to us); events are window-scoped to the instrumented app's windows.
2. **`view.dispatchTouchEvent` replay: zero-permission, in-process.** Root-view dispatch of hand-built MotionEvents works (multi-pointer gesture sequences supported). No system pipeline involvement; keyboard equivalent is `View.dispatchKeyEvent` / direct `EditText` mutation.
3. **AccessibilityService `dispatchGesture`: REJECTED.** MIUI kills accessibility services (battery saver, autostart, MIUI optimization, auto-disable after updates). Dev UX is a settings maze and it's the thing dontkillmyapp documents as most fragile on Xiaomi.
4. **MIUI battery killer also threatens OUR foreground service.** Not an injection problem, a lifetime problem. Dev checklist applies: battery "No restrictions", autostart. There are detection libs (github.com/XomaDev/MIUI-autostart) to warn instead of silently dying.

**Conclusion:** the Xiaomi `INJECT_EVENTS` wall only exists for system-wide injection. Our bridge injects into its own window only, so the wall never triggers. The in-app bridge architecture survives. Spike bets on (1) with (2) as fallback; (3) stays rejected.

## Spike experiment spec (VelaVoice, debug build)

- Hand-wired (no Gradle plugin yet): debug-only component that starts a tiny bridge on a fixed port.
- Target element: one button on VelaVoice's main screen.
- Acceptance criteria (all must pass):
  1. `sendPointerSync` ACTION_DOWN/UP programmatically lands on the button and its handler fires. No SecurityException.
  2. Fallback: `dispatchTouchEvent` replay achieves the same, including a scroll gesture (DOWN, N MOVEs, UP).
  3. Keyboard: `dispatchKeyEvent`/`sendKeySync` types into an editable field.
  4. Works while app is foregrounded AND with app in background + service alive (foreground service).
  5. `/ui-tree` returns the real node list; `/screenshot` returns a real PNG.
- If all five pass, the Bridge SDK's injection adapter = sendPointerSync, with dispatchTouchEvent as documented fallback.
