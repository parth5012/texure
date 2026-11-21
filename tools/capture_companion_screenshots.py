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

def capture_screenshots(output_dir=None):
    repo_root = Path(__file__).resolve().parent.parent
    if output_dir:
        out_path = Path(output_dir) if Path(output_dir).is_absolute() else repo_root / output_dir
    else:
        out_path = repo_root / "tmp" / "screenshots"
    out_path.mkdir(parents=True, exist_ok=True)
    
    port = find_free_port()
    companion_dir = Path(__file__).resolve().parent.parent / "companion"
    dist_dir = companion_dir / "dist"
    
    server_process = subprocess.Popen(
        ["python3", "-m", "http.server", str(port), "--directory", str(dist_dir)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL
    )
    time.sleep(1)
    
    try:
        with sync_playwright() as p:
            browser = p.firefox.launch(headless=True)
            context = browser.new_context(
                viewport={"width": 1280, "height": 850},
                reduced_motion="reduce"
            )
            page = context.new_page()
            
            url = f"http://localhost:{port}/index.html"
            page.goto(url)
            page.wait_for_selector("text=Texure Companion")
            time.sleep(1)
            
            # Hide mock toolbar for pure production product screenshot capture
            page.evaluate("""() => {
                const tb = document.querySelector('[data-testid="mock-toolbar"]');
                if (tb) tb.style.display = 'none';
            }""")
            
            tabs = [
                ("dashboard", "text=Device Status"),
                ("controls", "text=Manual Controls"),
                ("scanner", "text=QR Pairing"),
                ("logs", "text=Live Logs")
            ]
            
            # Dark mode screenshots
            for name, tab_selector in tabs:
                page.click(tab_selector)
                page.wait_for_timeout(400)
                page.evaluate("() => Promise.all(document.getAnimations().map(a => a.finished))")
                page.wait_for_timeout(200)
                shot_path = out_path / f"{name}_dark.png"
                page.screenshot(path=str(shot_path), full_page=False)
                print(f"Captured {shot_path}")
                
            # Toggle to Light mode
            page.click("button[title='Toggle theme']")
            page.wait_for_timeout(400)
            
            # Light mode screenshots
            for name, tab_selector in tabs:
                page.click(tab_selector)
                page.wait_for_timeout(400)
                page.evaluate("() => Promise.all(document.getAnimations().map(a => a.finished))")
                page.wait_for_timeout(200)
                shot_path = out_path / f"{name}_light.png"
                page.screenshot(path=str(shot_path), full_page=False)
                print(f"Captured {shot_path}")
                
            browser.close()
    finally:
        server_process.terminate()
        server_process.wait()

if __name__ == "__main__":
    capture_screenshots()
