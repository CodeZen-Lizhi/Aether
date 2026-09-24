"""Read-only timeout audit; persist only allowlisted operational metadata."""
import pathlib,sqlite3,json,collections,re,datetime
root=pathlib.Path.home()/'Library/Application Support/com.aether.desktop'
c=sqlite3.connect((root/'aether.db').as_uri()+'?mode=ro',uri=True);c.row_factory=sqlite3.Row
usage={r['request_id']:dict(r) for r in c.execute('select request_id,request_type,status,status_code,first_byte_time_ms,response_time_ms,model,created_at_unix_ms from usage')}
candidates=[dict(r) for r in c.execute("select id,request_id,status,error_type,latency_ms,started_at,finished_at from request_candidates where error_type='local_stream_candidate_watchdog_timeout' or error_type='stream_failover_budget_exhausted' or lower(error_message) like '%timeout%' or lower(error_message) like '%timed out%'")]
byid={r['id']:r for r in candidates};requests={r['request_id'] for r in candidates};eventcounts=collections.Counter();errcounts=collections.Counter();idle=[];paths={}
fields=['event_name','provider_name','model_name','status_code','upstream_ttfb_ms','upstream_elapsed_ms','timeout_ms','elapsed_ms','provider_bytes','client_bytes','last_upstream_frame_elapsed_ms','last_client_chunk_elapsed_ms','plan_kind','execution_mode']
for p in sorted((root/'logs').glob('*.log')):
 for n,line in enumerate(p.open(),1):
  try:d=json.loads(line)
  except:continue
  f=d.get('fields',{}); rid=f.get('trace_id')
  if rid not in requests:continue
  event=f.get('event_name','');cid=f.get('candidate_id','');m=re.search(r'[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}',str(cid));cid=m.group() if m else cid
  if event=='gateway_request_accepted':paths[rid]=f.get('path')
  if cid in byid:
   record={k:f[k] for k in fields if k in f};record.update(timestamp=d.get('timestamp'),file=p.name,line=n)
   byid[cid].setdefault('events',[]).append(record);eventcounts[event]+=1
  if event=='stream_execution_runtime_unavailable':errcounts[str(f.get('error'))[:220]]+=1
  if event=='stream_execution_upstream_idle':idle.append({k:f.get(k) for k in ['trace_id','candidate_id',*fields]})
for x in candidates:x['usage']=usage.get(x['request_id']);x['path']=paths.get(x['request_id'])
watch=[x for x in candidates if x['error_type']=='local_stream_candidate_watchdog_timeout'];withheaders=[]
for x in watch:
 for e in x.get('events',[]):
  if e['event_name']=='upstream_response_headers_received':withheaders.append((x,e));break
summary={'usage_rows':len(usage),'watchdog_attempts':len(watch),'watchdog_requests':len({x['request_id'] for x in watch}),'watchdog_with_usage':len({x['request_id'] for x in watch if x['usage']}),'watchdog_with_headers':len(withheaders),'watchdog_200_headers_lt10s':sum(e.get('status_code')==200 and e.get('upstream_ttfb_ms',999999)<10000 for x,e in withheaders),'watchdog_200_headers_lt30s':sum(e.get('status_code')==200 and e.get('upstream_ttfb_ms',999999)<30000 for x,e in withheaders),'watchdog_path_counts':dict(collections.Counter(x['path'] for x in watch)),'candidate_event_counts':dict(eventcounts)}
report={'summary':summary,'candidates':candidates,'idle_events':idle}
out=pathlib.Path(__file__).parent/'audit.json';out.write_text(json.dumps(report,ensure_ascii=False,indent=2))
print(json.dumps(summary,ensure_ascii=False,indent=2));print('transport_error_classes',json.dumps(errcounts,ensure_ascii=False));print('idle_events',json.dumps(idle,ensure_ascii=False))
print('recent ordinary chat timeouts with early 200')
examples=sorted([(x,e) for x,e in withheaders if x.get('usage',{}) and x['usage']['request_type']=='chat' and e.get('status_code')==200 and e.get('upstream_ttfb_ms',999999)<10000],key=lambda z:z[0]['started_at'] or 0,reverse=True)[:12]
for x,e in examples:print(json.dumps({'request_id':x['request_id'],'candidate_id':x['id'],'header_ms':e['upstream_ttfb_ms'],'latency_ms':x['latency_ms'],'timestamp':e['timestamp'],'provider':e['provider_name'],'usage':x['usage'],'events':[i['event_name'] for i in x['events']]},ensure_ascii=False))
