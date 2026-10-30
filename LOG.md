# Log

## 2026-10-30
- Repo created from grilling session. Docs only: README, ARCHITECTURE, docs/bridge-contract.md, docs/lld-outline.md, DECISIONS.md.
- Status: Ready. Next: LLD dig-in starting with injection mechanism spike decision; code after contract freeze.
- Renamed to texure; GitHub repo created (parth5012/texure, public).
- LLD round 2 decisions baked into contract (rev guard, /ws + wait_for, fg service, overlay pairing, login/text flow).
- Injection research done inline (subagent infra failed twice): own-window injection needs no INJECT_EVENTS; AccessibilityService rejected; spike spec written.
- Spike placement corrected: throwaway code lives in texure/android/sample, never in VelaVoice.
- ROADMAP.md added (now/next/later/not-doing).
- Spike scaffold request PARKED by user as GitHub issues: #1 spike 1 (bridge + sample + 5 criteria), #2 spike 2 (VelaVoice one-liner + agent flow). Implementation deferred.
- docs/testing-architecture.md added: test plan through the three seams, FakeBridge second adapter, scenario DSL, agent eval harness; build order mapped to issues #1/#2.
- docs/blackbox-plan.md added + implementation parked as issue #3 (BB runner P0/P1). Suite is write-first: runs red before implementation, flips green across #1/#2 milestones.
- Blackbox design audited for completeness: 6 gaps found and closed (scenario/assertion DSL v0 schema, tool-freeze + token-budget + token-rotation test IDs A0/A6/B11, report/CI contract, sequential-per-target rule, plugin-guard ownership boundary). Issue #3 updated with the additions.- Behavior spec authored (docs/behavior-spec.md) via LLD round 3 grilling; DECISIONS carries 10+ entries; docs now: architecture, contract, behavior, spike, testing, blackbox plan.
- ROADMAP extended with user brainstorm: record+replay, regression tracking, crash capture, ADB bootstrap, screen mirroring (scrcpy MCP), config/deep-link matrices, perf probes, assertion SDK, trace viewer.
- User refinements applied: record+replay also exports codemode scripts for agent self-scripting; crash/ANR capture promoted to ASAP tier (issue #4); ADB confirmed optional-only, never a runtime dependency; deep-link matrix explained and kept Later.
