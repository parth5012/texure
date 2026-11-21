import json
import os
import subprocess
import time
import sys

COUNCIL_MEMBERS = [
    {
        "name": "deepseek v4.1 flash",
        "displayName": "DeepSeek V4.1 Flash",
        "providerInstanceId": "opencode",
        "model": "opencode/deepseek-v4.1-flash-free",
    },
    {
        "name": "hy4",
        "displayName": "Hy4 Flash",
        "providerInstanceId": "opencode",
        "model": "opencode/hy4-flash-free",
    },
    {
        "name": "beam",
        "displayName": "Beam",
        "providerInstanceId": "opencode",
        "model": "opencode/beam-free",
    },
    {
        "name": "sonnet 4.6",
        "displayName": "Claude Sonnet 4.6",
        "providerInstanceId": "antigravity",
        "model": "claude-sonnet-4-6",
    },
]

TASK_PROMPT_TEMPLATE = """You are serving as an expert design reviewer on the Council to evaluate Round {round_num} of the "UI polish overhaul" of Texure Companion (an Android developer testing bridge & gateway desktop/web app).

Goal: Determine if the UI is "Robin-approved" — looks significantly better, easier on the eyes, highly refined, with modern typography, frosted glass surfaces, cohesive dark and light modes, scannable cards, and professional micro-interactions.

Context & Exhaustive Refinements implemented directly from Council Round 5 feedback:
1. CSS Cascade & Utility Precedence: Moved `.glass-panel` into `@layer components` and separated `border-width`, `border-style`, and `border-color`. Utility classes like `border-dashed border-indigo-400` now reliably render on the Dev Shortcut card in both modes.
2. High-Contrast Action CTAs: Upgraded `btn-reconnect-tunnel` with `border-indigo-500 hover:border-indigo-600 bg-indigo-50 hover:bg-indigo-100 dark:bg-indigo-950/60 text-indigo-700 dark:text-indigo-300 font-semibold`, providing prominent contrast and immediate visual clarity.
3. Persistent Active Conduit Track: The gateway topology conduit now features a persistent base gradient (`from-indigo-500/40 via-violet-500/40 to-emerald-400/40`) under the animated pulse sweep, completely resolving the "empty/stuck progress bar" effect and reading as an energized data conduit.
4. Content-Driven Terminal Height: Log container uses `min-h-[140px] max-h-[520px]`, eliminating the tall empty black void when displaying initial logs while preserving fixed-width `w-24` source column alignment.
5. Toast Deck Header & Nav Clearance: Toasts repositioned to `top-32` (`top-[8.25rem]`), clearing both the 64px header and 48px navigation band.
6. Tactile Filled Secondary Buttons: Replaced hollow wireframe secondary buttons with filled tactile surfaces (`bg-slate-100 hover:bg-slate-200/90 text-slate-800 border-slate-300 dark:bg-zinc-800 dark:border-zinc-600`), establishing optical weight harmony between light and dark modes.
7. Card 1 Semantic Footer: Corrected duplicate "Input Dispatcher" label to "Shell Protocol".
8. Unified Status Badge Grammar: Card 4 badge updated to status-derived pill with dot (`[dot] {networkType} • {batteryPercent}%`), matching Cards 1-3.
9. Navigation Tab Text Contrast: Inactive tabs upgraded to `text-slate-600 dark:text-zinc-400` (contrast >= 4.7:1 against segmented background, exceeding WCAG AA).
10. Machine Metal Surfaces: Deep near-black `#09090b` with `#1e1e24` milled card tops, zero-void telemetry wells with `flex-1 flex flex-col justify-center`, and Inter + JetBrains Mono typography.

Please review the implementation and fresh screenshots in this repository:
Screenshots:
- tmp/screenshots/dashboard_dark.png
- tmp/screenshots/dashboard_light.png
- tmp/screenshots/controls_dark.png
- tmp/screenshots/controls_light.png
- tmp/screenshots/scanner_dark.png
- tmp/screenshots/scanner_light.png
- tmp/screenshots/logs_dark.png
- tmp/screenshots/logs_light.png

Source code:
- companion/src/App.tsx
- companion/src/components/StatusDashboard.tsx
- companion/src/components/ControlPanel.tsx
- companion/src/components/QrScanner.tsx
- companion/src/components/LogViewer.tsx
- companion/src/components/MockToolbar.tsx
- companion/src/index.css
- companion/tailwind.config.js

Evaluate along 3 dimensions:
1. Visual Craft & Elegance (typography, contrast, surface architecture, colors, spacing)
2. Usability & Scannability (hierarchy, status visibility, action clarity, information density)
3. "Easy on the eyes" & Cohesion (luxurious deep dark mode, crisp clean light mode, subtle accents vs harsh borders)

Provide your concrete review comments and conclude with your explicit design quality score in the format:
Score: X/10 (e.g. Score: 9/10, Score: 9.5/10, or Score: 10/10).
"""

