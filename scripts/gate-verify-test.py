#!/usr/bin/env python3
"""Exercises `scripts/gate-verify.py`: builds a config, runs the gate against a
stub upstream, and asserts the reader accepts the mirror and rejects tampering.

This is the M10 test: the reader must recompute (a) the expanded order, (b) the
attempt chain, (c) the egress host and (d) the absence of a credential.
"""
from __future__ import annotations

import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GATE = ROOT / "target" / "debug" / ("minicon-gate.exe" if os.name == "nt" else "minicon-gate")
READER = ROOT / "scripts" / "gate-verify.py"

RESULTS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    RESULTS.append((name, ok, detail))


class Upstream(BaseHTTPRequestHandler):
    """Answers 429 to the first call and 200 afterwards, so a chain is visible."""

    calls = 0

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        length = int(self.headers.get("content-length", "0"))
        self.rfile.read(length)
        Upstream.calls += 1
        if Upstream.calls == 1:
            self.send_response(429)
            body = b'{"error":{"message":"slow down"}}'
        else:
            self.send_response(200)
            body = b'{"choices":[{"message":{"role":"assistant","content":"ok"}}]}'
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args) -> None:
        pass


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def main() -> int:
    if not GATE.exists():
        print(f"gate-verify-test: build the binary first ({GATE})", file=sys.stderr)
        return 2
    work = Path(tempfile.mkdtemp(prefix="gate-verify-"))
    upstream = ThreadingHTTPServer(("127.0.0.1", 0), Upstream)
    threading.Thread(target=upstream.serve_forever, daemon=True).start()
    upstream_port = upstream.server_address[1]
    gate_port = free_port()
    mirror = work / "events.jsonl"
    config = {
        "listen": {"bind": f"127.0.0.1:{gate_port}"},
        "events": {"ring": 64, "mirror": str(mirror).replace("\\", "/")},
        "endpoints": {
            "up": {
                "provider": "stub",
                "protocol": "openai-chat",
                "baseURL": f"http://127.0.0.1:{upstream_port}/v1",
            }
        },
        "credentials": {
            "one": {"label": "one", "secret": {"env": "GATE_VERIFY_KEY_ONE"}},
            "two": {"label": "two", "secret": {"env": "GATE_VERIFY_KEY_TWO"}},
        },
        "egress": {"direct": {"proxy": "direct"}},
        "lanes": {
            "lane": {
                "endpoint": "up",
                "model": "m",
                "credentialOrder": ["one", "two"],
                "egressOrder": ["direct"],
            }
        },
        "agents": {"a": {"lanes": ["lane"], "budget": {"maxAttempts": 3}}},
    }
    config_path = work / "gate.jsonc"
    config_path.write_text(json.dumps(config, indent=2), encoding="utf-8")

    env = dict(os.environ, GATE_VERIFY_KEY_ONE="sk-verify-one", GATE_VERIFY_KEY_TWO="sk-verify-two")
    gate = subprocess.Popen([str(GATE), "--config", str(config_path)], env=env,
                            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    try:
        for _ in range(100):
            try:
                with socket.create_connection(("127.0.0.1", gate_port), timeout=0.2):
                    break
            except OSError:
                time.sleep(0.05)
        else:
            print("gate-verify-test: the gate did not start", file=sys.stderr)
            return 2

        import urllib.request

        request = urllib.request.Request(
            f"http://127.0.0.1:{gate_port}/v1/chat/completions",
            data=json.dumps({"model": "gate/a", "messages": []}).encode(),
            headers={"content-type": "application/json"},
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            response.read()
    finally:
        gate.terminate()
        try:
            gate.wait(timeout=10)
        except subprocess.TimeoutExpired:
            gate.kill()
        upstream.shutdown()

    rows = [json.loads(line) for line in mirror.read_text(encoding="utf-8").splitlines() if line.strip()]
    check("the mirror has at least two attempts", len(rows) >= 2, f"rows={len(rows)}")
    check("the chain records triggered_by", any(r.get("triggered_by") for r in rows), "")
    joined = mirror.read_text(encoding="utf-8")
    check("no key in the mirror", "sk-verify-one" not in joined and "sk-verify-two" not in joined, "")

    def run_reader(cfg: Path, mir: Path) -> subprocess.CompletedProcess:
        return subprocess.run([sys.executable, str(READER), str(cfg), str(mir)],
                              capture_output=True, text=True)

    good = run_reader(config_path, mirror)
    check("the reader accepts the real mirror", good.returncode == 0, (good.stdout + good.stderr).strip()[:200])

    # Tampering: an attempt that claims an egress the config does not have.
    tampered = work / "tampered.jsonl"
    altered = [dict(row) for row in rows]
    altered[0]["egress_host"] = "10.9.9.9:1"
    tampered.write_text("\n".join(json.dumps(row) for row in altered) + "\n", encoding="utf-8")
    bad = run_reader(config_path, tampered)
    check("the reader rejects a wrong egress host", bad.returncode == 1, bad.stderr.strip()[:160])

    # Tampering: a credential-shaped value in a row.
    leaked = work / "leaked.jsonl"
    altered = [dict(row) for row in rows]
    altered[0]["error_class"] = "sk-abcdefghijklmnop"
    leaked.write_text("\n".join(json.dumps(row) for row in altered) + "\n", encoding="utf-8")
    leak = run_reader(config_path, leaked)
    check("the reader rejects a credential-shaped value", leak.returncode == 1, leak.stderr.strip()[:160])

    # Tampering: an attempt on a candidate the agent cannot reach.
    unreachable = work / "unreachable.jsonl"
    altered = [dict(row) for row in rows]
    altered[0]["credential_id"] = "not-a-credential"
    unreachable.write_text("\n".join(json.dumps(row) for row in altered) + "\n", encoding="utf-8")
    off = run_reader(config_path, unreachable)
    check("the reader rejects an unreachable candidate", off.returncode == 1, off.stderr.strip()[:160])

    shutil.rmtree(work, ignore_errors=True)

    ok = sum(1 for _, passed, _ in RESULTS if passed)
    for name, passed, detail in RESULTS:
        print(f"{'PASS' if passed else 'FAIL'}  {name}" + (f"  | {detail}" if detail else ""))
    print(f"\n{ok}/{len(RESULTS)} passed")
    return 0 if ok == len(RESULTS) else 1


if __name__ == "__main__":
    raise SystemExit(main())
