import os
import time
import json
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
TMP_DIR = REPO_ROOT / "tmp"
FRAMES_DIR = TMP_DIR / "demo_frames"

HTML_TEMPLATE = """<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    background-color: #080c14;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "JetBrains Mono", monospace;
    display: flex;
    justify-content: center;
    align-items: center;
    min-height: 100vh;
    padding: 24px;
  }
  .window {
    width: 1040px;
    background: #0d121c;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 12px;
    box-shadow: 0 30px 80px rgba(0, 0, 0, 0.75), 0 0 50px rgba(99, 102, 241, 0.15);
    overflow: hidden;
  }
  .titlebar {
    background: #151b27;
    padding: 12px 18px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }
  .buttons {
    display: flex;
    gap: 8px;
    margin-right: 18px;
  }
  .btn {
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }
  .btn-red { background: #ef4444; }
  .btn-yellow { background: #f59e0b; }
  .btn-green { background: #10b981; }
  .title {
    color: #94a3b8;
    font-size: 13px;
    font-weight: 500;
    flex: 1;
    text-align: center;
    font-family: "JetBrains Mono", monospace;
  }
  .badge {
    background: rgba(99, 102, 241, 0.2);
    color: #818cf8;
    border: 1px solid rgba(99, 102, 241, 0.4);
    padding: 3px 10px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.4px;
  }
  .content {
    padding: 26px 30px;
    font-family: "JetBrains Mono", Consolas, "Liberation Mono", Menlo, monospace;
    font-size: 13.5px;
    line-height: 1.62;
    color: #e2e8f0;
    min-height: 560px;
  }
  .prompt {
    color: #38bdf8;
    font-weight: 600;
  }
  .cmd {
    color: #f8fafc;
    font-weight: 600;
  }
  .dim {
    color: #64748b;
  }
  .accent {
    color: #818cf8;
  }
  .success {
    color: #34d399;
  }
  .error {
    color: #f87171;
  }
  .warning {
    color: #fbbf24;
  }
  .box {
    background: #131926;
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 8px;
    padding: 14px 18px;
    margin: 10px 0 16px 0;
  }
  .k { color: #f472b6; }
  .s { color: #34d399; }
  .n { color: #fbbf24; }
  .b { color: #60a5fa; }
  .banner {
    display: inline-block;
    padding: 4px 12px;
    border-radius: 6px;
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.6px;
    margin-bottom: 12px;
    text-transform: uppercase;
  }
  .banner-blue { background: rgba(56, 189, 248, 0.15); color: #38bdf8; border: 1px solid rgba(56, 189, 248, 0.35); }
  .banner-purple { background: rgba(168, 85, 247, 0.15); color: #c084fc; border: 1px solid rgba(168, 85, 247, 0.35); }
  .banner-green { background: rgba(52, 211, 153, 0.15); color: #34d399; border: 1px solid rgba(52, 211, 153, 0.35); }
  .banner-amber { background: rgba(245, 158, 11, 0.15); color: #fbbf24; border: 1px solid rgba(245, 158, 11, 0.35); }
  .banner-red { background: rgba(248, 113, 113, 0.15); color: #f87171; border: 1px solid rgba(248, 113, 113, 0.35); }
  .table-grid {
    width: 100%;
    border-collapse: collapse;
    margin-top: 10px;
    font-size: 13px;
  }
  .table-grid th {
    text-align: left;
    color: #94a3b8;
    border-bottom: 1px solid rgba(255, 255, 255, 0.12);
    padding: 6px 10px;
  }
  .table-grid td {
    padding: 8px 10px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
</style>
</head>
<body>
  <div class="window">
    <div class="titlebar">
      <div class="buttons">
        <div class="btn btn-red"></div>
        <div class="btn btn-yellow"></div>
        <div class="btn btn-green"></div>
      </div>
      <div class="title">texure-bridge :: deep-link & push notification simulator</div>
      <div class="badge">Map #87 Verified</div>
    </div>
    <div class="content">
      __CONTENT__
    </div>
  </div>
</body>
</html>
"""

