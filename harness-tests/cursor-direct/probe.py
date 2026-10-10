#!/usr/bin/env python3
"""Invoke the unmodified official local CLI with private state and no account."""
import argparse
import json
import os
from pathlib import Path
import signal
import shlex
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
SCRATCH = ROOT / '.build/cursor-direct'
SOURCE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--base-url')
    parser.add_argument('--case', default='mock')
    parser.add_argument('--model', default='md-nemotron')
    parser.add_argument('--token-file')
    parser.add_argument('--tools', action='store_true')
    parser.add_argument('--max-retries', type=int, default=0)
    parser.add_argument('--managed-command', type=Path)
    parser.add_argument('--list-models', action='store_true')
    args = parser.parse_args()
    download = json.loads((SCRATCH / 'download.json').read_text())
    binary = Path(download['entry']).parent
    state = SCRATCH / '.runtime' / args.case
    state.mkdir(parents=True, exist_ok=True, mode=0o700)
    home = state / 'home'
    home.mkdir(parents=True, exist_ok=True)
    work = ROOT / 'harness-tests/workspaces/cursor-agent-local' / args.case
    work.mkdir(parents=True, exist_ok=True)
    (work / 'fixture.json').write_text(json.dumps({'numbers': [17, 29, 43], 'case': args.case}) + '\n')
    (work / 'result.txt').write_text('')
    # Stop repository discovery at this disposable fixture; it contains no user source.
    if not (work / '.git').exists():
        subprocess.run(['git', 'init', '--quiet', str(work)], check=True)
    env = dict(os.environ)
    for name in list(env):
        if name.endswith('_OR_KEY') or 'API_KEY' in name or 'AUTH_TOKEN' in name or name.startswith('MD_TEST_'):
            env.pop(name)
    if args.base_url:
        base = args.base_url
        token_file = Path(args.token_file)
    else:
        ready = json.loads((SCRATCH / '.runtime/mock-server/ready.json').read_text())
        base = ready['url'] + '/v1'
        token_file = SCRATCH / '.runtime/mock-server/token'
    token = token_file.read_text().strip()
    env.update({'CURSOR_CONFIG_DIR': str(home / '.cursor'), 'CURSOR_DATA_DIR': str(home / '.cursor'),
        'XDG_CONFIG_HOME': str(home / '.config'), 'XDG_DATA_HOME': str(home / '.local/share'),
        'XDG_CACHE_HOME': str(home / '.cache'), 'NODE_COMPILE_CACHE': str(state / 'compile-cache'),
        'CURSOR_ENABLE_AUTHLESS': '1', 'CURSOR_LOCAL_AGENT_BASE_URL': base,
        'CURSOR_LOCAL_AGENT_API_KEY': token, 'CURSOR_LOCAL_AGENT_MAX_RETRIES': str(args.max_retries),
        'AGENT_CLI_CREDENTIAL_STORE': 'memory', 'CURSOR_INVOKED_AS': 'cursor-agent-local',
        'MODELDECK_TEST_PRIVATE_HOME': str(home),
        'MODELDECK_TEST_NETWORK_LOG': str(state / 'network.jsonl')})
    prompt = ('Use tools to read fixture.json in this workspace. Sum its numbers and write only '
              'the integer to result.txt. Finish with MD_TEST_OK. Do not create subagents or '
              'read files outside this workspace.') if args.tools else 'For a local API transport test, reply exactly MD_LOCAL_OK.'
    command = [str(binary / 'node'), '--use-system-ca', '--require', str(SOURCE / 'private-home.cjs'),
        str(binary / 'index.js'), '--authless', '--base-url', base, '--model', args.model,
        '--disable-project-configs', '--exclude-workspace-context', '--disable-auto-update',
        '--workspace', str(work), '--trust', '--force', '--print', '--output-format', 'stream-json', prompt]
    if args.managed_command:
        env['NODE_OPTIONS'] = '--require ' + shlex.quote(str(SOURCE / 'private-home.cjs'))
        command = [str(args.managed_command), '--disable-project-configs', '--exclude-workspace-context',
            '--workspace', str(work), '--trust', '--force', '--model', args.model]
        command += ['--list-models'] if args.list_models else ['--print', '--output-format', 'stream-json', prompt]
    start = time.monotonic()
    process = subprocess.Popen(command, env=env, cwd=work, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, start_new_session=True)
    try:
        output, _ = process.communicate(timeout=125)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            output, _ = process.communicate(timeout=3)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            output, _ = process.communicate()
    text = output.decode(errors='replace').replace(token, '<redacted>')
    (state / 'output.log').write_text(text)
    result = (work / 'result.txt').read_text().strip()
    events = []
    session_ids = []
    for line in text.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        events.append(event)
        if isinstance(event.get('session_id'), str) and event['session_id'] not in session_ids:
            session_ids.append(event['session_id'])
    marker = 'MD_TEST_OK' if args.tools else 'MD_LOCAL_OK'
    final_seen = (all(model in text for model in ['md-nemotron', 'md-inkling']) if args.list_models
        else any(marker in json.dumps(event) for event in events if event.get('type') in ['assistant', 'result']))
    network = [json.loads(line) for line in (state / 'network.jsonl').read_text().splitlines()] if (state / 'network.jsonl').exists() else []
    report = {'case': args.case, 'model': args.model, 'seconds': round(time.monotonic() - start, 2),
        'exitCode': process.returncode, 'finalTextSeen': final_seen, 'sessionIds': session_ids,
        'resultFileValue': result, 'cursorAccountCredentialsProvided': False,
        'nativeVendorBinaryUnmodified': True, 'network': network,
        'managedLauncher': str(args.managed_command) if args.managed_command else None,
        'modelCatalogProbe': args.list_models,
        'passed': process.returncode == 0 and final_seen and (result == '89' if args.tools else True)}
    (state / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))
    if not report['passed']:
        print(text[-3000:])


if __name__ == '__main__':
    main()
