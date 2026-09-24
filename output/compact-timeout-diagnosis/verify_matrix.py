"""Run isolated real-gateway timeout acceptance cases; never calls a live upstream."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent
BINARY = Path(sys.argv[1]).resolve()
assert BINARY.is_file(), BINARY
OUT = ROOT / ('acceptance-' + str(time.time_ns()))
OUT.mkdir()
BASE = {
    'DIAG_ONLY_GATEWAY': '1', 'DIAG_SKIP_VALIDATION': '1',
    'DIAG_FIRST_TIMEOUT': '1', 'DIAG_BUDGET_MS': '1800',
    'DIAG_TOTAL_TIMEOUT_MS': '5000', 'DIAG_HEADERS_DELAY': '0.1',
    'DIAG_OUTPUT_DELAY': '0', 'DIAG_BODY_DELAY': '3',
    'DIAG_SETUP_ENABLED': '0', 'DIAG_OUTPUT_KIND': 'compaction_done',
    'DIAG_GATEWAY_BINARY': str(BINARY),
}
CASES = [
    ('compact_headers_then_delayed_body', {}, 'completed', False),
    ('chat_headers_then_delayed_body', {'DIAG_OPERATION': 'chat', 'DIAG_OUTPUT_KIND': 'delta'}, 'completed', False),
    ('compact_startup_then_silence', {'DIAG_BODY_DELAY': '0', 'DIAG_SETUP_ENABLED': '1', 'DIAG_OUTPUT_DELAY': '3'}, 'completed', False),
    ('no_headers_first_timeout', {'DIAG_HEADERS_DELAY': '3', 'DIAG_BODY_DELAY': '0'}, 'failed', False),
    ('headers_only_total_timeout', {'DIAG_TOTAL_TIMEOUT_MS': '1500'}, 'failed', False),
    ('startup_then_total_timeout', {'DIAG_BODY_DELAY': '0', 'DIAG_SETUP_ENABLED': '1', 'DIAG_OUTPUT_DELAY': '3', 'DIAG_TOTAL_TIMEOUT_MS': '1500'}, 'failed', False),
    ('chat_output_then_total_timeout', {'DIAG_BODY_DELAY': '0', 'DIAG_TOTAL_TIMEOUT_MS': '1500', 'DIAG_AFTER_OUTPUT_DELAY': '3', 'DIAG_OPERATION': 'chat', 'DIAG_OUTPUT_KIND': 'delta'}, 'failed', True),
    ('compact_item_then_total_timeout', {'DIAG_BODY_DELAY': '0', 'DIAG_TOTAL_TIMEOUT_MS': '1500', 'DIAG_AFTER_OUTPUT_DELAY': '3'}, 'failed', True),
    ('chat_output_then_broken_stream', {'DIAG_BODY_DELAY': '0', 'DIAG_OPERATION': 'chat', 'DIAG_OUTPUT_KIND': 'delta', 'DIAG_BROKEN_STREAM': '1'}, 'failed', True),
]
results=[]
for name, patch, terminal, partial in CASES:
    env=os.environ.copy()
    for key in list(env):
        if key.startswith('DIAG_'):
            del env[key]
    env.update(BASE)
    env.update(patch)
    proc=subprocess.run([sys.executable, str(ROOT/'verify_scenario.py')], env=env, text=True, capture_output=True, timeout=35)
    (OUT/(name+'.log')).write_text(proc.stdout+'\n'+proc.stderr)
    refs=[line[7:].strip() for line in proc.stdout.splitlines() if line.startswith('REPORT ')]
    errors=[]
    report={}
    if proc.returncode or not refs:
        errors.append('scenario process/report failed')
    else:
        report=json.loads(Path(refs[-1]).read_text())
        row=report['results'][0]
        usage=report['usage']
        if len(usage)!=1 or usage[0]['status']!=terminal:
            errors.append('incorrect persisted request terminal')
        if terminal=='completed':
            if row['status']!=200 or not row['has_output'] or not row['completed_event']:
                errors.append('delayed response did not complete successfully')
            if row['elapsed_ms'] < 2700 or row['elapsed_ms'] >= 5000:
                errors.append('unexpected successful response timing')
        else:
            if row['completed_event']:
                errors.append('failure synthesized completion')
            if partial and not row['has_output']:
                errors.append('expected first output before failure')
            if not partial and row['has_output']:
                errors.append('unexpected output after request deadline')
            if 'total_timeout' in name:
                if not usage or usage[0]['status_code']!=504 or 'total' not in (usage[0]['error_message'] or '').lower():
                    errors.append('total timeout not reported distinctly')
                if not 1200 <= row['elapsed_ms'] <= 2600:
                    errors.append('total deadline not enforced during body lifetime')
                if (partial or name=='startup_then_total_timeout') and (not usage or usage[0]['first_byte_time_ms'] is None):
                    errors.append('observed first body timing lost on total timeout')
                if name=='startup_then_total_timeout':
                    timings=report.get('stream_timings',[])
                    if not timings or timings[-1].get('response_headers_elapsed_ms') is None or timings[-1].get('first_body_elapsed_ms') is None or timings[-1].get('first_effective_output_elapsed_ms') is not None:
                        errors.append('headers/body/effective output milestones not preserved separately')
            if name=='no_headers_first_timeout' and (row['status']!=504 or row['elapsed_ms']>=2500):
                errors.append('no-headers request not stopped by first timeout')
        if report.get('candidates') and any(c['status']=='pending' for c in report['candidates']):
            errors.append('candidate left pending after request end')
    result={'case':name,'passed':not errors,'errors':errors,'report':refs[-1] if refs else None}
    if report:
        result['elapsed_ms']=report['results'][0]['elapsed_ms']
        result['http_status']=report['results'][0]['status']
        result['usage']=report['usage']
    results.append(result)
    print(json.dumps(result,ensure_ascii=False),flush=True)
    (OUT/'results.json').write_text(json.dumps(results,ensure_ascii=False,indent=2))
print('MATRIX_REPORT',OUT/'results.json',flush=True)
sys.exit(0 if all(r['passed'] for r in results) else 1)
