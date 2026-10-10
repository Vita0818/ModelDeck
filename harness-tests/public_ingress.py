#!/usr/bin/env python3
"""Temporary Cursor-only ingress; the control API is never exposed."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import http.client
import json
from pathlib import Path
import os

ROOT = Path(__file__).resolve().parent / '.runtime'
STORE = ROOT / 'home/.cc-switch/model-deck.json'


class Ingress(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *_):
        pass

    def handle_api(self):
        prefix = next((p for p in ['/deck/entry_cursor_agent/', '/deck/entry_cursor/']
                       if self.path.startswith(p)), None)
        if prefix is None:
            self.send_error(404)
            return
        store = json.loads(STORE.read_text())
        endpoint = prefix.split('/')[2]
        token = store['endpoints'][endpoint]['token']
        if self.headers.get('Authorization') != 'Bearer ' + token:
            self.send_error(401)
            return
        length = int(self.headers.get('Content-Length', 0))
        if length > 8 * 1024 * 1024:
            self.send_error(413)
            return
        body = self.rfile.read(length) if length else None
        headers = {key: value for key, value in self.headers.items()
                   if key.lower() in ['authorization', 'content-type', 'user-agent', 'http-referer',
                                      'x-title', 'x-openrouter-title', 'x-openrouter-categories']}
        conn = http.client.HTTPConnection('127.0.0.1', 15721, timeout=120)
        try:
            conn.request(self.command, self.path, body=body, headers=headers)
            response = conn.getresponse()
            # Quick Tunnels do not support live SSE. Send a complete fixed-length
            # SSE payload; the client still consumes the production wire events.
            data = response.read()
            self.send_response(response.status)
            self.send_header('Content-Type', response.getheader('Content-Type', 'application/json'))
            self.send_header('Content-Length', str(len(data)))
            self.send_header('Cache-Control', 'no-store')
            self.end_headers()
            self.wfile.write(data)
        finally:
            conn.close()

    do_GET = handle_api
    do_POST = handle_api


if __name__ == '__main__':
    (ROOT / 'ingress-ready.json').write_text(json.dumps({'pid': os.getpid(), 'port': 15722}))
    ThreadingHTTPServer(('127.0.0.1', 15722), Ingress).serve_forever()