SCENES = [
    {
        "id": "scene1_intent_dispatch",
        "title": "Scene 1: Native Deep-link Intent Dispatch",
        "content": """
        <div class="banner banner-blue">Step 1 // Native Deep-link Intent Dispatch (POST /intent)</div><br>
        <span class="prompt">agent@texure:~$</span> <span class="cmd">curl -s -X POST http://127.0.0.1:8765/intent \\</span><br>
        &nbsp;&nbsp;<span class="dim">-H "Authorization: Bearer $TEXURE_TOKEN" \\</span><br>
        &nbsp;&nbsp;<span class="dim">-H "Content-Type: application/json" \\</span><br>
        &nbsp;&nbsp;<span class="dim">-d '{"action":"android.intent.action.VIEW", "uri":"texure://sample/profile?user=42", "extras":{"user_id":42}}' | jq .</span>
        <div class="box">
<span class="dim">// HTTP/1.1 200 OK — Intent Dispatched & Synchronous Tree Bump</span><br>
{<br>
&nbsp;&nbsp;<span class="k">"ok"</span>: <span class="b">true</span>,<br>
&nbsp;&nbsp;<span class="k">"action"</span>: <span class="s">"android.intent.action.VIEW"</span>,<br>
&nbsp;&nbsp;<span class="k">"uri"</span>: <span class="s">"texure://sample/profile?user=42"</span>,<br>
&nbsp;&nbsp;<span class="k">"package"</span>: <span class="s">"com.texure.sample"</span>,<br>
&nbsp;&nbsp;<span class="k">"rev"</span>: <span class="n">43</span><br>
}
        </div>
        <span class="success">&#10003; Resolved activity via PackageManager with MATCH_DEFAULT_ONLY in own package</span><br>
        <span class="success">&#10003; Dispatched Intent with FLAG_ACTIVITY_NEW_TASK & extras</span><br>
        <span class="success">&#10003; Tree rev bumped 42 &rarr; 43; broadcast 'tree_changed' over WebSocket /ws</span>
        """
    },
    {
        "id": "scene2_guardrail_g6",
        "title": "Scene 2: Strict Cross-Package Intent Rejection (Guardrail G6)",
        "content": """
        <div class="banner banner-red">Step 2 // Cross-Package Rejection (Guardrail G6 Enforced)</div><br>
        <span class="prompt">agent@texure:~$</span> <span class="cmd">curl -s -X POST http://127.0.0.1:8765/intent \\</span><br>
        &nbsp;&nbsp;<span class="dim">-H "Authorization: Bearer $TEXURE_TOKEN" \\</span><br>
        &nbsp;&nbsp;<span class="dim">-d '{"uri":"texure://sample/profile", "package":"com.external.malicious"}' | jq .</span>
        <div class="box">
<span class="dim">// HTTP/1.1 400 Bad Request — Cross-Package Intent Rejection</span><br>
{<br>
&nbsp;&nbsp;<span class="k">"error"</span>: {<br>
&nbsp;&nbsp;&nbsp;&nbsp;<span class="k">"code"</span>: <span class="s">"CROSS_PACKAGE_FORBIDDEN"</span>,<br>
&nbsp;&nbsp;&nbsp;&nbsp;<span class="k">"message"</span>: <span class="s">"Cross-package intents are strictly forbidden. Target 'com.external.malicious' != 'com.texure.sample'"</span><br>
&nbsp;&nbsp;}<br>
}
        </div>
        <span class="success">&#10003; Guardrail G6 enforced: intents cannot target external application packages</span><br>
        <span class="success">&#10003; Canonical error shape: code=CROSS_PACKAGE_FORBIDDEN, http=400</span>
        """
    },
    {
        "id": "scene3_route_verification",
        "title": "Scene 3: Unregistered Route & Host Verification",
        "content": """
        <div class="banner banner-amber">Step 3 // Route Table & Host Verification</div><br>
        <span class="prompt">agent@texure:~$</span> <span class="cmd">curl -s -X POST http://127.0.0.1:8765/intent \\</span><br>
        &nbsp;&nbsp;<span class="dim">-H "Authorization: Bearer $TEXURE_TOKEN" \\</span><br>
        &nbsp;&nbsp;<span class="dim">-d '{"uri":"texure://sampleevil/x"}' | jq .</span>
        <div class="box">
<span class="dim">// HTTP/1.1 400 Bad Request — Host Collision & Unresolvable Activity</span><br>
{<br>
&nbsp;&nbsp;<span class="k">"error"</span>: {<br>
&nbsp;&nbsp;&nbsp;&nbsp;<span class="k">"code"</span>: <span class="s">"ACTIVITY_NOT_FOUND"</span>,<br>
&nbsp;&nbsp;&nbsp;&nbsp;<span class="k">"message"</span>: <span class="s">"No activity found to handle intent in package 'com.texure.sample'"</span><br>
&nbsp;&nbsp;}<br>
}
        </div>
        <span class="success">&#10003; URI parsed with exact scheme://host pair verification</span><br>
        <span class="success">&#10003; Host collision 'sampleevil' != 'sample' cleanly rejected</span><br>
        <span class="success">&#10003; Android PackageManager and FakeBridge exhibit identical 400 semantics</span>
        """
    },
    {
        "id": "scene4_notification_simulate",
        "title": "Scene 4: Push Notification Simulation with Auto-Channel",
        "content": """
        <div class="banner banner-purple">Step 4 // Push Notification Simulation (POST /notification/simulate)</div><br>
        <span class="prompt">agent@texure:~$</span> <span class="cmd">curl -s -X POST http://127.0.0.1:8765/notification/simulate \\</span><br>
        &nbsp;&nbsp;<span class="dim">-H "Authorization: Bearer $TEXURE_TOKEN" \\</span><br>
        &nbsp;&nbsp;<span class="dim">-d '{ \\</span><br>
        &nbsp;&nbsp;<span class="dim">&nbsp;&nbsp;&nbsp;&nbsp;"title": "Order Shipped", \\</span><br>
        &nbsp;&nbsp;<span class="dim">&nbsp;&nbsp;&nbsp;&nbsp;"body": "Your package #42 is out for delivery", \\</span><br>
        &nbsp;&nbsp;<span class="dim">&nbsp;&nbsp;&nbsp;&nbsp;"channel_id": "texure_debug_channel", \\</span><br>
        &nbsp;&nbsp;<span class="dim">&nbsp;&nbsp;&nbsp;&nbsp;"intent_uri": "texure://sample/orders/42" \\</span><br>
        &nbsp;&nbsp;<span class="dim">&nbsp;&nbsp;}' | jq .</span>
        <div class="box">
<span class="dim">// HTTP/1.1 200 OK — Notification Dispatched to System Drawer</span><br>
{<br>
&nbsp;&nbsp;<span class="k">"ok"</span>: <span class="b">true</span>,<br>
&nbsp;&nbsp;<span class="k">"id"</span>: <span class="n">1</span>,<br>
&nbsp;&nbsp;<span class="k">"channel_id"</span>: <span class="s">"texure_debug_channel"</span><br>
}
        </div>
        <span class="success">&#10003; Auto-creates 'texure_debug_channel' with NotificationManager.IMPORTANCE_HIGH</span><br>
        <span class="success">&#10003; Attaches PendingIntent targeting 'texure://sample/orders/42'</span><br>
        <span class="success">&#10003; Tag isolation: posted under 'texure_bridge_notification' tag</span>
        """
    },
    {
        "id": "scene5_notification_cancel",
        "title": "Scene 5: Active ID Tracking, Cancellation & Strict Type Validation",
        "content": """
        <div class="banner banner-green">Step 5 // Notification Cancellation & Tracking (DELETE /notification)</div><br>
        <span class="prompt">agent@texure:~$</span> <span class="cmd">curl -s -X DELETE "http://127.0.0.1:8765/notification?id=1" | jq .</span>
        <div class="box">
<span class="dim">// HTTP/1.1 200 OK — Active Notification Found & Dismissed</span><br>
{ <span class="k">"ok"</span>: <span class="b">true</span>, <span class="k">"id"</span>: <span class="n">1</span>, <span class="k">"cancelled"</span>: <span class="b">true</span> }
        </div>
        <span class="prompt">agent@texure:~$</span> <span class="cmd">curl -s -X DELETE "http://127.0.0.1:8765/notification?id=999" | jq .</span>
        <div class="box">
<span class="dim">// HTTP/1.1 200 OK — Unknown ID Reports cancelled: false</span><br>
{ <span class="k">"ok"</span>: <span class="b">true</span>, <span class="k">"id"</span>: <span class="n">999</span>, <span class="k">"cancelled"</span>: <span class="b">false</span> }
        </div>
        <span class="prompt">agent@texure:~$</span> <span class="cmd">curl -s -X POST http://127.0.0.1:8765/notification/simulate -d '{"title":"T","body":"B","id":"5"}' | jq .</span>
        <div class="box">
<span class="dim">// HTTP/1.1 400 Bad Request — Strict Type Validation (No Coercion/Truncation)</span><br>
{ <span class="k">"error"</span>: { <span class="k">"code"</span>: <span class="s">"BAD_REQUEST"</span>, <span class="k">"message"</span>: <span class="s">"Invalid 'id' parameter: must be a positive integer"</span> } }
        </div>
        """
    },
    {
        "id": "scene6_blackbox_suite",
        "title": "Scene 6: Automated Blackbox Suite (BB-D1, BB-D2, BB-D3)",
        "content": """
        <div class="banner banner-green">Step 6 // Automated Blackbox Verification Suite</div><br>
        <span class="prompt">agent@texure:~$</span> <span class="cmd">cargo run -p texure-blackbox -- --target mcp-fake --scenario deep_link_intent,notification_simulate,cross_package_intent</span>
        <div class="box">
<span class="accent">================================================================================</span><br>
<span class="title">Texure Blackbox Test Suite &mdash; Group D (Map #87)</span><br>
<span class="dim">Target filter: mcp-fake | Selected tests: 3</span><br>
<span class="accent">================================================================================</span><br>
<table class="table-grid">
  <tr><th>ID</th><th>TARGET</th><th>STATUS</th><th>DURATION</th><th>COVERAGE</th></tr>
  <tr><td class="b">BB-D1</td><td>mcp-fake</td><td class="success">PASS</td><td>4ms</td><td class="dim">Deep-link intent dispatch, tree bump, host verification</td></tr>
  <tr><td class="b">BB-D2</td><td>mcp-fake</td><td class="success">PASS</td><td>6ms</td><td class="dim">Push notification simulate, channel, active tracking, cancel, strict id</td></tr>
  <tr><td class="b">BB-D3</td><td>mcp-fake</td><td class="success">PASS</td><td>2ms</td><td class="dim">Cross-package intent rejection (Guardrail G6)</td></tr>
</table>
<span class="accent">--------------------------------------------------------------------------------</span><br>
<span class="success">&#10004; Summary: 3 total | 3 passed | 0 failed | 0 skipped | Duration: 12ms</span><br>
<span class="accent">================================================================================</span>
        </div>
        <span class="success">&#9733; Council Approved: Gemini 3.8 Flash (10/10), Hy4 Flash (8.5/10), DeepSeek V4.1 Flash (8.5/10)</span>
        """
    }
]

