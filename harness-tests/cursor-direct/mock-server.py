#!/usr/bin/env python3
"""Loopback-only model mock for the vendor's native authless CLI."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import argparse
import json
from pathlib import Path
import time


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def send_json(self, value, status=200):
        body = json.dumps(value).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        self.send_json({'object': 'list', 'data': [
            {'id': 'md-nemotron', 'object': 'model', 'created': 0, 'owned_by': 'modeldeck'},
            {'id': 'md-inkling', 'object': 'model', 'created': 0, 'owned_by': 'modeldeck'},
        ]})

    def do_POST(self):
        if self.headers.get('Authorization') != 'Bearer ' + self.server.token and \
                self.headers.get('x-api-key') != self.server.token:
            return self.send_json({'error': {'message': 'Invalid local mock token'}}, 401)
        data = json.loads(self.rfile.read(int(self.headers.get('Content-Length', '0'))))
        record = {'path': self.path, 'model': data.get('model'), 'stream': data.get('stream'),
                  'userAgent': self.headers.get('User-Agent'), 'body': data}
        with (self.server.state / 'requests.jsonl').open('a') as out:
            out.write(json.dumps(record) + '\n')
        # First verify plain native model transport before testing tool calls.
        if self.path.endswith('/chat/completions'):
            mode_file = self.server.state / 'mode.json'
            mode = json.loads(mode_file.read_text()) if mode_file.exists() else {}
            tool_results = [item.get('tool_call_id') for item in data.get('messages', [])
                            if item.get('role') == 'tool']
            message = {'role': 'assistant', 'content': 'MD_LOCAL_OK'}
            finish = 'stop'
            if mode.get('tools') and data.get('tools'):
                work = Path(mode['workspace'])
                if 'md-read' not in tool_results:
                    call = {'id': 'md-read', 'type': 'function', 'function': {
                        'name': 'Read', 'arguments': json.dumps({'path': str(work / 'fixture.json')})}}
                elif 'md-write' not in tool_results:
                    call = {'id': 'md-write', 'type': 'function', 'function': {
                        'name': 'Write', 'arguments': json.dumps({'path': str(work / 'result.txt'), 'contents': '89'})}}
                else:
                    call = None
                message = {'role': 'assistant', 'content': None, 'tool_calls': [call]} if call else \
                    {'role': 'assistant', 'content': 'MD_TEST_OK'}
                finish = 'tool_calls' if call else 'stop'
            response = {'id': 'chatcmpl-local-test', 'object': 'chat.completion',
                'created': int(time.time()), 'model': data.get('model'),
                'choices': [{'index': 0, 'message': message, 'finish_reason': finish}],
                'usage': {'prompt_tokens': 100, 'completion_tokens': 10, 'total_tokens': 110}}
            if not data.get('stream'):
                return self.send_json(response)
            delta = dict(message)
            if delta.get('tool_calls'):
                delta['tool_calls'] = [dict(item, index=index) for index, item in enumerate(delta['tool_calls'])]
            payload = [
                {'id': response['id'], 'object': 'chat.completion.chunk', 'created': response['created'],
                 'model': data.get('model'), 'choices': [{'index': 0, 'delta': delta, 'finish_reason': None}]},
                {'id': response['id'], 'object': 'chat.completion.chunk', 'created': response['created'],
                 'model': data.get('model'), 'choices': [{'index': 0, 'delta': {}, 'finish_reason': finish}], 'usage': response['usage']},
            ]
            body = ''.join('data: ' + json.dumps(item) + '\n\n' for item in payload) + 'data: [DONE]\n\n'
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Content-Length', str(len(body.encode())))
            self.end_headers()
            self.wfile.write(body.encode())
            return
        self.send_json({'error': {'message': 'Unsupported local mock operation: ' + self.path}}, 404)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('state')
    args = parser.parse_args()
    state = Path(args.state)
    state.mkdir(parents=True, exist_ok=True)
    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    server.state = state
    server.token = (state / 'token').read_text().strip()
    (state / 'ready.json').write_text(json.dumps({'port': server.server_port,
        'url': 'http://127.0.0.1:' + str(server.server_port), 'pid': __import__('os').getpid()}) + '\n')
    server.serve_forever()


if __name__ == '__main__':
    main()
