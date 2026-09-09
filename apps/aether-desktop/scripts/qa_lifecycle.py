"""Exercise the real gateway with disposable SQLite, HTTP/SSE and OS signals.

Usage: python3 qa_lifecycle.py [--desktop] /absolute/aether-gateway /absolute/frontend/dist
Artifacts and synthetic test keys stay in the printed temporary directory.
"""
import json
import os
from pathlib import Path
import signal
import socket
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse
import uuid

from qa_api import MockUpstream, Session, ThreadingHTTPServer, save


def available(port):
    with socket.socket() as connection:
        connection.settimeout(0.2)
        return connection.connect_ex(("127.0.0.1", port)) == 0


def wait_for(check, seconds=15):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if check():
            return
        time.sleep(0.05)
    raise AssertionError("Timed out waiting for lifecycle condition")


def main():
    os.umask(0o077)
    if sys.argv[1] == "--owner":
        command = json.loads(os.environ["AETHER_QA_COMMAND"])
        with open(os.environ["AETHER_QA_LOG"], "ab") as log:
            child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=log, stderr=log)
            print(child.pid, flush=True)
            while True:
                time.sleep(60)
    arguments = sys.argv[1:]
    desktop = arguments[0] == "--desktop"
    if desktop:
        arguments = arguments[1:]
    gateway, web = map(lambda value: str(Path(value).resolve()), arguments)
    root = Path(tempfile.mkdtemp(prefix="aether-cli-lifecycle-")).resolve()
    data = root / "CLI 数据#1"
    data.mkdir()
    logs = data / "logs"
    logs.mkdir()
    server = ThreadingHTTPServer(("127.0.0.1", 0), MockUpstream)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    with socket.socket() as reserved:
        reserved.bind(("127.0.0.1", 0)); port = reserved.getsockname()[1]
    state = {"root": str(root), "base": f"http://127.0.0.1:{port}", "upstream_url": f"http://127.0.0.1:{server.server_port}"}
    statefile = root / "qa.json"
    save(statefile, state)
    env = {name: os.environ[name] for name in ("HOME", "PATH", "TMPDIR", "LANG") if name in os.environ}
    env.update(ENVIRONMENT="production", AETHER_DATABASE_DRIVER="sqlite", AETHER_DATABASE_URL="sqlite://" + urllib.parse.urlsplit((data / "aether.db").as_uri()).path,
               AETHER_RUNTIME_BACKEND="memory", AETHER_GATEWAY_AUTO_PREPARE_DATABASE="true", JWT_SECRET_KEY=uuid.uuid4().hex + uuid.uuid4().hex,
               ENCRYPTION_KEY=uuid.uuid4().hex + uuid.uuid4().hex, ADMIN_USERNAME="desktopqa", ADMIN_PASSWORD="Aether-QA-2026!",
               AUTH_REFRESH_COOKIE_SECURE="false", AUTH_REFRESH_COOKIE_SAMESITE="lax", AETHER_LOG_FORMAT="json", RUST_LOG="aether_gateway=info,aether_data=info")
    command = [gateway, "--app-host", "127.0.0.1", "--app-port", str(port), "--listener-shards", "1", "--shutdown-timeout-seconds", "3", "--exit-on-stdin-close", "--static-dir", web]
    desktop_secret = None
    if desktop:
        command.append("--desktop-mode")
        del env["ADMIN_USERNAME"]
        del env["ADMIN_PASSWORD"]
    logpath = root / "gateway.log"
    processes = []

    def start():
        nonlocal desktop_secret
        if desktop:
            desktop_secret = uuid.uuid4().hex + uuid.uuid4().hex
            env["AETHER_DESKTOP_SESSION_SECRET"] = desktop_secret
        log = open(logpath, "ab")
        child = subprocess.Popen(command, env=env, cwd=data, stdin=subprocess.PIPE, stdout=log, stderr=log, start_new_session=True)
        log.close()
        processes.append(child)
        wait_for(lambda: available(port))
        assert child.poll() is None
        return child

    def stream(seconds):
        session = Session(json.loads(statefile.read_text()), desktop_secret)
        session.login()
        started = threading.Event()
        result = {"done": False}

        def consume():
            try:
                body = {"model": "desktop-qa-model", "messages": [{"role": "user", "content": f"hold:{seconds}"}], "stream": True}
                with session.request("POST", "/v1/chat/completions", body, token=session.state["api_key"]) as response:
                    for line in response:
                        if b"Aether desktop QA OK" in line:
                            started.set()
                        if b"[DONE]" in line:
                            result["done"] = True
            except Exception as error:
                result["error"] = type(error).__name__
        worker = threading.Thread(target=consume, daemon=True)
        worker.start()
        assert started.wait(10), result
        return worker, result

    try:
        child = start()
        qa_env = dict(os.environ)
        if desktop:
            qa_env["AETHER_QA_DESKTOP_SESSION_SECRET"] = desktop_secret
        else:
            qa_env.pop("AETHER_QA_DESKTOP_SESSION_SECRET", None)
        subprocess.run([sys.executable, str(Path(__file__).with_name("qa_api.py")), "seed", str(statefile)], check=True, env=qa_env)
        subprocess.run([sys.executable, str(Path(__file__).with_name("qa_api.py")), "check", str(statefile)], check=True, env=qa_env)
        worker, result = stream(1)
        before = time.monotonic(); child.stdin.close()
        wait_for(lambda: not available(port), 2)
        assert child.wait(timeout=6) == 0
        worker.join(2)
        assert result["done"] and not worker.is_alive(), result
        print(f"PASS: stdin EOF stops accepting and preserves an active SSE ({time.monotonic()-before:.2f}s)", flush=True)
        child = start()
        worker, result = stream(30)
        before = time.monotonic(); child.send_signal(signal.SIGTERM)
        assert child.wait(timeout=6) == 0
        worker.join(2)
        assert not worker.is_alive() and not result["done"], result
        assert "gateway_shutdown_connections_timed_out" in logpath.read_text()
        print(f"PASS: SIGTERM bounds an overlong SSE and releases its socket ({time.monotonic()-before:.2f}s)", flush=True)
        owner_env = dict(env, AETHER_QA_COMMAND=json.dumps(command), AETHER_QA_LOG=str(logpath))
        owner = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), "--owner"], env=owner_env, cwd=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
        processes.append(owner)
        grandchild_pid = int(owner.stdout.readline())
        wait_for(lambda: available(port))
        before = time.monotonic(); owner.kill(); owner.wait(timeout=3)
        wait_for(lambda: not available(port), 6)
        def child_gone():
            try: os.kill(grandchild_pid, 0); return False
            except ProcessLookupError: return True
        wait_for(child_gone, 6)
        print(f"PASS: killing the owning process exits the gateway, with no orphan ({time.monotonic()-before:.2f}s)", flush=True)
        connection = sqlite3.connect(data / "aether.db")
        tables = [row[0] for row in connection.execute("select name from sqlite_master where type='table' and name like '%usage%'")]
        counts = {table: connection.execute('select count(*) from "' + table + '"').fetchone()[0] for table in tables}
        connection.close()
        print("Persisted usage counts:", counts, flush=True)
        assert any(count > 0 for count in counts.values()), counts
        print("Artifacts:", root, flush=True)
    finally:
        for child in processes:
            if child.poll() is None:
                child.terminate()
                try: child.wait(timeout=6)
                except subprocess.TimeoutExpired: child.kill(); child.wait()
        server.shutdown()


if __name__ == "__main__":
    main()
