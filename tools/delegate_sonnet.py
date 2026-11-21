import json
import os
import subprocess
import time
import sys

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

from run_council_round import TASK_PROMPT_TEMPLATE

payload = {
    "task": TASK_PROMPT_TEMPLATE.format(round_num=2),
    "role": "review",
    "isolation": "shared",
    "target": {
        "providerInstanceId": "antigravity",
        "model": "claude-sonnet-4-6"
    },
    "clientRequestId": "council-round-2-sonnet-46-retry",
    "title": "Council Review R2: Claude Sonnet 4.6"
}

print("Delegating to Claude Sonnet 4.6 on antigravity...")
res = acp_call("delegate_task", payload)
print("Response:", json.dumps(res, indent=2))
task_id = res.get("structuredContent", {}).get("taskId")
print(f"Task ID: {task_id}")

while True:
    status_res = acp_call("task_status", {"taskId": task_id})
    sc = status_res.get("structuredContent", {})
    st = sc.get("status")
    ws = sc.get("workState")
    print(f"Status: {st}, WorkState: {ws}")
    if ws == "result_available" or st in ("completed", "failed", "cancelled"):
        summary = sc.get("summary") or sc.get("latestTerminalSummary")
        print("\n=== Result ===")
        print(summary)
        break
    time.sleep(5)
