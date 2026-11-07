#!/usr/bin/env python3
"""Standalone test MCP client for texure-server.

Spawns the real ``texure-server`` binary over stdio JSON-RPC, drives it
against an in-process FakeBridge (stdlib HTTP, Auth + rev-guard + PNG
screenshot + HTTP polling fallback for wait_for), and records a per-tool
availability / latency / correctness report.

Usage:
    python3 tools/test_mcp_client.py [--server target/debug/texure-server]
                                     [--out tmp/mcp-probe]
                                     [--no-build]
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import queue
import socket
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse, parse_qs

# --------------------------------------------------------------------------
# FakeBridge (stdlib-only): mirrors crates/texure-blackbox/src/fake_bridge.rs
# --------------------------------------------------------------------------

INITIAL_REV = 42
TOKEN = "test-token"
UI_TREE = {
    "window": "com.velavoice/.MainActivity",
    "nodes": [
        {"id": "btn_send", "text": "Send", "cls": "android.widget.Button",
         "bounds": [12, 340, 108, 56], "clickable": True, "editable": False},
        {"id": "input_message", "text": "Hello world", "cls": "android.widget.EditText",
         "bounds": [12, 280, 300, 48], "clickable": True, "editable": True},
    ],
}
# 1x1 transparent PNG; scale only affects length for the probe (0.25/0.5/1.0)
_PNG_1PX = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
)


def _png_bytes(scale: str) -> bytes:
    mult = {"0.25": 1, "0.5": 2, "1.0": 4}.get(scale, 2)
    return _PNG_1PX + b"\x00" * (mult * 8)


class FakeBridgeState:
    def __init__(self, expected_token: str = TOKEN):
        self.rev = INITIAL_REV
        self.expected_token = expected_token
        self.lock = threading.Lock()


def _err(code: str, message: str, hint: str) -> bytes:
    return json.dumps({"error": {"code": code, "message": message, "hint": hint}}).encode()


def make_handler(state: FakeBridgeState):
    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, fmt, *args):  # silence
            pass

        def _send(self, status: int, body: bytes, ctype: str = "application/json"):
            self.send_response(status)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(body)
            self.close_connection = True

        def _authed(self) -> bool:
            return self.headers.get("Authorization") == f"Bearer {state.expected_token}"

        def _auth_fail(self):
            self._send(401, _err("AUTH_FAILED", "token mismatch", "re-paste fresh token from overlay"))

        def _read_body(self) -> bytes:
            n = int(self.headers.get("Content-Length") or 0)
            return self.rfile.read(n) if n else b""

        def do_GET(self):
            u = urlparse(self.path)
            if u.path == "/ws":
                self._send(426, _err("UPGRADE_REQUIRED", "ws not implemented in probe fakebridge", "use HTTP polling fallback"))
                return
            if not self._authed():
                return self._auth_fail()
            if u.path == "/ui-tree":
                with state.lock:
                    rev = state.rev
                body = json.dumps({"rev": rev, **UI_TREE}).encode()
                return self._send(200, body)
            if u.path == "/screenshot":
                scale = parse_qs(u.query).get("scale", ["0.5"])[0]
                return self._send(200, _png_bytes(scale), ctype="image/png")
            self._send(404, _err("NOT_FOUND", f"Route not found: GET {u.path}", ""))

        def do_POST(self):
            u = urlparse(self.path)
            body = self._read_body()
            try:
                payload = json.loads(body) if body else {}
            except json.JSONDecodeError:
                payload = {}
            if not self._authed():
                return self._auth_fail()
            if u.path in ("/tap", "/type"):
                rev = payload.get("rev", 0)
                with state.lock:
                    if rev != state.rev:
                        return self._send(400, _err(
                            "NODE_OUTDATED",
                            "tree refreshed since reference; re-run /ui-tree",
                            "re-run device_tree, re-target",
                        ))
                    state.rev += 1
                return self._send(200, json.dumps({"ok": True}).encode())
            if u.path in ("/keypress", "/back"):
                return self._send(200, json.dumps({"ok": True}).encode())
            self._send(404, _err("NOT_FOUND", f"Route not found: POST {u.path}", ""))

    return Handler


def free_port() -> int:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


class FakeBridge:
    def __init__(self, expected_token: str = TOKEN):
        self.state = FakeBridgeState(expected_token)
        self.port = free_port()
        self.server = ThreadingHTTPServer(("127.0.0.1", self.port), make_handler(self.state))
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *a):
        self.server.shutdown()
        self.server.server_close()

    @property
    def url(self) -> str:
        return f"http://127.0.0.1:{self.port}"


# --------------------------------------------------------------------------
# MCP stdio client (newline-delimited JSON-RPC 2.0)
# --------------------------------------------------------------------------

class McpError(RuntimeError):
    def __init__(self, code: int, message: str):
        super().__init__(f"JSON-RPC {code}: {message}")
        self.code = code
        self.message = message


class McpClient:
    def __init__(self, argv: list[str], env: dict | None = None):
        full_env = dict(os.environ)
        for k in ("TEXURE_URL", "TEXURE_TOKEN", "TEXURE_TIMEOUT", "TEXURE_CONFIG"):
            full_env.pop(k, None)
        if env:
            full_env.update(env)
        self.proc = subprocess.Popen(
            argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True, bufsize=1, env=full_env,
        )
        self.next_id = 1
        self._q: queue.Queue[str] = queue.Queue()
        self._stderr_lines: list[str] = []
        threading.Thread(target=self._reader, daemon=True).start()
        threading.Thread(target=self._stderr_reader, daemon=True).start()

    def _reader(self):
        assert self.proc.stdout
        for line in self.proc.stdout:
            self._q.put(line)

    def _stderr_reader(self):
        assert self.proc.stderr
        for line in self.proc.stderr:
            self._stderr_lines.append(line.rstrip())

    def close(self):
        try:
            if self.proc.stdin:
                self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(timeout=3)

    def _send_raw(self, obj: dict):
        assert self.proc.stdin
        self.proc.stdin.write(json.dumps(obj) + "\n")
        self.proc.stdin.flush()

    def send_raw_line(self, line: str):
        assert self.proc.stdin
        self.proc.stdin.write(line + "\n")
        self.proc.stdin.flush()

    def request(self, method: str, params: dict | None = None, timeout: float = 8.0) -> dict:
        rid = self.next_id
        self.next_id += 1
        self._send_raw({"jsonrpc": "2.0", "id": rid, "method": method, "params": params or {}})
        deadline = time.time() + timeout
        while True:
            remaining = deadline - time.time()
            if remaining <= 0:
                raise TimeoutError(f"no response to {method} id={rid} within {timeout}s")
            try:
                line = self._q.get(timeout=remaining)
            except queue.Empty:
                raise TimeoutError(f"no response to {method} id={rid}") from None
            if not line.strip():
                continue
            resp = json.loads(line)
            if resp.get("id") != rid:
                continue
            if "error" in resp:
                e = resp["error"]
                raise McpError(e.get("code", -1), e.get("message", ""))
            return resp.get("result", {})

    def notify(self, method: str, params: dict | None = None):
        self._send_raw({"jsonrpc": "2.0", "method": method, "params": params or {}})

    def initialize(self) -> dict:
        return self.request("initialize", {
            "protocolVersion": "2024-11-05", "capabilities": {},
            "clientInfo": {"name": "test-mcp-client", "version": "1.0"},
        })

    def call_tool(self, name: str, arguments: dict | None = None, timeout: float = 15.0) -> dict:
        return self.request("tools/call", {"name": name, "arguments": arguments or {}}, timeout=timeout)


def tool_text(result: dict) -> str:
    for c in result.get("content", []):
        if c.get("type") == "text":
            return c.get("text", "")
    return ""


def excerpt(text: str, limit: int = 220) -> str:
    return text if len(text) <= limit else text[:limit] + f"…({len(text)} chars)"


# --------------------------------------------------------------------------
# Probe driver
# --------------------------------------------------------------------------

class Report:
    def __init__(self):
        self.checks: list[dict] = []

    def check(self, name: str, tool: str, fn, expect_substr: str | None = None):
        t0 = time.monotonic()
        entry: dict = {"name": name, "tool": tool, "status": "FAIL", "latency_ms": 0, "detail": ""}
        try:
            detail = fn()
            entry["detail"] = excerpt(str(detail))
            entry["status"] = "PASS"
        except Exception as e:  # noqa: BLE001
            entry["detail"] = excerpt(f"{type(e).__name__}: {e}")
            entry["status"] = "FAIL"
        entry["latency_ms"] = round((time.monotonic() - t0) * 1000, 1)
        self.checks.append(entry)
        mark = "✅" if entry["status"] == "PASS" else "❌"
        print(f"{mark} [{entry['latency_ms']:7.1f} ms] {tool:12} {name}: {entry['detail']}")
        return entry["status"] == "PASS"

    def summary(self) -> dict:
        passed = sum(1 for c in self.checks if c["status"] == "PASS")
        return {"passed": passed, "failed": len(self.checks) - passed, "total": len(self.checks)}


def write_config(path: Path, url: str, token: str):
    path.write_text(f'url = "{url}"\ntoken = "{token}"\ntimeout_secs = 10\n')


def run_happy_session(server_argv: list[str], report: Report) -> int:
    """Main client session against FakeBridge with correct token."""
    with FakeBridge() as bridge, tempfile.TemporaryDirectory() as td:
        cfg = Path(td) / "config.toml"
        write_config(cfg, bridge.url, TOKEN)
        client = McpClient([*server_argv, "--config", str(cfg)])
        try:
            tree_rev = None

            def do_initialize():
                r = client.initialize()
                assert r.get("protocolVersion") == "2024-11-05", r
                assert list(r.get("capabilities", {}).keys()) == ["tools"], r
                assert r["serverInfo"]["name"] == "texure-server", r
                client.notify("notifications/initialized")
                return f"proto={r['protocolVersion']} server={r['serverInfo']}"

            report.check("handshake initialize", "protocol", do_initialize)

            def do_ping():
                r = client.request("ping")
                assert isinstance(r, dict), r
                return "ping ok"

            report.check("ping", "protocol", do_ping)

            def do_tools_list():
                r = client.request("tools/list")
                names = [t["name"] for t in r.get("tools", [])]
                assert names == ["device_tree", "tap", "type", "keypress", "screenshot", "wait_for"], names
                return ", ".join(names)

            report.check("exactly 6 frozen tools", "protocol", do_tools_list)

            def do_tree():
                nonlocal tree_rev
                r = client.call_tool("device_tree", {"max_depth": 12})
                assert r.get("isError") is not True, tool_text(r)
                tree = json.loads(tool_text(r))
                assert isinstance(tree.get("rev"), int) and tree["rev"] > 0, tree
                assert any(n.get("clickable") for n in tree["nodes"]), tree
                tree_rev = tree["rev"]
                return f"rev={tree['rev']} nodes={len(tree['nodes'])} window={tree['window']}"

            report.check("fetch UI tree", "device_tree", do_tree)

            def need_rev():
                assert tree_rev is not None, "device_tree must succeed first"
                return tree_rev

            def do_tap():
                r = client.call_tool("tap", {"node": 1, "rev": need_rev()})
                assert r.get("isError") is not True, tool_text(r)
                d = json.loads(tool_text(r))
                assert d.get("ok") is True, d
                return tool_text(r)

            report.check("tap node with fresh rev", "tap", do_tap)

            def do_type():
                # re-fetch rev (tap bumped it)
                r = client.call_tool("device_tree", {})
                rev = json.loads(tool_text(r))["rev"]
                r2 = client.call_tool("type", {"node": 2, "text": "probe", "rev": rev})
                assert r2.get("isError") is not True, tool_text(r2)
                assert json.loads(tool_text(r2)).get("ok") is True, tool_text(r2)
                return tool_text(r2)

            report.check("type into editable", "type", do_type)

            def do_keypress():
                r = client.call_tool("keypress", {"key": "ENTER"})
                assert r.get("isError") is not True, tool_text(r)
                assert json.loads(tool_text(r)).get("ok") is True, tool_text(r)
                return tool_text(r)

            report.check("keypress ENTER", "keypress", do_keypress)

            def do_back():
                # BACK rides the keypress tool in this server (bridge /back under
                # the hood is exercised by the blackbox runner); the probe sends
                # keypress BACK to validate the key enum path.
                r = client.call_tool("keypress", {"key": "BACK"})
                assert r.get("isError") is not True, tool_text(r)
                return tool_text(r)

            report.check("keypress BACK", "keypress", do_back)

            def do_screenshot():
                r = client.call_tool("screenshot", {"scale": 0.5})
                assert r.get("isError") is not True, tool_text(r)
                img = next(c for c in r["content"] if c.get("type") == "image")
                assert img.get("mimeType") == "image/png", img
                raw = base64.b64decode(img["data"])
                assert raw[:8] == b"\x89PNG\r\n\x1a\n", raw[:8]
                return f"png {len(raw)} bytes, magic ok"

            report.check("screenshot PNG decode", "screenshot", do_screenshot)

            def do_wait_hit():
                r = client.call_tool("wait_for", {"where": "text", "value": "Send", "timeout": 2})
                assert r.get("isError") is not True, tool_text(r)
                d = json.loads(tool_text(r))
                assert d.get("ok") is True, d
                return tool_text(r)

            report.check("wait_for immediate hit", "wait_for", do_wait_hit)

            def do_wait_timeout():
                r = client.call_tool("wait_for", {"where": "text", "value": "NopeProbe", "timeout": 1},
                                     timeout=10)
                assert r.get("isError") is not True, tool_text(r)  # timeout is non-error per LLD
                d = json.loads(tool_text(r))
                assert d.get("ok") is False and "last_rev" in d, d
                return tool_text(r)

            report.check("wait_for timeout -> ok:false", "wait_for", do_wait_timeout)

            def do_stale():
                r = client.call_tool("tap", {"node": 1, "rev": 0})
                assert r.get("isError") is True, r
                assert tool_text(r).startswith("NODE_OUTDATED:"), tool_text(r)
                return tool_text(r)

            report.check("stale rev -> NODE_OUTDATED", "tap", do_stale)

            def do_unknown():
                try:
                    client.call_tool("nope", {})
                except McpError as e:
                    assert e.code == -32602, e
                    assert "Unknown tool" in e.message, e.message
                    return e.message
                raise AssertionError("expected -32602 unknown tool")

            report.check("unknown tool -> -32602", "protocol", do_unknown)

            def do_alive_after_errors():
                r = client.call_tool("device_tree", {})
                assert r.get("isError") is not True, tool_text(r)
                return "daemon still serving"

            report.check("daemon survives tool errors", "device_tree", do_alive_after_errors)

            return 0
        finally:
            client.close()


def run_negative_sessions(server_argv: list[str], report: Report):
    # NO_CONFIG: missing file + no env
    with tempfile.TemporaryDirectory() as td:
        missing = str(Path(td) / "absent.toml")
        client = McpClient([*server_argv, "--config", missing])
        try:
            def do_no_config():
                r = client.call_tool("device_tree", {})
                assert r.get("isError") is True, r
                assert tool_text(r).startswith("NO_CONFIG:"), tool_text(r)
                return tool_text(r)
            report.check("missing config -> NO_CONFIG", "device_tree", do_no_config)
        finally:
            client.close()

    # OFFLINE: dead port
    with tempfile.TemporaryDirectory() as td:
        cfg = Path(td) / "config.toml"
        write_config(cfg, f"http://127.0.0.1:{free_port()}", TOKEN)
        client = McpClient([*server_argv, "--config", str(cfg)])
        try:
            def do_offline():
                r = client.call_tool("device_tree", {})
                assert r.get("isError") is True, r
                assert tool_text(r).startswith("OFFLINE:"), tool_text(r)
                return tool_text(r)
            report.check("dead port -> OFFLINE", "device_tree", do_offline)
        finally:
            client.close()

    # AUTH_FAILED + verbose payload shape
    with FakeBridge() as bridge, tempfile.TemporaryDirectory() as td:
        cfg = Path(td) / "config.toml"
        write_config(cfg, bridge.url, "wrong-token")
        client = McpClient([*server_argv, "--config", str(cfg)])
        try:
            def do_auth():
                r = client.call_tool("device_tree", {})
                assert r.get("isError") is True, r
                assert tool_text(r).startswith("AUTH_FAILED:"), tool_text(r)
                return tool_text(r)
            report.check("wrong token -> AUTH_FAILED", "device_tree", do_auth)

            def do_verbose():
                r = client.call_tool("device_tree", {"verbose": True})
                assert r.get("isError") is True, r
                text = tool_text(r)
                payload = json.loads(text)  # verbose => full JSON, not one-liner
                assert payload["error"]["code"] == "AUTH_FAILED", payload
                assert "bridge_url" in payload and "ws_connected" in payload, payload
                return excerpt(text, 160)
            report.check("verbose:true full JSON payload", "device_tree", do_verbose)
        finally:
            client.close()

    # malformed JSON line -> -32700
    with tempfile.TemporaryDirectory() as td:
        client = McpClient([*server_argv, "--config", str(Path(td) / "absent.toml")])
        try:
            def do_parse_error():
                client.send_raw_line("{not json")
                deadline = time.time() + 5
                while time.time() < deadline:
                    line = client._q.get(timeout=max(0.1, deadline - time.time()))
                    if not line.strip():
                        continue
                    resp = json.loads(line)
                    assert resp.get("id") is None, resp
                    assert resp.get("error", {}).get("code") == -32700, resp
                    return "parse error -32700"
                raise AssertionError("no parse-error response")
            report.check("malformed JSON -> -32700", "protocol", do_parse_error)
        finally:
            client.close()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--server", default="target/debug/texure-server")
    ap.add_argument("--out", default="tmp/mcp-probe")
    ap.add_argument("--no-build", action="store_true")
    args = ap.parse_args()

    server = Path(args.server)
    if not args.no_build and not server.exists():
        print(f"Building texure-server… (binary missing at {server})")
        subprocess.run(["cargo", "build", "-p", "texure-server"], check=True)
    if not server.exists():
        print(f"ERROR: server binary not found: {server}", file=sys.stderr)
        return 2

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    print("=" * 72)
    print("texure test MCP client — texure-server availability probe")
    print(f"server: {server.resolve()}")
    print("=" * 72)

    report = Report()
    t0 = time.monotonic()
    run_happy_session([str(server.resolve())], report)
    run_negative_sessions([str(server.resolve())], report)
    wall = time.monotonic() - t0
    s = report.summary()

    (out / "report.json").write_text(json.dumps({"summary": s, "checks": report.checks,
                                                 "wall_seconds": round(wall, 2)}, indent=2))
    md_lines = ["# texure MCP probe report", "", f"Passed {s['passed']}/{s['total']} in {wall:.1f}s", "",
                "| check | tool | latency | detail |", "|---|---|---|---|"]
    for c in report.checks:
        detail = c["detail"].replace("|", "\\|")
        md_lines.append(f"| {c['name']} | {c['tool']} | {c['latency_ms']}ms | {detail} |")
    (out / "report.md").write_text("\n".join(md_lines) + "\n")

    print(f"\nSummary: {s['passed']} passed, {s['failed']} failed, {s['total']} total, {wall:.1f}s wall")
    print(f"Wrote {out / 'report.json'} and {out / 'report.md'}")
    return 0 if s["failed"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
