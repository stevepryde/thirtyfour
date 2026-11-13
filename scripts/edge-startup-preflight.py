#!/usr/bin/env python3
"""Diagnostic-only external Edge session warm-up; never removes browser files."""
import json
import pathlib
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request


def request(base, method, path, data=None):
    body = None if data is None else json.dumps(data).encode()
    req = urllib.request.Request(base + path, data=body, method=method,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=125) as response:
            return response.status, json.load(response)
    except urllib.error.HTTPError as exc:
        return exc.code, json.load(exc)


def main():
    driver, output = sys.argv[1:]
    output = pathlib.Path(output)
    output.mkdir(parents=True, exist_ok=True)
    outcomes = []
    for attempt in range(1, 4):
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        base = f"http://127.0.0.1:{port}"
        started = time.monotonic()
        with (output / f"warmup-{attempt}.log").open("w") as log:
            process = subprocess.Popen([driver, f"--port={port}", "--verbose"],
                                       stdout=log, stderr=subprocess.STDOUT)
            outcome = {"attempt": attempt, "session_created": False}
            try:
                for _ in range(100):
                    try:
                        if request(base, "GET", "/status")[0] == 200:
                            break
                    except (OSError, ValueError):
                        pass
                    time.sleep(.1)
                else:
                    raise RuntimeError("EdgeDriver did not become ready in 10 seconds")
                status, response = request(base, "POST", "/session", {
                    "capabilities": {"alwaysMatch": {"browserName": "MicrosoftEdge",
                        "ms:edgeOptions": {"args": ["--headless=new", "--no-sandbox",
                            "--disable-gpu", "--disable-dev-shm-usage"]}}}})
                outcome.update(http_status=status, response=response)
                session = response.get("value", {}).get("sessionId")
                if status == 200 and session:
                    outcome["session_created"] = True
                    close_status, close_response = request(base, "DELETE", f"/session/{session}")
                    outcome.update(close_status=close_status, close_response=close_response)
            except Exception as exc:
                outcome["error"] = repr(exc)
            finally:
                try:
                    request(base, "GET", "/shutdown")
                except Exception:
                    pass
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    process.wait(timeout=5)
                outcome["seconds"] = round(time.monotonic() - started, 3)
                outcomes.append(outcome)
                print(json.dumps(outcome), flush=True)
                (output / "warmup.json").write_text(json.dumps(outcomes, indent=2))
            if outcome.get("close_status") == 200:
                return 0
        time.sleep(2)
    return 1


if __name__ == "__main__":
    sys.exit(main())
