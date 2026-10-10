#!/usr/bin/env python3
"""Launch a real interactive coding CLI in a private Arena test workspace."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('harness', ROOT / 'harness-tests/run.py')
RUN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUN)
RUN.RUNTIME = ROOT / '.build/arena-retest/.runtime'


def main():
    client = sys.argv[1]
    capture = sys.argv[2] if len(sys.argv) > 2 else ''
    if client not in ['codex', 'claude', 'opencode']:
        raise SystemExit('Choose codex, claude, or opencode')
    if capture not in ['', 'direct', 'gateway'] or (capture == 'direct' and client != 'opencode'):
        raise SystemExit('Direct capture supports OpenCode; gateway capture supports every test client')
    if not sys.stdin.isatty() or not sys.stdout.isatty():
        raise SystemExit('Arena interactive tests require a real terminal')
    ready = json.loads((RUN.RUNTIME / 'gateway-ready.json').read_text())
    status, _, body = RUN.request(ready['origin'] + '/__test/apply',
        body={'client': client, 'label': 'arena'})
    if status != 200:
        raise SystemExit('Isolated ModelDeck apply failed: ' + str(status))
    work = RUN.RUNTIME / 'workspaces' / client / ('arena-' + capture if capture else 'arena')
    work.mkdir(parents=True, exist_ok=True)
    subprocess.run(['git', 'init', '--quiet', str(work)], check=True)
    RUN.save(work / 'fixture.json', {'numbers': [17, 29, 43], 'case': client + '/arena-interactive'})
    (work / 'result.txt').write_text('')
    env = RUN.credentials()
    session = str(uuid.uuid4())
    base_command = RUN.client_command(client, 'arena', work, session, env)
    prompt = ('Use tools to read fixture.json in this workspace. Sum its numbers and write only '
        'the integer to result.txt. Finish with MD_TEST_OK. Do not create subagents or '
        'read files outside this workspace.')
    if client == 'opencode':
        if capture:
            config_path = RUN.RUNTIME / 'home/.config/opencode/opencode.json'
            config = json.loads(config_path.read_text())
            proxy = json.loads((RUN.RUNTIME / ('opencode-' + capture + '-proxy.json')).read_text())
            if capture == 'direct':
                source = json.loads((RUN.USER / '.config/opencode/opencode.json').read_text())
                config['provider'] = {'arena': source['provider']['arena']}
                config['provider']['arena']['options']['baseURL'] = proxy['origin']
                config['model'] = 'arena/coding-router-preview'
                config['permission'] = {'*': 'deny', 'read': 'allow', 'edit': 'allow', 'write': 'allow'}
                config['share'] = 'disabled'
                config['autoupdate'] = False
            else:
                config['provider']['modeldeck']['options']['baseURL'] = proxy['origin']
            RUN.save(config_path, config)
        command = [str(RUN.USER / '.opencode/bin/opencode'), str(work), '--pure', '--mini',
            '--no-replay', '--prompt', prompt]
    elif client == 'claude':
        if capture:
            proxy = json.loads((RUN.RUNTIME / 'claude-gateway-proxy.json').read_text())
            config_path = RUN.RUNTIME / 'home/.claude/settings.json'
            config = json.loads(config_path.read_text())
            config['env']['ANTHROPIC_BASE_URL'] = proxy['origin']
            RUN.save(config_path, config)
            env['ANTHROPIC_BASE_URL'] = proxy['origin']
        command = [str(RUN.USER / '.local/bin/claude'), '--bare', '--ax-screen-reader',
            '--tools', 'Read,Edit', '--allowedTools', 'Read,Edit', '--permission-mode', 'acceptEdits',
            '--settings', str(RUN.RUNTIME / 'home/.claude/settings.json'), '--setting-sources', '',
            '--strict-mcp-config', '--mcp-config', '{"mcpServers":{}}', '--disable-slash-commands',
            '--session-id', session, prompt]
    else:
        # Reuse the already-tested native config overrides, but run the real TUI.
        command = base_command[:base_command.index('exec')]
        import tomllib
        personal = tomllib.loads((RUN.USER / '.codex/config.toml').read_text())
        command += ['-c', 'features.apps=false', '-c', 'plugins={}']
        for name in personal.get('mcp_servers', {}):
            command += ['-c', 'mcp_servers.' + name + '.enabled=false']
        command += ['-c', 'projects=' + RUN.toml_value({str(work): {'trust_level': 'trusted'}})]
        command += ['-c', 'sandbox_mode="workspace-write"', '-c',
                    'sandbox_workspace_write.writable_roots=' + RUN.toml_value([str(work)])]
        if capture:
            proxy = json.loads((RUN.RUNTIME / 'codex-gateway-proxy.json').read_text())
            command += ['-c', 'model_providers.modeldeck.base_url=' + RUN.toml_value(proxy['origin'])]
        command += ['-c', 'history.persistence="none"', '--no-alt-screen', '--cd', str(work), prompt]
    for name in list(env):
        if name.endswith('_OR_KEY') or name.startswith('MD_TEST_'):
            env.pop(name)
    env['TERM'] = 'xterm-256color'
    RUN.save(RUN.RUNTIME / (client + ('-' + capture if capture else '') + '-interactive.json'), {'client': client, 'mode': 'interactive',
        'work': str(work), 'requestedSessionId': session, 'gatewayRecordStart': len(RUN.records()),
        'configurationOnlyContainsGatewayToken': capture != 'direct'})
    os.chdir(work)
    os.execvpe(command[0], command, env)


if __name__ == '__main__':
    main()
