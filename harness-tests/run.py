#!/usr/bin/env python3
"""Real client smoke tests. Secrets remain in process memory or ignored runtime files."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import ssl
import sys
import time
import urllib.error
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parent
RUNTIME = ROOT / '.runtime'
USER = Path('/Users/vita')
CLIENTS = ['codex', 'claude', 'claude-desktop', 'antigravity', 'cursor-agent',
           'cursor', 'grokbuild', 'muse', 'opencode', 'hermes']
MODELS = {
    'nemotron': 'nvidia/nemotron-3-ultra-550b-a55b:free',
    'inkling': 'thinkingmachines/inkling:free',
    'arena': 'coding-router-preview',
}
PROTECTED = [
    USER / '.codex/config.toml', USER / '.codex/auth.json',
    USER / '.config/codex-cli/config.toml', USER / '.claude/settings.json',
    USER / '.config/opencode/opencode.json', USER / '.local/share/opencode/auth.json',
    USER / '.cc-switch/model-clients.json', USER / '.grok/config.toml',
    USER / '.config/muse/settings.json', USER / '.gemini/antigravity-cli/settings.json',
]

_CREDENTIAL_REFERENCE = re.compile(r'\{(env|file):([^{}]+)\}')


def resolve_credential(value, config_dir, env):
    """Resolve OpenCode key references without logging or persisting the key."""
    if not isinstance(value, str):
        raise ValueError('The configured API credential must be a string')

    def substitute(match):
        kind, reference = match.groups()
        if kind == 'env':
            return env.get(reference, '')
        path = Path(reference).expanduser()
        if not path.is_absolute():
            path = Path(config_dir) / path
        try:
            return path.read_text(encoding='utf-8').strip()
        except (OSError, UnicodeError) as error:
            raise ValueError('The API credential file reference could not be read') from None

    resolved = _CREDENTIAL_REFERENCE.sub(substitute, value).strip()
    if not resolved:
        raise ValueError('The resolved API credential is missing or empty')
    if '{file:' in resolved or '{env:' in resolved:
        raise ValueError('The API credential contains an unresolved reference')
    if '\n' in resolved or '\r' in resolved:
        raise ValueError('The API credential must contain a single line')
    return resolved


def credentials():
    import sqlite3
    env = dict(os.environ)
    config_path = USER / '.config/opencode/opencode.json'
    config = json.loads(config_path.read_text())
    env['MD_TEST_ARENA_KEY'] = resolve_credential(
        config['provider']['arena']['options']['apiKey'], config_path.parent, env)
    # The desktop connection has its own saved upstream credential.
    with sqlite3.connect(f'file:{USER}/.cc-switch/cc-switch.db?mode=ro', uri=True) as db:
        row = db.execute("SELECT settings_config FROM providers WHERE app_type='claude-desktop' AND id='openrouter-claude-desktop'").fetchone()
        if row:
            desktop = json.loads(row[0])
            token = desktop.get('env', {}).get('ANTHROPIC_AUTH_TOKEN') or desktop.get('env', {}).get('ANTHROPIC_API_KEY')
            if token:
                env['MD_TEST_DESKTOP_OR_KEY'] = token
    desktop_files = list((USER / 'Library/Application Support/Claude-3p/configLibrary').glob('*.json'))
    for path in desktop_files:
        doc = json.loads(path.read_text())
        if str(doc.get('inferenceGatewayBaseUrl', '')).startswith('http://127.0.0.1:15721'):
            env['MD_TEST_DESKTOP_GATEWAY_TOKEN'] = doc.get('inferenceGatewayApiKey', '')
            env['MD_TEST_DESKTOP_PROFILE_FILE'] = str(path)
            break
    return env


def scrub(text, env=None):
    if env:
        for name, value in env.items():
            if value and len(value) > 10 and ('KEY' in name or 'TOKEN' in name):
                text = text.replace(value, '<redacted>')
    text = re.sub(r'sk-(?:or-v1-|proj-)?[A-Za-z0-9_-]{12,}', '<redacted>', text)
    return text


def brief(record):
    return {key: record.get(key) for key in ['client','case','passed','seconds','exit_code',
            'result_file_value','runtime_session_ids','free_rate_limit_429','output_log']}


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + '.tmp')
    tmp.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n')
    tmp.replace(path)


def request(url, key='', body=None, timeout=35):
    headers = {'Content-Type': 'application/json', 'User-Agent': 'ModelDeck/0.3.0'}
    if key:
        if '{file:' in key or '{env:' in key:
            raise ValueError('Resolve API credential references before making a request')
        headers['Authorization'] = 'Bearer ' + key
    req = urllib.request.Request(url, data=json.dumps(body).encode() if body is not None else None,
                                 headers=headers)
    context = ssl.create_default_context(cafile='/etc/ssl/cert.pem')
    try:
        with urllib.request.urlopen(req, timeout=timeout, context=context) as response:
            return response.status, dict(response.headers), response.read().decode()
    except urllib.error.HTTPError as error:
        return error.code, dict(error.headers), error.read().decode(errors='replace')


def fingerprint():
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
            for path in PROTECTED}


def prepare():
    RUNTIME.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(RUNTIME, 0o700)
    env = credentials()
    save(RUNTIME / 'protected-before.json', fingerprint())
    metadata = {}
    observations = []
    providers = [('openrouter', 'https://openrouter.ai/api/v1', env.get('OC_OR_KEY', ''))]
    if env.get('MD_TEST_INCLUDE_ARENA') == '1':
        providers.append(('arena', 'https://api.preview.arena.ai/v1', env['MD_TEST_ARENA_KEY']))
    for provider, base, key in providers:
        try:
            status, headers, body = request(base + '/models', key)
            doc = json.loads(body)
            models = doc.get('data', [])
            metadata[provider] = {item['id']: item for item in models if isinstance(item, dict) and item.get('id')}
            observations.append({'provider': provider, 'status': status, 'model_count': len(models),
                                 'error': scrub(body[:1200], env) if status != 200 else None})
        except Exception as error:
            observations.append({'provider': provider, 'error': scrub(str(error), env)})
    save(ROOT / 'provider-preflight.json', observations)
    wanted = {k: metadata.get('openrouter', {}).get(v) for k, v in MODELS.items() if k != 'arena'}
    fallback_id = next((name for name in ['openai/gpt-6-luna', 'openai/gpt-5-nano', 'openai/gpt-4.1-nano']
                        if name in metadata.get('openrouter', {})), None)
    save(ROOT / 'model-metadata.json', {'requested': wanted,
         'fallback_candidate': metadata.get('openrouter', {}).get(fallback_id) if fallback_id else None,
         'arena_requested': metadata.get('arena', {}).get(MODELS['arena'])})
    store = {'schemaVersion': 1, 'revision': 1, 'providers': {}, 'models': {},
             'endpoints': {}, 'bindings': {}, 'activeBindings': {},
             'migration': {'importedProviders': 0, 'importedModels': 0, 'retained': []}}
    key_vars = {'codex': 'CX_OR_KEY', 'claude': 'CC_OR_KEY',
                'claude-desktop': 'MD_TEST_DESKTOP_OR_KEY'}
    execs = {'antigravity': str(USER / '.local/bin/agy'),
             'muse': str(USER / '.local/bin/muse-bin-0.1.0-R708.1'),
             'cursor-agent': str(USER / '.local/bin/cursor-agent')}
    for client in CLIENTS:
        provider_id = 'or_' + client.replace('-', '_')
        store['providers'][provider_id] = {'id': provider_id, 'name': 'OpenRouter test ' + client,
            'baseUrl': 'https://openrouter.ai/api/v1', 'protocol': 'openai-chat',
            'apiKey': '', 'keyEnv': key_vars.get(client, 'OC_OR_KEY')}
        endpoint_id = 'entry_' + client.replace('-', '_')
        endpoint = {'id': endpoint_id, 'name': client + ' isolated test',
                    'protocols': ['openai-chat', 'openai-responses', 'anthropic', 'gemini'],
                    'publicUrl': '', 'token': 'md-test-' + uuid.uuid4().hex, 'routes': {}}
        if client == 'claude-desktop' and env.get('MD_TEST_DESKTOP_GATEWAY_TOKEN'):
            endpoint['token'] = env['MD_TEST_DESKTOP_GATEWAY_TOKEN']
        aliases = {'nemotron': 'md-nemotron', 'inkling': 'md-inkling', 'arena': 'md-arena'}
        if client == 'claude-desktop':
            aliases = {'nemotron': 'claude-sonnet-4-6', 'inkling': 'claude-opus-4-6', 'arena': 'claude-haiku-4-5'}
        for label, model_id in MODELS.items():
            resource_id = client.replace('-', '_') + '_' + label
            info = metadata.get('openrouter' if label != 'arena' else 'arena', {}).get(model_id, {})
            context = info.get('context_length') or 131072
            output = min(2048, info.get('top_provider', {}).get('max_completion_tokens') or 2048)
            store['models'][resource_id] = {'id': resource_id,
                'providerId': provider_id if label != 'arena' else 'arena',
                'upstreamId': model_id, 'name': info.get('name') or label,
                'context': context, 'output': output, 'reasoning': [],
                'tools': True, 'images': False}
            endpoint['routes'][aliases[label]] = {'modelId': resource_id, 'effort': ''}
        store['endpoints'][endpoint_id] = endpoint
        store['bindings'][client] = {'endpointId': endpoint_id, 'model': aliases['nemotron'],
                                     'utilityModel': aliases['nemotron'], 'executable': execs.get(client, '')}
    store['providers']['arena'] = {'id': 'arena', 'name': 'Arena Router test',
        'baseUrl': 'https://api.preview.arena.ai/v1', 'protocol': 'openai-chat',
        'apiKey': '', 'keyEnv': 'MD_TEST_ARENA_KEY'}
    save(RUNTIME / 'initial-store.json', store)
    save(RUNTIME / 'plan.json', {'fallbackId': fallback_id, 'executables': execs,
          'models': MODELS, 'keyReuseAuthorization': 'Only this test, reuse OC_OR_KEY for missing clients'})
    print(json.dumps({'preflight': observations, 'requested_available': {k: bool(v) for k,v in wanted.items()},
                      'fallback_candidate': fallback_id, 'runtime': str(RUNTIME)}, ensure_ascii=False))


def server_env():
    env = credentials()
    env.update({'CC_SWITCH_TEST_HOME': str(RUNTIME / 'home'),
                'XDG_CONFIG_HOME': str(RUNTIME / 'home/.config'),
                'HERMES_HOME': str(RUNTIME / 'home/.hermes'),
                'LOCALAPPDATA': str(RUNTIME / 'home/AppData/Local'),
                'MODELDECK_LIVE_TEST_ROOT': str(RUNTIME),
                'CARGO_HOME': str(ROOT.parent / '.build/cargo-home'),
                'DYLD_FALLBACK_LIBRARY_PATH': '/opt/homebrew/Cellar/z3/4.16.0/lib'})
    return env


def gateway():
    cmd = ['cargo', 'test', '--lib', 'real_harness_gateway_fixture', '--', '--ignored', '--exact', '--nocapture']
    # Use the complete test name so only this network fixture runs.
    cmd[3] = 'deck::native::tests::real_harness_gateway_fixture'
    proc = subprocess.Popen(cmd, cwd=ROOT.parent / 'src-tauri', env=server_env())
    raise SystemExit(proc.wait())


def toml_value(value):
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, bool):
        return str(value).lower()
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, list):
        return '[' + ','.join(toml_value(item) for item in value) + ']'
    if isinstance(value, dict):
        return '{' + ','.join(json.dumps(key) + '=' + toml_value(item) for key,item in value.items()) + '}'
    raise ValueError('Unsupported TOML value')


def client_command(client, label, work, session, env):
    isolated = RUNTIME / 'home'
    env.update({'XDG_CONFIG_HOME': str(isolated / '.config'),
                'XDG_DATA_HOME': str(RUNTIME / 'client-data' / client),
                'XDG_CACHE_HOME': str(RUNTIME / 'client-cache' / client),
                'CLAUDE_CONFIG_DIR': str(isolated / '.claude'),
                'GROK_HOME': str(isolated / '.grok'),
                'GROK_DISABLE_AUTOUPDATER': '1', 'GROK_SUBAGENTS': '0', 'GROK_MEMORY': '0',
                'HERMES_HOME': str(isolated / '.hermes'),
                'HERMES_RUNTIME_DIR': str(RUNTIME / 'hermes-pm'),
                'HERMES_NO_AUTO_UPDATE': '1',
                'AGY_CLI_DISABLE_AUTO_UPDATE': '1', 'MUSE_NO_AUTO_UPDATE': '1',
                'OPENCODE_DISABLE_AUTOUPDATE': 'true',
                'OPENCODE_DISABLE_EXTERNAL_SKILLS': 'true',
                'OPENCODE_CONFIG': str(isolated / '.config/opencode/opencode.json')})
    for key in ['MUSE_AUTH_PATH', 'ANTHROPIC_API_KEY', 'ANTHROPIC_AUTH_TOKEN', 'ANTHROPIC_BASE_URL',
                'GOOGLE_API_KEY', 'GEMINI_API_KEY', 'OPENAI_API_KEY', 'OPENAI_BASE_URL',
                'CLAUDE_CODE_OAUTH_TOKEN']:
        env.pop(key, None)
    prompt = ('This is a small isolated coding test. Use tools to read fixture.json in the current workspace. '
              'Compute the sum of its numbers, then write only that integer to result.txt in the same workspace. '
              'Finish with MD_TEST_OK. Do not create subagents or read files outside this workspace.')
    if client == 'muse':
        prompt += ' Use JSON numbers for numeric tool arguments such as line limits, never quoted strings.'
    launcher = isolated / '.cc-switch/bin'
    if client == 'codex':
        import tomllib
        config = tomllib.loads((isolated / '.codex/config.toml').read_text())
        # CLI -c overrides resolve file paths from the invocation rather than the source config.
        if config.get('model_catalog_json') and not Path(config['model_catalog_json']).is_absolute():
            config['model_catalog_json'] = str(isolated / '.codex' / config['model_catalog_json'])
        args = [str(USER / '.codex/packages/standalone/current/bin/codex'), '--no-daemon',
                '-a', 'never', '-s', 'workspace-write']
        for key,value in config.items():
            args.extend(['-c', key + '=' + toml_value(value)])
        for setting in ['features.context_management.experimental_mode=false',
                        'features.token_budget.enabled=false',
                        'features.token_budget.use_history_notes_extension=false',
                        'project_root_markers=[]', 'project_doc_max_bytes=0']:
            args.extend(['-c', setting])
        args.extend(['exec', '--ignore-user-config', '--ephemeral', '--json', '--color', 'never',
                     '--skip-git-repo-check', '--cd', str(work), prompt])
        return args
    if client == 'claude':
        settings = json.loads((isolated / '.claude/settings.json').read_text())
        env.update(settings.get('env', {}))
        return [str(USER / '.local/bin/claude'), '--bare', '-p', prompt, '--output-format', 'stream-json',
                '--verbose', '--max-turns', '6', '--tools', 'Read,Edit', '--allowedTools', 'Read,Edit',
                '--permission-mode', 'acceptEdits', '--settings', str(isolated / '.claude/settings.json'),
                '--strict-mcp-config', '--mcp-config', '{"mcpServers":{}}', '--disable-slash-commands',
                '--session-id', session, '--no-session-persistence']
    if client == 'opencode':
        file = isolated / '.config/opencode/opencode.json'
        config = json.loads(file.read_text())
        config['permission'] = {'*': 'deny', 'read': 'allow', 'edit': 'allow', 'write': 'allow'}
        save(file, config)
        return [str(USER / '.opencode/bin/opencode'), 'run', '--pure', '--format', 'json',
                '--dir', str(work), '--title', 'ModelDeck test ' + label, prompt]
    if client == 'antigravity':
        return [str(launcher / client), '--gemini_dir=' + str(isolated / '.gemini'),
                '--disable-slash-commands', '--mode', 'accept-edits', '--print-timeout', '110s',
                '--output-format', 'stream-json', '--print', prompt]
    if client == 'muse':
        return [str(launcher / client), 'exec', '--json', '--workspace', str(work),
                '--max-model-steps', '6', '--no-session-log', '--no-foreign-personal-context',
                '--disable-web-tools', '--disable-shell', '--disable-approval', prompt]
    if client == 'grokbuild':
        return [str(USER / '.local/bin/grok'), '--cwd', str(work), '--session-id', session,
                '--no-memory', '--no-subagents', '--always-approve', '--tools', 'read_file,write',
                '--disable-web-search', '--max-turns', '4',
                '--output-format', 'streaming-json', '-p', prompt]
    if client == 'hermes':
        return [str(ROOT / 'vendor/hermes-agent/.venv/bin/hermes'), 'chat', '--oneshot', '--quiet',
                '--format', 'stream-json', '--toolsets', 'file', '--max-turns', '5', '--run-budget', '100',
                '--ignore-rules', '--source', 'tool', '-q', prompt]
    if client == 'cursor-agent':
        return [str(launcher / client), '--print', '--output-format', 'stream-json',
                '--trust', '--workspace', str(work), prompt]
    raise ValueError('GUI client needs native UI automation')


def records():
    path = RUNTIME / 'gateway-records.jsonl'
    if not path.exists():
        return []
    values = []
    for line in path.read_text().splitlines():
        try:
            values.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    return values


def run_case(client, label, fallback_id=None):
    env = credentials()
    if label == 'arena' and env.get('MD_TEST_INCLUDE_ARENA') != '1':
        raise SystemExit('Arena tests are disabled. Explicitly set MD_TEST_INCLUDE_ARENA=1 to opt in.')
    store = json.loads((RUNTIME / 'initial-store.json').read_text())
    for endpoint in store['endpoints'].values():
        env['MD_TEST_TOKEN_' + endpoint['id']] = endpoint['token']
    ready = json.loads((RUNTIME / 'gateway-ready.json').read_text())
    body = {'client': client, 'label': label}
    if fallback_id:
        body['fallbackId'] = fallback_id
    status, _, apply_output = request(ready['origin'] + '/__test/apply', body=body)
    suffix = label + ('-fallback' if fallback_id else '')
    name = client + '/' + suffix
    record = {'client': client, 'requested': MODELS[label], 'case': suffix,
              'session_id': str(uuid.uuid4()), 'apply_status': status,
              'model_used': fallback_id or MODELS[label], 'real_client': False,
              'gateway_requests': [], 'passed': False}
    result_file = ROOT / 'results' / client / (suffix + '.json')
    if result_file.exists():
        stamp = time.strftime('%Y%m%dT%H%M%SZ', time.gmtime())
        old = json.loads(result_file.read_text())
        save(ROOT / 'attempts' / client / (suffix + '-' + stamp + '.json'), old)
        previous_log = ROOT / 'logs' / client / (suffix + '.log')
        if previous_log.is_file():
            archive = ROOT / 'attempts' / client / (suffix + '-' + stamp + '.log')
            archive.write_text(previous_log.read_text())
    if status != 200:
        record['failure'] = scrub(apply_output, env)
        save(result_file, record)
        print(json.dumps({'client':client,'case':suffix,'apply_status':status,'failure':record['failure']}, ensure_ascii=False), flush=True)
        return record
    work = RUNTIME / 'workspaces' / client / suffix
    work.mkdir(parents=True, exist_ok=True)
    numbers = [17, 29, 43]
    save(work / 'fixture.json', {'numbers': numbers, 'case': name})
    # Each attempt starts with an empty result; a prior successful run cannot pass it.
    (work / 'result.txt').write_text('')
    index = len(records())
    try:
        command = client_command(client, label, work, record['session_id'], env)
        redaction_env = dict(env)
        for name in list(env):
            if name.endswith('_OR_KEY') or name.startswith('MD_TEST_'):
                env.pop(name)
        start = time.monotonic()
        process = subprocess.Popen(command, cwd=work, env=env, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        record['real_client'] = True
        record['pid'] = process.pid
        try:
            output, _ = process.communicate(timeout=125)
            record['exit_code'] = process.returncode
        except subprocess.TimeoutExpired:
            import signal
            os.killpg(process.pid, signal.SIGTERM)
            try:
                output, _ = process.communicate(timeout=4)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                output, _ = process.communicate()
            record['exit_code'] = process.returncode
            record['timeout'] = True
        record['seconds'] = round(time.monotonic() - start, 2)
        output = scrub(output.decode(errors='replace'), redaction_env)
        actual_ids = []
        for line in output.splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            for item in [event, event.get('step_update', {}), event.get('result', {}), event.get('stream', {})]:
                if not isinstance(item, dict):
                    continue
                for key in ['session_id', 'sessionID', 'sessionId', 'thread_id', 'conversation_id']:
                    value = item.get(key)
                    if isinstance(value, str) and value and value not in actual_ids:
                        actual_ids.append(value)
                if item.get('kind') == 'session' and isinstance(item.get('id'), str):
                    if item['id'] not in actual_ids:
                        actual_ids.append(item['id'])
        record['runtime_session_ids'] = actual_ids
        log = ROOT / 'logs' / client / (suffix + '.log')
        log.parent.mkdir(parents=True, exist_ok=True)
        log.write_text(output)
        record['output_log'] = str(log.relative_to(ROOT))
        result = (work / 'result.txt').read_text().strip() if (work / 'result.txt').exists() else None
        record['result_file_value'] = result
        record['passed'] = record['exit_code'] == 0 and result == str(sum(numbers)) and 'MD_TEST_OK' in output
        if not record['passed']:
            record['failure'] = output[-2200:]
    except Exception as error:
        record['failure'] = scrub(str(error), env)
    paths = ['/'+client+'/', '/deck/entry_'+client.replace('-','_')+'/']
    record['gateway_requests'] = [item for item in records()[index:]
                                  if any(path in item.get('path','') for path in paths)]
    # A paid retry requires a captured upstream 429 explicitly identifying free-model rate limits.
    def explicit_free_limit(item):
        detail = json.dumps(item.get('error', {})).lower()
        names_free_models = any(term in detail for term in ['free-models-per', 'free model', 'free-model'])
        names_limit = any(term in detail for term in ['rate limit', 'rate-limit', 'rate_limit', 'limit exceeded'])
        return item.get('status') == 429 and names_free_models and names_limit
    record['free_rate_limit_429'] = any(explicit_free_limit(item)
        for item in record['gateway_requests']) and MODELS[label].endswith(':free')
    save(result_file, record)
    print(json.dumps(brief(record), ensure_ascii=False), flush=True)
    if record['free_rate_limit_429'] and not fallback_id:
        plan = json.loads((RUNTIME / 'plan.json').read_text())
        if plan.get('fallbackId'):
            return run_case(client, label, plan['fallbackId'])
    return record


def run():
    args = sys.argv[2:]
    if len(args) != 2 or args[0] not in CLIENTS or args[1] not in MODELS:
        raise SystemExit('run CLIENT nemotron|inkling|arena')
    run_case(*args)


def probe():
    env = credentials()
    result = []
    for label, model in MODELS.items():
        if label == 'arena' and env.get('MD_TEST_INCLUDE_ARENA') != '1':
            continue
        base = 'https://api.preview.arena.ai/v1' if label == 'arena' else 'https://openrouter.ai/api/v1'
        key = env['MD_TEST_ARENA_KEY'] if label == 'arena' else env['OC_OR_KEY']
        try:
            status, headers, body = request(base + '/chat/completions', key,
                {'model': model, 'messages': [{'role': 'user', 'content': 'For an API coding test, reply only MD_PROBE_OK.'}],
                 'max_tokens': 32, 'stream': False}, timeout=45)
            result.append({'label': label, 'model': model, 'status': status,
                           'content_type': headers.get('Content-Type'), 'body': scrub(body[:4000], env)})
        except Exception as error:
            result.append({'label': label, 'model': model, 'error': scrub(str(error), env)})
        save(ROOT / 'provider-inference-probes.json', result)
        print(json.dumps(result[-1], ensure_ascii=False), flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=['prepare', 'gateway', 'run', 'probe'])
    args, _ = parser.parse_known_args()
    globals()[args.command]()


if __name__ == '__main__':
    main()
