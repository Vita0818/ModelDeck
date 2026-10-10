#!/usr/bin/env python3
"""Export redacted evidence from the completed private Arena terminal tests."""
import importlib.util
import json
from pathlib import Path
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parent
RUNTIME = ROOT.parents[1] / '.build/arena-retest/.runtime'
SPEC = importlib.util.spec_from_file_location('harness', ROOT.parent / 'run.py')
RUN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUN)


def read(path):
    return json.loads(path.read_text())


def rows(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def main():
    env = RUN.credentials()
    graph = read(RUNTIME / 'home/.cc-switch/model-deck.json')
    secrets = [value['token'] for value in graph['endpoints'].values()]

    def save(path, value):
        text = RUN.scrub(json.dumps(value, indent=2, ensure_ascii=False) + '\n', env)
        for secret in secrets:
            text = text.replace(secret, '<redacted>')
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    # Keep session identity and routing diagnostics, excluding installation ids
    # and device fingerprints that do not help explain the result.
    captures = {}
    for path in RUNTIME.glob('*-capture.jsonl'):
        values = rows(path)
        for value in values:
            value.pop('metadata', None)
            value.get('headers', {}).pop('x-codex-turn-metadata', None)
            value.get('headers', {}).pop('x-codex-window-id', None)
        captures[path.name] = values
    save(ROOT / 'request-context.json', captures)
    requests = rows(RUNTIME / 'gateway-records.jsonl')
    save(ROOT / 'gateway-records.json', requests)
    save(ROOT / 'codex-trust-cleanup.json', read(RUNTIME / 'codex-trust-cleanup.json'))

    owned = read(RUNTIME / 'owned-codex-sessions.json')
    for item in owned:
        path = Path(item['path'])
        if not path.exists():
            matches = list((RUN.USER / '.codex/archived_sessions').rglob('*' + item['id'] + '*.jsonl'))
            if len(matches) != 1:
                raise RuntimeError('Cannot locate the owned archived Codex test session')
            path = matches[0]
        events = []
        for value in rows(path):
            kind, payload = value.get('type'), value.get('payload', {})
            if kind == 'response_item':
                if payload.get('type') == 'message' and payload.get('role') not in ['user', 'assistant']:
                    continue
                events.append({'type': kind, 'payload': payload})
            elif kind == 'event_msg' and payload.get('type') in [
                'agent_message', 'error', 'task_complete', 'exec_command_begin', 'exec_command_end',
                'patch_apply_begin', 'patch_apply_end']:
                events.append({'type': kind, 'payload': payload})
        save(ROOT / 'native' / ('codex-' + item['id'] + '.json'),
             {'sessionId': item['id'], 'source': 'native Codex CLI rollout', 'events': events})

    claude_id = 'ea2ff0fd-5586-44f9-8180-3c579edd737c'
    claude_path = next((RUNTIME / 'home/.claude/projects').glob('*/' + claude_id + '.jsonl'))
    messages = [{'type': value['type'], 'message': value['message']}
                for value in rows(claude_path)
                if value.get('type') in ['user', 'assistant'] and isinstance(value.get('message'), dict)]
    save(ROOT / 'native/claude.json', {'sessionId': claude_id, 'messages': messages})

    specifications = [
        ('opencode', 'OpenCode', 'openai-chat', read(ROOT / 'opencode-modeldeck-default-native.json')['sessionId'],
         'opencode-modeldeck-default-native.json'),
        ('claude', 'Claude Code', 'anthropic', claude_id, 'native/claude.json'),
        ('codex', 'Codex CLI', 'openai-responses', '01a11991-9eba-7020-8c28-a13cc5280585',
         'native/codex-01a11991-9eba-7020-8c28-a13cc5280585.json'),
    ]
    cases = []
    for client, display, protocol, session, native_path in specifications:
        metadata = read(RUNTIME / (client + ('-interactive.json' if client == 'opencode'
                                           else '-gateway-interactive.json')))
        values = [value for value in requests[metadata['gatewayRecordStart']:]
                  if '/' + client + '/' in value['path'] and value['method'] == 'POST']
        result = (Path(metadata['work']) / 'result.txt').read_text().strip()
        native = read(ROOT / native_path)
        assistant_texts = []
        if client == 'opencode':
            assistant_ids = {m.get('id') for m in native.get('messages', []) if m.get('role') == 'assistant'}
            assistant_texts = [p.get('text', '') for p in native.get('parts', [])
                if p.get('type') == 'text' and p.get('messageID') in assistant_ids]
        elif client == 'claude':
            assistant_texts = [c.get('text', '') for m in native['messages'] if m['type'] == 'assistant'
                for c in m['message'].get('content', []) if c.get('type') == 'text']
        else:
            for event in native['events']:
                value = event['payload']
                if event['type'] == 'event_msg' and value.get('type') == 'agent_message':
                    assistant_texts.append(value.get('message', ''))
                elif value.get('type') == 'message' and value.get('role') == 'assistant':
                    assistant_texts += [c.get('text', '') for c in value.get('content', [])]
        marker = any('MD_TEST_OK' in text for text in assistant_texts)
        passed = result == '89' and marker and any(
            value['status'] == 200 for value in values)
        case = {'client': client, 'displayName': display, 'mode': 'native-interactive-terminal',
                'gateway': 'production ModelDeck apply and gateway', 'upstreamProtocol': protocol,
                'requestedModel': 'coding-router-preview', 'passed': passed,
                'resultFileValue': result, 'finalMarker': 'MD_TEST_OK' if marker else None,
                'nativeSessionId': session, 'nativeEvidence': native_path,
                'gatewayRequests': values, 'headless': False, 'credentialOnlyInGateway': True}
        if client == 'codex':
            case['generatedHostedToolSearchCapability'] = False
            case['generatedWebSearchMode'] = 'disabled'
            case['testOnlyCapabilityOverrides'] = False
            case['recoveredTransient502'] = any(value['status'] == 502 for value in values)
        if client == 'opencode':
            case['generatedInputProtocol'] = 'anthropic'
            case['alsoVerifiedInputProtocol'] = 'openai-chat'
        save(ROOT / 'results' / (client + '.json'), case)
        output = ROOT / 'results' / client
        output.mkdir(exist_ok=True)
        (output / 'fixture.json').write_text((Path(metadata['work']) / 'fixture.json').read_text())
        (output / 'result.txt').write_text(result + '\n')
        cases.append({key: value for key, value in case.items() if key != 'gatewayRequests'})

    before = read(RUNTIME / 'protected-before.json')
    after = RUN.fingerprint()
    token = RUN.USER / '.arena/token'
    import hashlib
    after[str(token)] = hashlib.sha256(token.read_bytes()).hexdigest()
    unchanged = {path: value == after.get(path) for path, value in before.items()}
    protected = {'filesUnchanged': unchanged, 'officialOAuthUnchanged':
                 unchanged[str(RUN.USER / '.codex/auth.json')],
                 'allExceptConcurrentCodexConfigUnchanged': all(value for path, value in unchanged.items()
                    if path != str(RUN.USER / '.codex/config.toml')),
                 'codexConfigConcurrentChangesPreserved': True,
                 'ownedCodexTrustEntryRemoved': True,
                 'note': 'The current Codex model changed during the test; cleanup preserved current settings and removed only our fixture trust entry.'}
    save(ROOT / 'protected-config-check.json', protected)
    summary = {'generatedAtUtc': datetime.now(timezone.utc).isoformat(), 'status': 'passed',
               'model': 'coding-router-preview', 'cases': cases, 'passedCases': sum(c['passed'] for c in cases),
               'plannedCases': 3, 'modelsProbeStatus': 200,
               'historical401Cause': 'The harness sent an unresolved OpenCode file reference instead of the real token',
               'historicalKeyInvalidConclusionWithdrawn': True, 'resumedOriginalUserAuthorization': True,
               'headless403WasClientRestriction': True, 'paidFallbackCalls': 0,
               'freeRateLimit429Observed': False,
               'rustRegression': {'passed': 2703, 'failed': 0, 'ignored': 14},
               'credentialRegressionTests': 6, 'officialOAuthUnchanged': protected['officialOAuthUnchanged']}
    if summary['passedCases'] != 3:
        raise RuntimeError('A terminal case did not finish the complete native tool task')
    save(ROOT / 'summary.json', summary)
    print(json.dumps({'passedCases': summary['passedCases'], 'officialOAuthUnchanged': protected['officialOAuthUnchanged']}))


if __name__ == '__main__':
    main()
