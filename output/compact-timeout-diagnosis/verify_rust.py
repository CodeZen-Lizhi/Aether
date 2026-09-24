"""Focused gateway checks for this change, run sequentially against one target dir."""
import json
import re
from pathlib import Path
import subprocess
import time
root=Path(__file__).resolve().parent
out=root/('rust-checks-'+str(time.time_ns()))
out.mkdir()
filters=['stream_response_timeout','scheduler_failover::http_failures','delivered_sse_text_is_not_replayed_after_upstream_disconnect','scheduler_failover::target_admission','stream_candidate_watchdog','direct_stream_execution_runtime','gateway_admin_usage_retry_lifecycle_sqlite','gateway_derives_legacy_admin_usage','gateway_preserves_admin_usage_explicit_image_failure','observability::usage::summary_routes::tests','observability::monitoring::tests::trace','stream_total_timeout','tunnel::embedded::local_relay::tests','responses::route_smoke']
results=[]
for index,name in enumerate(filters):
 print('RUN',name,flush=True)
 proc=subprocess.run(['cargo','test','-p','aether-gateway','--lib',name],text=True,capture_output=True,timeout=300)
 log=out/(str(index)+'-'+name.replace('::','_')+'.log')
 log.write_text(proc.stdout+'\n'+proc.stderr)
 summary=[line for line in proc.stdout.splitlines() if line.startswith('test result:')]
 passed=proc.returncode==0 and bool(summary) and int(re.search(r'(\d+) passed;', summary[-1]).group(1)) > 0
 result={'filter':name,'passed':passed,'summary':summary,'log':str(log)}
 results.append(result)
 print(json.dumps(result),flush=True)
 (out/'results.json').write_text(json.dumps(results,indent=2))
 if not passed:
  print(proc.stdout[-5000:]+proc.stderr[-2000:],flush=True)
print('RUST_REPORT',out/'results.json',flush=True)
raise SystemExit(0 if all(r['passed'] for r in results) else 1)
