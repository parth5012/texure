import os
import time
import socket
import subprocess
from pathlib import Path
from playwright.sync_api import sync_playwright

def find_free_port():
    s = socket.socket()
    s.bind(('', 0))
    port = s.getsockname()[1]
    s.close()
    return port

def upload_to_catbox(file_path):
    cmd = [
        "curl", "-s",
        "-F", "reqtype=fileupload",
        "-F", f"fileToUpload=@{file_path}",
        "https://catbox.moe/user/api.php"
    ]
    res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res.returncode == 0 and res.stdout.strip().startswith("https://"):
        return res.stdout.strip()
    # Fallback to 0x0.st
    cmd_fallback = [
        "curl", "-s",
        "-F", f"file=@{file_path}",
        "https://0x0.st"
    ]
    res_fb = subprocess.run(cmd_fallback, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res_fb.returncode == 0 and res_fb.stdout.strip().startswith("http"):
        return res_fb.stdout.strip()
    return None

def record_demo_video(video_dir, port):
    video_dir.mkdir(parents=True, exist_ok=True)
    with sync_playwright() as p:
        browser = p.firefox.launch(headless=True)
        context = browser.new_context(
            viewport={"width": 1280, "height": 850},
            record_video_dir=str(video_dir),
            record_video_size={"width": 1280, "height": 850}
        )
        page = context.new_page()
        page.goto(f"http://localhost:{port}/index.html")
        page.wait_for_selector("text=Texure Companion")
        page.wait_for_timeout(1000)

        # Hide mock toolbar for pure UI walkthrough
        page.evaluate("""() => {
            const tb = document.querySelector('[data-testid="mock-toolbar"]');
            if (tb) tb.style.display = 'none';
        }""")
        page.wait_for_timeout(600)

        # 1. Dashboard (Dark Mode)
        page.click("text=Device Status")
        page.wait_for_timeout(800)
        page.click("[data-testid='dashboard-refresh-btn']")
        page.wait_for_timeout(1000)

        # 2. Controls (Dark Mode)
        page.click("text=Manual Controls")
        page.wait_for_timeout(800)
        page.click("[data-testid='btn-wake-screen']")
        page.wait_for_timeout(1200)
        page.click("[data-testid='btn-dismiss-keyguard']")
        page.wait_for_timeout(1200)

        # 3. QR Pairing (Dark Mode)
        page.click("text=QR Pairing")
        page.wait_for_timeout(1000)
        if page.locator("[data-testid='load-fixture-qr-btn']").count() > 0:
            page.click("[data-testid='load-fixture-qr-btn']")
            page.wait_for_timeout(1200)

        # 4. Logs (Dark Mode)
        page.click("text=Live Logs")
        page.wait_for_timeout(800)
        page.fill("[data-testid='input-log-search']", "tunnel")
        page.wait_for_timeout(1000)
        page.fill("[data-testid='input-log-search']", "")
        page.wait_for_timeout(800)

        # 5. Toggle to Light Mode!
        page.click("button[title='Toggle theme']")
        page.wait_for_timeout(1000)

        # 6. Dashboard (Light Mode)
        page.click("text=Device Status")
        page.wait_for_timeout(1000)

        # 7. Controls (Light Mode)
        page.click("text=Manual Controls")
        page.wait_for_timeout(1000)

        # 8. QR Pairing (Light Mode)
        page.click("text=QR Pairing")
        page.wait_for_timeout(1000)

        # 9. Logs (Light Mode)
        page.click("text=Live Logs")
        page.wait_for_timeout(1000)

        # Settle
        page.wait_for_timeout(800)
        page.close()
        context.close()
        browser.close()

def main():
    repo_root = Path(__file__).resolve().parent.parent
    companion_dir = repo_root / "companion"
    dist_dir = companion_dir / "dist"
    tmp_dir = repo_root / "tmp"
    video_dir = tmp_dir / "demo_raw_video"
    screenshots_dir = tmp_dir / "screenshots"

    port = find_free_port()
    server_process = subprocess.Popen(
        ["python3", "-m", "http.server", str(port), "--directory", str(dist_dir)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL
    )
    time.sleep(1)

    try:
        print("Recording interactive demo video...")
        record_demo_video(video_dir, port)
    finally:
        server_process.terminate()
        server_process.wait()

    # Find the recorded video file
    video_files = list(video_dir.glob("*.webm"))
    if not video_files:
        print("No video recorded!")
        return

    raw_video = video_files[0]
    output_gif = tmp_dir / "companion_demo.gif"
    output_mp4 = tmp_dir / "companion_demo.mp4"

    print(f"Converting {raw_video} to optimized GIF and MP4...")
    # Convert to MP4
    subprocess.run([
        "ffmpeg", "-y", "-i", str(raw_video),
        "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "22",
        str(output_mp4)
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    # Convert to GIF with palettegen for high quality
    palette_png = tmp_dir / "palette.png"
    subprocess.run([
        "ffmpeg", "-y", "-i", str(output_mp4),
        "-vf", "fps=12,scale=800:-1:flags=lanczos,palettegen=stats_mode=diff",
        str(palette_png)
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    subprocess.run([
        "ffmpeg", "-y", "-i", str(output_mp4), "-i", str(palette_png),
        "-lavfi", "fps=12,scale=800:-1:flags=lanczos [x]; [x][1:v] paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
        str(output_gif)
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    print(f"GIF size: {output_gif.stat().st_size / 1024:.1f} KB")

    print("\nUploading assets to public host...")
    urls = {}
    
    # Upload GIF
    if output_gif.exists():
        url = upload_to_catbox(output_gif)
        print(f"Demo GIF: {url}")
        urls["demo_gif"] = url

    # Upload MP4
    if output_mp4.exists():
        url = upload_to_catbox(output_mp4)
        print(f"Demo MP4: {url}")
        urls["demo_mp4"] = url

    # Upload Screenshots
    for name in [
        "dashboard_dark", "dashboard_light",
        "controls_dark", "controls_light",
        "scanner_dark", "scanner_light",
        "logs_dark", "logs_light"
    ]:
        png_path = screenshots_dir / f"{name}.png"
        if png_path.exists():
            url = upload_to_catbox(png_path)
            print(f"{name}: {url}")
            urls[name] = url

    # Save URLs mapping to JSON
    import json
    urls_file = tmp_dir / "demo_urls.json"
    with open(urls_file, "w") as f:
        json.dump(urls, f, indent=2)
    print(f"\nSaved URLs to {urls_file}")

if __name__ == "__main__":
    main()
