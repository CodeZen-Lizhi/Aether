"""Local-only desktop acceptance fixture. Use with a disposable --data-dir.

The state file belongs outside the checkout and contains synthetic test keys.
Start `serve STATE`, set up the .app as desktopqa / Aether-QA-2026!, then run
`seed STATE` and `check STATE`. `stream STATE SECONDS` holds an SSE request for
the shutdown checks. No requests leave loopback.
"""
import http.cookiejar
import json
import os
from pathlib import Path
import sys
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class MockUpstream(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def do_GET(self):
        body = json.dumps({"object": "list", "data": [{"id": "desktop-qa-model", "object": "model", "owned_by": "local-qa"}]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
        assert self.headers.get("Authorization") == "Bearer sk-aether-desktop-qa-upstream"
        base = {"id": "chatcmpl-desktop-qa", "created": int(time.time()), "model": request["model"]}
        usage = {"prompt_tokens": 12, "completion_tokens": 5, "total_tokens": 17}
        if not request.get("stream"):
            body = json.dumps({**base, "object": "chat.completion", "choices": [{"index": 0, "message": {"role": "assistant", "content": "Aether desktop QA OK"}, "finish_reason": "stop"}], "usage": usage}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "close")
        self.end_headers()
        prompt = request["messages"][-1]["content"]
        hold = int(prompt.removeprefix("hold:")) if prompt.startswith("hold:") else 0
        try:
            for index in range(max(1, hold)):
                chunk = {**base, "object": "chat.completion.chunk", "choices": [{"index": 0, "delta": {"content": "Aether desktop QA OK" if index == 0 else "."}, "finish_reason": None}]}
                self.wfile.write(("data: " + json.dumps(chunk) + "\n\n").encode())
                self.wfile.flush()
                if hold:
                    time.sleep(1)
            final = {**base, "object": "chat.completion.chunk", "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}], "usage": usage}
            self.wfile.write(("data: " + json.dumps(final) + "\n\ndata: [DONE]\n\n").encode())
            self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass
        self.close_connection = True


class Session:
    def __init__(self, state, desktop_session_secret=None):
        self.state = state
        self.desktop_session_secret = desktop_session_secret
        self.cookies = http.cookiejar.CookieJar()
        self.client = urllib.request.build_opener(urllib.request.ProxyHandler({}), urllib.request.HTTPCookieProcessor(self.cookies))
        self.token = None

    def request(self, method, route, body=None, token=None):
        headers = {"Content-Type": "application/json", "X-Client-Device-Id": "aether-desktop-qa-api"}
        if token or self.token:
            headers["Authorization"] = "Bearer " + (token or self.token)
        request = urllib.request.Request(self.state["base"] + route, data=None if body is None else json.dumps(body).encode(), headers=headers, method=method)
        return self.client.open(request, timeout=65)

    def json(self, method, route, body=None):
        with self.request(method, route, body) as response:
            return json.load(response)

    def login(self):
        if self.desktop_session_secret:
            request = urllib.request.Request(self.state["base"] + "/_gateway/desktop/session", method="POST", headers={
                "Origin": self.state["base"],
                "X-Client-Device-Id": "aether-desktop-qa-api",
                "X-Aether-Desktop-Session": self.desktop_session_secret,
            })
            with self.client.open(request, timeout=10) as result:
                response = json.load(result)
        else:
            response = self.json("POST", "/api/auth/login", {"email": "desktopqa", "password": "Aether-QA-2026!"})
        self.token = response["access_token"]


def save(statefile, state):
    statefile.write_text(json.dumps(state, indent=2))
    statefile.chmod(0o600)


def main():
    mode, filename, *args = sys.argv[1:]
    statefile = Path(filename)
    state = json.loads(statefile.read_text())
    if not state["base"].startswith("http://127.0.0.1:"):
        raise ValueError("Only a local disposable gateway is supported")
    if mode == "serve":
        server = ThreadingHTTPServer(("127.0.0.1", 0), MockUpstream)
        state.update(upstream_url=f"http://127.0.0.1:{server.server_port}", upstream_pid=os.getpid())
        save(statefile, state)
        print(f"QA upstream listening on 127.0.0.1:{server.server_port}", flush=True)
        server.serve_forever()
        return
    session = Session(state, os.environ.get("AETHER_QA_DESKTOP_SESSION_SECRET"))
    session.login()
    if mode == "seed":
        provider = session.json("POST", "/api/admin/providers/", {"name": "Desktop QA local upstream", "provider_type": "custom", "billing_type": "pay_as_you_go", "is_active": True})
        provider_id = provider["id"]
        session.json("POST", f"/api/admin/endpoints/providers/{provider_id}/endpoints", {"provider_id": provider_id, "api_format": "openai:chat", "base_url": state["upstream_url"], "is_active": True})
        session.json("POST", f"/api/admin/endpoints/providers/{provider_id}/keys", {"name": "QA upstream key", "api_formats": ["openai:chat"], "api_key": "sk-aether-desktop-qa-upstream", "is_active": True, "auto_fetch_models": False})
        model = session.json("POST", "/api/admin/models/global", {"name": "desktop-qa-model", "display_name": "Desktop QA Model", "is_active": True, "default_tiered_pricing": {"tiers": [{"up_to": None, "input_price_per_1m": 1, "output_price_per_1m": 2}]}})
        session.json("POST", f"/api/admin/providers/{provider_id}/models", {"provider_model_name": "desktop-qa-model", "global_model_id": model["id"], "supports_streaming": True, "is_active": True})
        api_key = session.json("POST", "/api/admin/api-keys", {"name": "Desktop QA client", "initial_balance_usd": None, "rate_limit": 0, "concurrent_limit": 0})
        state.update(provider_id=provider_id, model_id=model["id"], api_key_id=api_key["id"], api_key=api_key["key"])
        save(statefile, state)
        print("PASS: provider, endpoint, encrypted upstream key, model mapping and client API key created")
    elif mode in ("check", "stream"):
        def proxy(stream, prompt="hello"):
            return session.request("POST", "/v1/chat/completions", {"model": "desktop-qa-model", "messages": [{"role": "user", "content": prompt}], "stream": stream}, token=state["api_key"])
        if mode == "stream":
            start = time.monotonic()
            done = False
            with proxy(True, "hold:" + args[0]) as response:
                for line in response:
                    if b"Aether desktop QA OK" in line:
                        print("STREAM_STARTED", flush=True)
                    done |= b"[DONE]" in line
            print(json.dumps({"elapsed_seconds": round(time.monotonic() - start, 2), "completed": done}), flush=True)
            return
        with proxy(False) as response:
            assert json.load(response)["choices"][0]["message"]["content"] == "Aether desktop QA OK"
        with proxy(True) as response:
            content = response.read()
            assert b"[DONE]" in content and b"Aether desktop QA OK" in content
        print("PASS: JSON and SSE proxy requests reached the local upstream")
        refreshed = session.json("POST", "/api/auth/refresh")
        assert refreshed["access_token"]
        session.token = refreshed["access_token"]
        assert any(cookie.has_nonstandard_attr("HttpOnly") for cookie in session.cookies)
        print("PASS: HttpOnly refresh cookie renews the same-origin session")
        exported = session.json("GET", "/api/admin/system/config/export")
        assert any(provider["name"] == "Desktop QA local upstream" for provider in exported["providers"])
        exportfile = Path(state["root"]) / "qa-config-export.json"
        exportfile.write_text(json.dumps(exported, indent=2)); exportfile.chmod(0o600)
        imported = session.json("POST", "/api/admin/system/config/import", {**exported, "merge_mode": "skip"})
        assert not imported.get("stats", {}).get("errors"), imported
        print("PASS: config export can be imported using the existing skip contract")
        for route in ("dashboard", "keys", "providers", "models", "routing", "usage", "system", "settings"):
            with session.request("GET", "/admin/" + route) as response:
                assert b"<html" in response.read().lower()
        print("PASS: eight management routes return bundled SPA resources; config export contains QA provider")
        time.sleep(1)
        stats = session.json("GET", "/api/admin/usage/stats")
        print("Usage stats:", json.dumps(stats, ensure_ascii=False))
    else:
        raise ValueError("Expected serve, seed, check or stream")


if __name__ == "__main__":
    main()