def acp_call(tool, args_dict):
    node = os.environ.get("T3_ACP_MCP_NODE", "/home/parth/.t3/runtime/versions/0.1.12-nightly.20261121.5024/t3")
    entrypoint = os.environ.get("T3_ACP_MCP_ENTRYPOINT", "")
    cmd = ["acp-mcp-call", tool, json.dumps(args_dict)]
    env = dict(os.environ)
    env["ELECTRON_RUN_AS_NODE"] = "1"
    
    full_cmd = [node]
    if entrypoint:
        full_cmd.append(entrypoint)
    full_cmd.extend(cmd)
    
    proc = subprocess.run(full_cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if proc.returncode != 0:
        raise RuntimeError(f"acp-mcp-call error: {proc.stderr}")
    return json.loads(proc.stdout)

def run_round(round_num=1):
    print(f"=== Starting Council Round {round_num} ===")
    tasks = {}
    
    for member in COUNCIL_MEMBERS:
        req_id = f"council-round-{round_num}-{member['name'].replace(' ', '-').replace('.', '')}"
        print(f"Delegating to {member['displayName']} ({member['model']})...")
        payload = {
            "task": TASK_PROMPT_TEMPLATE.replace("{round_num}", str(round_num)),
            "role": "review",
            "isolation": "shared",
            "target": {
                "providerInstanceId": member["providerInstanceId"],
                "model": member["model"]
            },
            "clientRequestId": req_id,
            "title": f"Council Review R{round_num}: {member['displayName']}"
        }
        res = acp_call("delegate_task", payload)
        task_id = res.get("structuredContent", {}).get("taskId")
        tasks[member["name"]] = {
            "member": member,
            "taskId": task_id,
            "status": "starting",
            "summary": None,
            "score": None
        }
        print(f"  -> Task ID: {task_id}")

    # Poll tasks until all are completed
    start_time = time.time()
    while True:
        all_done = True
        for name, data in tasks.items():
            if data["status"] in ("completed", "failed", "cancelled"):
                continue
            all_done = False
            status_res = acp_call("task_status", {"taskId": data["taskId"]})
            sc = status_res.get("structuredContent", {})
            st = sc.get("status")
            ws = sc.get("workState")
            data["status"] = st
            if ws == "result_available" or st in ("completed", "failed", "cancelled"):
                data["summary"] = sc.get("summary") or sc.get("latestTerminalSummary")
                print(f"[{time.strftime('%H:%M:%S')}] {data['member']['displayName']} finished with status: {st}")
        
        if all_done:
            break
        time.sleep(5)
        if time.time() - start_time > 600:
            print("Timeout waiting for council after 10 minutes.")
            break

    print("\n=== Council Review Results ===")
    for name, data in tasks.items():
        summary = data.get("summary") or "No summary provided"
        print(f"\n--- {data['member']['displayName']} ---")
        print(summary[:500] + "..." if len(summary) > 500 else summary)
        
    return tasks

if __name__ == "__main__":
    r_num = int(sys.argv[1]) if len(sys.argv) > 1 else 1
    run_round(r_num)
