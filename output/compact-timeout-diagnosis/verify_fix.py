"""Run synthetic public HTTP/SQLite regressions without real upstream traffic."""
import concurrent.futures,json,os,pathlib,subprocess,time,sys
root=pathlib.Path(__file__).resolve().parent
binary=pathlib.Path(sys.argv[1]).resolve()
run=root/('verification-'+str(time.time_ns()));run.mkdir()
cases={
'chat_snapshot':dict(DIAG_FIRST_TIMEOUT='1',DIAG_BUDGET_MS='2500',DIAG_OUTPUT_DELAY='0.1',DIAG_AFTER_OUTPUT_DELAY='3',DIAG_OUTPUT_KIND='text_done',DIAG_OPERATION='chat',DIAG_EXPECT_SUCCESS='1'),
'compact_snapshot':dict(DIAG_FIRST_TIMEOUT='1',DIAG_BUDGET_MS='2500',DIAG_OUTPUT_DELAY='0.1',DIAG_AFTER_OUTPUT_DELAY='3',DIAG_OUTPUT_KIND='compaction_done',DIAG_EXPECT_SUCCESS='1'),
'delta_slow_finish':dict(DIAG_FIRST_TIMEOUT='1',DIAG_BUDGET_MS='2500',DIAG_OUTPUT_DELAY='0.1',DIAG_AFTER_OUTPUT_DELAY='3',DIAG_OPERATION='chat',DIAG_EXPECT_SUCCESS='1'),
'candidate_timeout':dict(DIAG_EXPECT_FAILED_TERMINAL='1'),
'logical_budget_timeout':dict(DIAG_FIRST_TIMEOUT='5',DIAG_BUDGET_MS='1800',DIAG_OUTPUT_DELAY='3',DIAG_EXPECT_FAILED_TERMINAL='1'),
'upstream_eof_after_output':dict(DIAG_FIRST_TIMEOUT='1',DIAG_OUTPUT_DELAY='0.1',DIAG_AFTER_OUTPUT_DELAY='1.2',DIAG_OPERATION='chat',DIAG_BROKEN_STREAM='1',DIAG_EXPECT_SUCCESS='1'),
}
if os.environ.get('DIAG_VERIFY_LONG')=='1':
 for operation,kind in [('chat','text_done'),('compact','compaction_done')]:
  cases['long_'+operation]=dict(DIAG_FIRST_TIMEOUT='30',DIAG_BUDGET_MS='90000',DIAG_OUTPUT_DELAY='0.1',DIAG_AFTER_OUTPUT_DELAY='95',DIAG_OUTPUT_KIND=kind,DIAG_OPERATION=operation,DIAG_EXPECT_SUCCESS='1',DIAG_CLIENT_TIMEOUT='120')
def verify(item):
 name,settings=item
 env={k:v for k,v in os.environ.items() if not k.startswith('DIAG_')}
 env.update(settings,DIAG_GATEWAY_BINARY=str(binary),DIAG_ONLY_GATEWAY='1')
 result=subprocess.run([sys.executable,str(root/'reproduce.py')],env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=130)
 (run/(name+'.log')).write_text(result.stdout)
 artifact=next((line.removeprefix('REPORT ') for line in result.stdout.splitlines() if line.startswith('REPORT ')),None)
 row={'case':name,'passed':result.returncode==0,'report':artifact,'log':str(run/(name+'.log'))}
 if artifact:
  report=json.loads(pathlib.Path(artifact).read_text());row['gateway']=next(r for r in report['results'] if r['route']=='gateway');row['usage']=report['usage']
 print(json.dumps(row,ensure_ascii=False),flush=True)
 return row
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:results=list(pool.map(verify,cases.items()))
(run/'summary.json').write_text(json.dumps(results,ensure_ascii=False,indent=2))
print('SUMMARY',run/'summary.json',flush=True)
sys.exit(0 if all(x['passed'] for x in results) else 1)
