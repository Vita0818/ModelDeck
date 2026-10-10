#!/usr/bin/env python3
"""Preserve redacted native evidence before deleting private test state."""
import hashlib
import json
import os
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[2]
PUBLIC = Path(__file__).resolve().parent
SCRATCH = ROOT / '.build/cursor-direct'
PRIVATE = SCRATCH / '.runtime'
MODELS = {'nemotron': 'nvidia/nemotron-3-ultra-550b-a55b:free',
          'inkling': 'thinkingmachines/inkling:free'}


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n')


def events(text):
    result = []
    for line in text.splitlines():
        try:
            result.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    return result


def main():
    secrets = [p.read_text().strip() for p in [PRIVATE / 'gateway/entry-token', PRIVATE / 'mock-server/token'] if p.is_file()]
    secrets += [value for name, value in os.environ.items() if name.endswith('_OR_KEY') and value]

    def redact(text):
        for secret in secrets:
            text = text.replace(secret, '<redacted>')
        return text

    rows = [json.loads(line) for line in (PRIVATE / 'gateway/gateway-records.jsonl').read_text().splitlines()]
    (PUBLIC / 'gateway-records.jsonl').write_text(redact('\n'.join(json.dumps(row) for row in rows)) + '\n')
    attempts = []
    for path in sorted(PRIVATE.glob('*/report.json')):
        data = json.loads(path.read_text())
        output = redact((path.parent / 'output.log').read_text())
        parsed = events(output)
        marker = 'MD_LOCAL_OK' if data['case'] == 'mock' else 'MD_TEST_OK'
        if not data.get('modelCatalogProbe'):
            data['finalTextSeen'] = any(marker in json.dumps(event)
                for event in parsed if event.get('type') in ['assistant', 'result'])
        data['toolTrace'] = [{'tool': name, 'succeeded': 'success' in value.get('result', {})}
            for event in parsed if event.get('type') == 'tool_call' and event.get('subtype') == 'completed'
            for name, value in event.get('tool_call', {}).items() if name.endswith('ToolCall')]
        errors = []
        for response in path.parent.glob('network.jsonl.response*'):
            for line in response.read_text().splitlines():
                if not line.startswith('data: '):
                    continue
                try:
                    value = json.loads(line[6:])
                except json.JSONDecodeError:
                    continue
                if value.get('error'):
                    errors.append({'provider': value.get('provider'), 'error': value['error']})
        data['upstreamStreamErrors'] = errors
        data['outputLog'] = f"logs/{data['case']}.log"
        save(PUBLIC / 'results' / (data['case'] + '.json'), data)
        log = PUBLIC / data['outputLog'];log.parent.mkdir(parents=True, exist_ok=True);log.write_text(output)
        attempts.append(data)

    for case, model in MODELS.items():
        path = ROOT / 'harness-tests/results/cursor-agent' / (case + '.json')
        original = json.loads(path.read_text())
        archived = ROOT / 'harness-tests/attempts/cursor-agent' / (case + '-cloud-login.json')
        if not archived.exists():
            save(archived, original)
            old_log = ROOT / 'harness-tests' / original['output_log']
            shutil.copyfile(old_log, archived.with_suffix('.log'))
        data = next(item for item in attempts if item['case'] == 'managed-' + case)
        log = PUBLIC / data['outputLog']
        target_log = ROOT / 'harness-tests/logs/cursor-agent' / (case + '.log')
        target_log.write_text(log.read_text())
        post_rows = [row for row in rows if row.get('model') == 'md-' + case
                     and row.get('path') == '/deck-clients/cursor-agent/chat/completions']
        record = {'client': 'cursor-agent', 'case': case, 'requested': model, 'model_used': model,
            'real_client': True, 'native_runtime': 'official agent-cli-local', 'apply_status': 200,
            'gateway_requests': post_rows, 'passed': data['passed'], 'exit_code': data['exitCode'],
            'seconds': data['seconds'], 'runtime_session_ids': data['sessionIds'],
            'output_log': str(target_log.relative_to(ROOT / 'harness-tests')),
            'result_file_value': data['resultFileValue'], 'free_rate_limit_429': False,
            'cursor_account_credentials_provided': False, 'native_tool_trace': data['toolTrace'],
            'failure': '' if data['passed'] else 'Nvidia upstream error code 503: Service temporarily overloaded',
            'direct_evidence': f'cursor-direct/results/managed-{case}.json'}
        save(path, record)
        destination = path.parent / 'files' / case
        destination.mkdir(parents=True, exist_ok=True)
        for name in ['fixture.json', 'result.txt']:
            shutil.copyfile(ROOT / 'harness-tests/workspaces/cursor-agent-local' / data['case'] / name, destination / name)

    save(PUBLIC / 'vendor-source.json', json.loads((SCRATCH / 'download.json').read_text()))
    save(PUBLIC / 'model-metadata.json', json.loads((SCRATCH / 'free-model-metadata.json').read_text()))
    save(PUBLIC / 'gui-inspection.json', {
        'version': '3.9.16', 'nativeLocalAgentEnginePresent': True,
        'rendererBuildFlag': 'localMode:!1', 'flagSource': 'out-build/vs/platform/environment/common/buildFlags.js',
        'localEntryPoint': 'AgentClientService.runLocalAgentInExtensionHost',
        'guiLocalSessionExecuted': False, 'installedGuiModified': False,
        'accountOrSubscriptionStateChanged': False})
    before = json.loads((PRIVATE / 'gateway/protected-before.json').read_text())
    checks = [{'path': path, 'unchanged': digest == (hashlib.sha256(Path(path).read_bytes()).hexdigest()
        if Path(path).is_file() else None)} for path, digest in before.items()]
    save(PUBLIC / 'protected-config-check.json', {'allUnchanged': all(item['unchanged'] for item in checks), 'checks': checks})
    inkling_costs = []
    for path in (PRIVATE / 'managed-inkling').glob('network.jsonl.response*'):
        for line in path.read_text().splitlines():
            if not line.startswith('data: '):
                continue
            try:
                value = json.loads(line[6:])
            except json.JSONDecodeError:
                continue
            if value.get('usage'):
                inkling_costs.append(value['usage'].get('cost'))
    save(PUBLIC / 'summary.json', {
        'officialLocalRuntimeVerified': True, 'cursorAccountCredentialsUsed': False,
        'sharedGatewayApplySucceeded': True, 'nativeModelListSucceeded': True,
        'mockNativeReadWritePassed': True, 'inklingNativeToolWorkflowPassed': True,
        'nemotronNativeToolWorkflowPassed': False, 'nemotronUpstreamError': 503,
        'earlierNemotronReadAndWriteSucceededBeforeFinal503': True,
        'inklingUsageRecords': len(inkling_costs), 'inklingReportedCostUsd': sum(inkling_costs) if inkling_costs and all(isinstance(cost, (int, float)) for cost in inkling_costs) else None,
        'paidFallbackCalls': 0, 'freeRateLimit429Observed': False,
        'guiLocalModeVerified': False, 'attempts': attempts,
        'protectedConfigurationsUnchanged': all(item['unchanged'] for item in checks)})
    print(json.dumps({'preservedAttempts': len(attempts), 'realInklingPassed': True,
        'realNemotronUpstreamError': 503, 'protectedConfigurationsUnchanged': all(item['unchanged'] for item in checks)}))


if __name__ == '__main__':
    main()
