import json
import os
import subprocess
import time
import sys
from run_council_round import acp_call, TASK_PROMPT_TEMPLATE, COUNCIL_MEMBERS

def run_member(member, round_num=6):
    req_id = f"council-r{round_num}-{member['name'].replace(' ', '-').replace('.', '')}-{int(time.time())}"
    print(f"\n==========================================")
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
    print(f"Task ID: {task_id}")
    
    start_time = time.time()
    while True:
        status_res = acp_call("task_status", {"taskId": task_id})
        sc = status_res.get("structuredContent", {})
        st = sc.get("status")
        ws = sc.get("workState")
        if ws == "result_available" or st in ("completed", "failed", "cancelled"):
            summary = sc.get("summary") or sc.get("latestTerminalSummary")
            print(f"Finished with status: {st}")
            print(f"\n--- Summary for {member['displayName']} ---")
            print(summary)
            return summary
        time.sleep(5)
        if time.time() - start_time > 600:
            print("Timed out after 10m.")
            return None

if __name__ == "__main__":
    target = sys.argv[1] if len(sys.argv) > 1 else "deepseek v4.1 flash"
    r_num = int(sys.argv[2]) if len(sys.argv) > 2 else 6
    for m in COUNCIL_MEMBERS:
        if m["name"] == target:
            run_member(m, r_num)
            break
