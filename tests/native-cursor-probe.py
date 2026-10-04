#!/usr/bin/env python3
"""Native Cursor wire probe: isolated state, localhost RPC, and dummy keys only."""
import argparse
import base64
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import time

parser = argparse.ArgumentParser()
parser.add_argument("--client", required=True, type=Path)
parser.add_argument("--adapter", required=True, type=Path)
args = parser.parse_args()
root = args.client.resolve().parent
records = []
done = threading.Event()
upstream_key = "cursor-upstream-dedicated-probe-key"
model = "openai/gpt-6-astra"
endpoint = "https://openrouter.ai/api/v1"


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_POST(self):
        content = self.headers.get("Content-Type", "")
        data = b""
        if self.headers.get("Transfer-Encoding", "").lower() == "chunked":
            self.connection.settimeout(10)
            while len(data) < 4 * 1024 * 1024:
                line = self.rfile.readline()
                if not line:
                    break
                length = int(line.split(b";", 1)[0].strip(), 16)
                if not length:
                    self.rfile.readline()
                    break
                data += self.rfile.read(length)
                self.rfile.read(2)
                if content.startswith("application/connect+") and len(data) >= 5:
                    frame_length = int.from_bytes(data[1:5], "big")
                    if len(data) >= frame_length + 5:
                        break
        else:
            data = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        record = {
            "path": self.path,
            "contentType": content,
            "bytes": len(data),
            "correctKey": upstream_key.encode() in data,
            "correctModel": model.encode() in data,
            "correctEndpoint": endpoint.encode() in data,
        }
        records.append(record)
        if record["correctKey"]:
            done.set()
        if content.startswith("application/connect+"):
            tail = json.dumps({"error": {"code": "unavailable", "message": "native-probe-stop"}}).encode()
            payload = b"\x02" + len(tail).to_bytes(4, "big") + tail
        else:
            # Empty proto3 responses suffice for metadata calls. Model metadata
            # comes from the managed catalog, not the fake server.
            payload = b"{}" if "json" in content else b""
        self.send_response(200)
        self.send_header("Content-Type", content or "application/proto")
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(payload)


