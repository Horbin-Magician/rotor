"""Local, deterministic custom-engine responses for actual window acceptance.

Use http://127.0.0.1:18765/translate?text={text}&to={to} in an isolated
profile. This checks UI/transport behavior, not translation quality or the
Google/DeepSeek services. No request text is written to logs.
"""
import argparse
import json
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        request = urlsplit(self.path)
        if request.path == "/health":
            payload, status = {"ready": True}, 200
        elif request.path != "/translate":
            payload, status = {"error": "Unknown fixture endpoint"}, 404
        else:
            text = parse_qs(request.query).get("text", [""])[0]
            if text == "slow":
                time.sleep(2)
            if text == "error":
                payload, status = {"error": "Simulated unavailable service"}, 503
            else:
                translated = "Rotor 让日常任务更轻松。"
                if text == "long":
                    translated = "\n\n".join(
                        f"第 {index} 段：中文与 English 混排，验证换行、滚动和完整复制。"
                        for index in range(1, 21)
                    )
                elif text == "slow":
                    translated = "这是迟到的旧结果，不应覆盖后续请求。"
                payload, status = {"translated": translated}, 200
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        try:
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass  # Expected when the application cancels an obsolete request.

    def log_message(self, *_):
        pass


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=18765)
    arguments = parser.parse_args()
    server = ThreadingHTTPServer(("127.0.0.1", arguments.port), Handler)
    print(f"Rotor translation fixture: 127.0.0.1:{server.server_port}", flush=True)
    server.serve_forever()
