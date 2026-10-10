#!/usr/bin/env python3
"""Run the production protocol gateway with only the two authorized free models."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import ssl
import subprocess
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[2]
SCRATCH = ROOT / '.build/cursor-direct'
RUNTIME = SCRATCH / '.runtime/gateway'
MODELS = {'md-nemotron': 'nvidia/nemotron-3-ultra-550b-a55b:free',
          'md-inkling': 'thinkingmachines/inkling:free'}


def prepare():
    assert os.environ.get('OC_OR_KEY'), 'Authorized OpenCode OpenRouter key is unavailable'
    RUNTIME.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(RUNTIME, 0o700)
    context = ssl.create_default_context(cafile='/etc/ssl/cert.pem')
    with urllib.request.urlopen('https://openrouter.ai/api/v1/models', context=context, timeout=30) as response:
        catalog = {item['id']: item for item in json.load(response)['data']}
    selected = {alias: catalog[model] for alias, model in MODELS.items()}
    for model in selected.values():
        assert float(model['pricing']['prompt']) == 0 and float(model['pricing']['completion']) == 0
    (SCRATCH / 'free-model-metadata.json').write_text(json.dumps(selected, indent=2) + '\n')
    provider = {'id': 'openrouter', 'name': 'Authorized test OpenRouter',
        'baseUrl': 'https://openrouter.ai/api/v1', 'protocol': 'openai-chat', 'apiKey': '', 'keyEnv': 'OC_OR_KEY'}
    token = 'md-local-' + uuid.uuid4().hex
    endpoint = {'id': 'cursor-local', 'name': 'Cursor local test', 'token': token,
        'protocols': ['openai-chat', 'openai-responses', 'anthropic'], 'publicUrl': '',
        'routes': {alias: {'modelId': alias, 'effort': ''} for alias in MODELS}}
    models = {alias: {'id': alias, 'providerId': 'openrouter', 'upstreamId': model,
        'name': selected[alias]['name'], 'context': selected[alias]['context_length'], 'output': 2048,
        'reasoning': [], 'tools': True, 'images': False} for alias, model in MODELS.items()}
    executable = json.loads((SCRATCH / 'download.json').read_text())['entry']
    binding = {'endpointId': endpoint['id'], 'model': 'md-nemotron', 'utilityModel': 'md-nemotron',
        'executable': executable}
    store = {'schemaVersion': 1, 'revision': 1, 'providers': {'openrouter': provider}, 'models': models,
        'endpoints': {endpoint['id']: endpoint}, 'bindings': {'cursor-agent': binding},
        'activeBindings': {'cursor-agent': binding},
        'migration': {'importedProviders': 0, 'importedModels': 0, 'retained': []}}
    (RUNTIME / 'initial-store.json').write_text(json.dumps(store, indent=2) + '\n')
    (RUNTIME / 'entry-token').write_text(token)
    protected = [Path('/Users/vita/.codex/auth.json'), Path('/Users/vita/.codex/config.toml'),
        Path('/Users/vita/.cursor/cli-config.json'), Path('/Users/vita/.config/opencode/opencode.json')]
    before = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else None for p in protected}
    (RUNTIME / 'protected-before.json').write_text(json.dumps(before, indent=2) + '\n')
    print(json.dumps({'models': MODELS, 'pricesVerifiedZero': True, 'cursorAccountUsed': False}))


def serve():
    env = dict(os.environ)
    env.update({'CC_SWITCH_TEST_HOME': str(RUNTIME / 'home'),
        'XDG_CONFIG_HOME': str(RUNTIME / 'home/.config'), 'MODELDECK_LIVE_TEST_ROOT': str(RUNTIME),
        'MODELDECK_LIVE_TEST_PORT': '0', 'CARGO_HOME': str(ROOT / '.build/cargo-home'),
        'DYLD_FALLBACK_LIBRARY_PATH': '/opt/homebrew/Cellar/z3/4.16.0/lib'})
    cmd = ['cargo', 'test', '--lib', 'deck::native::tests::real_harness_gateway_fixture',
        '--', '--ignored', '--exact', '--nocapture']
    result = subprocess.run(cmd, env=env, cwd=ROOT / 'src-tauri')
    raise SystemExit(result.returncode)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=['prepare', 'serve'])
    args = parser.parse_args()
    globals()[args.command]()
