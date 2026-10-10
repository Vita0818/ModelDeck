#!/usr/bin/env python3
"""Observe real native requests, preserving their headers and streaming bodies."""
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import ssl
import sys
import threading
from urllib.parse import urlsplit

RUNTIME = Path(__file__).resolve().parents[2] / '.build/arena-retest/.runtime'
TARGET = urlsplit(sys.argv[1])
LABEL = sys.argv[2]
LOCK = threading.Lock()
HOP_HEADERS = {'host', 'connection', 'keep-alive', 'proxy-authenticate',
               'proxy-authorization', 'te', 'trailer', 'transfer-encoding', 'upgrade'}


def record(value):
    with LOCK, (RUNTIME / (LABEL + '-capture.jsonl')).open('a') as output:
        output.write(json.dumps(value) + '\n')


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *_):
        pass

    def relay(self):
        length = int(self.headers.get('content-length', '0'))
        body = self.rfile.read(length) if length else None
        try:
            value = json.loads(body) if body else {}
        except (ValueError, TypeError):
            value = {}
        context = {}
        for name, header in self.headers.items():
            lower = name.lower()
            if lower in HOP_HEADERS:
                continue
            if any(word in lower for word in ('key', 'token', 'secret', 'credential', 'auth', 'cookie')):
                context[lower] = '<redacted>'
            else:
                context[lower] = header
        observation = {'method': self.command, 'path': self.path.split('?')[0],
                       'headers': context, 'model': value.get('model'),
                       'stream': value.get('stream'), 'metadata': value.get('metadata')}
        connection = (http.client.HTTPSConnection(TARGET.hostname, TARGET.port or 443,
                          timeout=90, context=ssl.create_default_context(cafile='/etc/ssl/cert.pem'))
                      if TARGET.scheme == 'https' else
                      http.client.HTTPConnection(TARGET.hostname, TARGET.port or 80, timeout=90))
        try:
            headers = {name: header for name, header in self.headers.items()
                       if name.lower() not in HOP_HEADERS}
            path = TARGET.path.rstrip('/') + self.path
            connection.request(self.command, path, body=body, headers=headers)
            response = connection.getresponse()
            observation['status'] = response.status
            observation['responseHeaders'] = {name.lower(): header for name, header in response.getheaders()
                if name.lower() in {'x-arena-resolved-model', 'x-arena-trace-id', 'x-request-id'}}
            error_body = None
            if response.status >= 400:
                error_body = response.read(2 * 1024 * 1024)
                error_text = error_body.decode(errors='replace')
                for name, header in self.headers.items():
                    if any(word in name.lower() for word in ('key', 'token', 'secret', 'credential', 'auth', 'cookie')):
                        error_text = error_text.replace(header, '<redacted>')
                        if header.startswith('Bearer '):
                            error_text = error_text.replace(header[7:], '<redacted>')
                try:
                    observation['error'] = json.loads(error_text)
                except ValueError:
                    observation['error'] = error_text[:2000]
            record(observation)
            self.send_response_only(response.status, response.reason)
            for name, header in response.getheaders():
                if name.lower() not in HOP_HEADERS and name.lower() != 'content-length':
                    self.send_header(name, header)
            self.send_header('connection', 'close')
            self.end_headers()
            if error_body is not None:
                self.wfile.write(error_body)
                self.wfile.flush()
            else:
                while chunk := response.read1(65536):
                    self.wfile.write(chunk)
                    self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass
        except Exception as error:
            record({'method': self.command, 'path': self.path.split('?')[0],
                    'exceptionType': type(error).__name__})
            if 'status' not in observation:
                self.send_error(502, 'Native capture upstream connection failed')
        finally:
            self.close_connection = True
            connection.close()

    do_GET = relay
    do_POST = relay
    do_HEAD = relay


server = ThreadingHTTPServer(('127.0.0.1', int(sys.argv[3]) if len(sys.argv) > 3 else 0), Handler)
ready = {'origin': 'http://127.0.0.1:' + str(server.server_port), 'target': sys.argv[1], 'label': LABEL,
         'pid': os.getpid()}
(RUNTIME / (LABEL + '-proxy.json')).write_text(json.dumps(ready) + '\n')
print(json.dumps(ready), flush=True)
server.serve_forever()