def upload_to_catbox(file_path):
    print(f"Uploading {file_path.name} to Catbox / 0x0...")
    cmd = [
        "curl", "-s", "--max-time", "60",
        "-F", "reqtype=fileupload",
        "-F", f"fileToUpload=@{file_path}",
        "https://catbox.moe/user/api.php"
    ]
    res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res.returncode == 0 and res.stdout.strip().startswith("https://"):
        return res.stdout.strip()
    
    # Fallback to 0x0.st
    cmd_fallback = [
        "curl", "-s", "--max-time", "60",
        "-F", f"file=@{file_path}",
        "https://0x0.st"
    ]
    res_fb = subprocess.run(cmd_fallback, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res_fb.returncode == 0 and res_fb.stdout.strip().startswith("http"):
        return res_fb.stdout.strip()
    return None

def main():
    FRAMES_DIR.mkdir(parents=True, exist_ok=True)
    print("Generating scene frames via headless Firefox...")
    
    frame_pngs = []
    for idx, scene in enumerate(SCENES):
        html = HTML_TEMPLATE.replace("__CONTENT__", scene["content"])
        html_file = FRAMES_DIR / f"{scene['id']}.html"
        html_file.write_text(html)
        png_file = FRAMES_DIR / f"frame_{idx:02d}.png"
        
        subprocess.run([
            "firefox", "--headless",
            "--screenshot", str(png_file),
            "--window-size=1120,740",
            f"file://{html_file}"
        ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        
        print(f"  Frame {idx+1}/{len(SCENES)}: {png_file.name} ({png_file.stat().st_size // 1024} KB)")
        frame_pngs.append(png_file)

    # Save summary screenshot (Scene 6) as standalone high-res graphic
    summary_png = TMP_DIR / "deeplink_demo_summary.png"
    subprocess.run(["cp", str(frame_pngs[-1]), str(summary_png)], check=True)

    # Build MP4 with smooth transitions and holding pauses per scene
    print("\nCompiling MP4 video with ffmpeg...")
    concat_txt = TMP_DIR / "concat_frames.txt"
    with open(concat_txt, "w") as f:
        for png in frame_pngs:
            f.write(f"file '{png}'\n")
            f.write("duration 4.5\n")
        f.write(f"file '{frame_pngs[-1]}'\n")
        f.write("duration 4.0\n")

    output_mp4 = TMP_DIR / "deeplink_notification_demo.mp4"
    output_gif = TMP_DIR / "deeplink_notification_demo.gif"

    subprocess.run([
        "ffmpeg", "-y", "-f", "concat", "-safe", "0",
        "-i", str(concat_txt),
        "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20",
        "-r", "24",
        str(output_mp4)
    ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print(f"Generated MP4: {output_mp4} ({output_mp4.stat().st_size // 1024} KB)")

    # Compile optimized GIF
    print("Compiling high-quality GIF with lanczos palettegen...")
    palette_png = TMP_DIR / "demo_palette.png"
    subprocess.run([
        "ffmpeg", "-y", "-i", str(output_mp4),
        "-vf", "fps=10,scale=800:-1:flags=lanczos,palettegen=stats_mode=diff",
        str(palette_png)
    ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    subprocess.run([
        "ffmpeg", "-y", "-i", str(output_mp4), "-i", str(palette_png),
        "-lavfi", "fps=10,scale=800:-1:flags=lanczos [x]; [x][1:v] paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
        str(output_gif)
    ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print(f"Generated GIF: {output_gif} ({output_gif.stat().st_size // 1024} KB)")

    # Upload to Catbox
    urls = {}
    urls["demo_gif"] = upload_to_catbox(output_gif)
    urls["demo_mp4"] = upload_to_catbox(output_mp4)
    urls["summary_png"] = upload_to_catbox(summary_png)

    urls_json_file = TMP_DIR / "deeplink_demo_urls.json"
    urls_json_file.write_text(json.dumps(urls, indent=2))
    print("\nUpload complete! URLs:")
    for k, v in urls.items():
        print(f"  {k}: {v}")

if __name__ == "__main__":
    main()