server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
with tempfile.TemporaryDirectory(prefix="cc-cursor-probe-") as temporary:
    work = Path(temporary)
    config = work / "config"
    config.mkdir()
    (config / "cli-config.json").write_text(json.dumps({"version": 1, "permissions": {"allow": [], "deny": ["Shell(*)", "Write(*)"]}}))
    profile_file = work / "profiles.json"
    profile_file.write_text(json.dumps({"active": {"cursor-agent": {
        "enabled": True, "endpoint": endpoint, "apiKey": upstream_key, "keyEnv": "",
        "models": [{"id": model, "name": "GPT-6 Astra", "context": 1050000, "output": 128000, "reasoning": ["max"]}],
        "defaultModel": model, "protocol": "openai-chat", "effort": "max",
    }}}))
    h2_source = work / "rpc.cjs"
    h2_records = work / "rpc-records.jsonl"
    h2_source.write_text(r'''
const http2 = require('node:http2');
const fs = require('node:fs');
const zlib = require('node:zlib');
const rows = process.argv[2];
const profile = JSON.parse(fs.readFileSync(process.argv[3], 'utf8')).active['cursor-agent'];
const server = http2.createServer();
server.on('stream', (stream, headers) => {
  let data = Buffer.alloc(0);
  stream.on('error', () => {});
  stream.on('data', chunk => {
    data = Buffer.concat([data, chunk]);
    let decoded = data;
    if (data.length >= 5 && data[0] === 1) {
      try { decoded = zlib.gunzipSync(data.subarray(5, 5 + data.readUInt32BE(1))); } catch {}
    }
    const row = {
      path: headers[':path'], contentType: headers['content-type'], bytes: data.length,
      correctKey: decoded.includes(Buffer.from(profile.apiKey)),
      correctModel: decoded.includes(Buffer.from(profile.defaultModel)),
      correctEndpoint: decoded.includes(Buffer.from(profile.endpoint)),
    };
    fs.appendFileSync(rows, JSON.stringify(row) + '\n');
    if (!stream.headersSent) stream.respond({':status': 200, 'content-type': headers['content-type'] || 'application/connect+proto'});
    if (row.correctKey) {
      const tail = Buffer.from(JSON.stringify({error:{code:'unavailable',message:'native-probe-stop'}}));
      const frame = Buffer.alloc(5); frame[0] = 2; frame.writeUInt32BE(tail.length, 1);
      stream.end(Buffer.concat([frame, tail]));
    }
  });
});
server.listen(0, '127.0.0.1', () => console.log(server.address().port));
''')
    h2 = subprocess.Popen([str(root / "node"), str(h2_source), str(h2_records), str(profile_file)], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    h2_port = int(h2.stdout.readline().strip())
    environment = os.environ.copy()
    environment.update({
        "CURSOR_CONFIG_DIR": str(config), "CURSOR_DATA_DIR": str(work / "data"),
        "XDG_CACHE_HOME": str(work / "cache"), "NODE_COMPILE_CACHE": str(work / "node-cache"),
        "CCSWITCH_CURSOR_PROFILE_FILE": str(profile_file), "CURSOR_AGENT_DISABLE_DEBUG_LOG": "1",
        "AGENT_CLI_CREDENTIAL_STORE": "memory", "CI": "1",
    })
    for name in ("CURSOR_API_KEY", "CURSOR_AUTH_TOKEN", "NODE_OPTIONS", "OPENAI_API_KEY", "ANTHROPIC_API_KEY"):
        environment.pop(name, None)
    origin = f"http://127.0.0.1:{server.server_port}"
    encoded = lambda value: base64.urlsafe_b64encode(json.dumps(value).encode()).rstrip(b"=").decode()
    token = encoded({"alg": "none", "typ": "JWT"}) + "." + encoded({"sub": "auth0|native-probe", "exp": 2000000000}) + ".test"
    command = [str(root / "node"), "--require", str(args.adapter.resolve()), str(root / "index.js"),
               "--auth-token", token, "--endpoint", origin, "--agent-endpoint", f"http://127.0.0.1:{h2_port}",
               "--disable-auto-update", "--model", model, "--workspace", str(work), "--trust",
               "--print", "Reply ready without using tools."]
    with (work / "stdout").open("wb") as stdout, (work / "stderr").open("wb") as stderr:
        child = subprocess.Popen(command, cwd=work, env=environment, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr)
        deadline = time.monotonic() + 35
        while child.poll() is None and not done.is_set() and time.monotonic() < deadline:
            if h2_records.exists():
                try:
                    if any(json.loads(line).get("correctKey") for line in h2_records.read_text().splitlines()):
                        break
                except json.JSONDecodeError:
                    pass
            time.sleep(0.1)
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
    h2.terminate()
    h2.wait(timeout=5)
    if h2_records.exists():
        records.extend(json.loads(line) for line in h2_records.read_text().splitlines())
    stdout = (work / "stdout").read_text(errors="replace")
    stderr = (work / "stderr").read_text(errors="replace")
    print(json.dumps({"requests": records, "exitCode": child.returncode, "keyAbsentFromOutput": upstream_key not in stdout + stderr}, indent=2))
    if not any(r["correctKey"] and r["correctModel"] and r["correctEndpoint"] for r in records):
        # All material is generated solely by this synthetic test.
        print("Native stdout:", stdout[-2500:])
        print("Native stderr:", stderr[-3500:])
        raise SystemExit("Cursor did not emit the configured BYOK credentials.")
    if upstream_key in stdout + stderr:
        raise SystemExit("Cursor leaked the probe key into its output.")
server.shutdown()
print("Native Cursor BYOK request serialization verified.")
