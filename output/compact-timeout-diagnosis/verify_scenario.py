"""Local HTTP/SQLite acceptance scenarios for first-response and total deadlines. No live upstream requests."""
import base64, hashlib, hmac, http.client, json, os, pathlib, socket, sqlite3, subprocess, threading, time, uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from cryptography.fernet import Fernet
ROOT=pathlib.Path(__file__).resolve().parent
RUN=ROOT/('run-'+str(time.time_ns())+'-'+uuid.uuid4().hex[:8]); RUN.mkdir()
DB=RUN/'fixture.db'
source=sqlite3.connect('file:/Users/zhenglizhi/Library/Application Support/com.aether.desktop/aether.db?mode=ro',uri=True)
db=sqlite3.connect(DB)
# Copy schema only; never copy user records or credentials.
for sql, in source.execute("select sql from sqlite_master where type='table' and sql is not null and name not like 'sqlite_%'"):
 db.execute(sql)
for table, in source.execute("select name from sqlite_master where type='table' and (name like '%migration%' or name like '%backfill%')"):
 for row in source.execute('select * from "'+table+'"'):
  db.execute('insert into "'+table+'" values ('+','.join('?' for _ in row)+')',row)
source.close()
secret=Fernet.generate_key(); upstream_key='sk-local-only-upstream'; client_key='sk-local-only-client'
encrypted=base64.urlsafe_b64encode(Fernet(secret).encrypt(upstream_key.encode())).decode()
now=int(time.time())
def insert(table,**values):
 values.update(created_at=now,updated_at=now)
 db.execute('insert into '+table+' ('+','.join(values)+') values ('+','.join('?' for _ in values)+')',list(values.values()))
receipts=[]
class Upstream(BaseHTTPRequestHandler):
 protocol_version='HTTP/1.1'
 def log_message(self,*args):pass
 def do_POST(self):
  raw=self.rfile.read(int(self.headers.get('Content-Length','0')))
  payload=json.loads(raw); padding=int(os.environ.get('DIAG_SETUP_PADDING','0')); rec={'padding':padding,'closed_before_output':False,'output_sent':False}; receipts.append(rec)
  time.sleep(float(os.environ.get('DIAG_HEADERS_DELAY','0')))
  self.send_response(200);self.send_header('Content-Type','text/event-stream');self.send_header('Transfer-Encoding','chunked');self.end_headers()
  def send(data):
   self.wfile.write(('%x\r\n'%len(data)).encode()+data+b'\r\n');self.wfile.flush()
  response={'id':'resp_local','object':'response','created_at':now,'model':'gpt-6-astra','status':'in_progress','output':[], 'instructions':'x'*padding}
  setup=('event: response.created\ndata: '+json.dumps({'type':'response.created','response':response})+'\n\n').encode()
  rec['setup_bytes']=len(setup)
  try:
   time.sleep(float(os.environ.get('DIAG_BODY_DELAY','0')))
   if os.environ.get('DIAG_SETUP_ENABLED','1')=='1': send(setup)
   # Upstream stays alive; only client/gateway can close before this output.
   time.sleep(float(os.environ.get('DIAG_OUTPUT_DELAY','0.7')))
   self.connection.setblocking(False)
   try: rec['closed_before_output']=self.connection.recv(1,socket.MSG_PEEK)==b''
   except BlockingIOError:pass
   finally:self.connection.setblocking(True)
   kind=os.environ.get('DIAG_OUTPUT_KIND','delta')
   if kind=='compaction_done':
    event={'type':'response.output_item.done','item':{'type':'compaction','encrypted_content':'LOCAL_OK'}}
   elif kind=='text_done':
    event={'type':'response.output_text.done','text':'LOCAL_OK','output_index':0,'content_index':0,'item_id':'msg_local'}
   else:
    event={'type':'response.output_text.delta','delta':'LOCAL_OK','output_index':0,'content_index':0,'item_id':'msg_local'}
   send(('event: '+event['type']+'\ndata: '+json.dumps(event)+'\n\n').encode())
   rec['early_output_event_sent']=event['type']
   time.sleep(float(os.environ.get('DIAG_AFTER_OUTPUT_DELAY','0')))
   if os.environ.get('DIAG_BROKEN_STREAM')=='1':
    self.close_connection=True
    self.connection.shutdown(socket.SHUT_RDWR)
    return
   response.update(status='completed',output=([{'id':'compact_local','type':'compaction','encrypted_content':'LOCAL_OK'}] if kind=='compaction_done' else [{'id':'msg_local','type':'message','role':'assistant','status':'completed','content':[{'type':'output_text','text':'LOCAL_OK','annotations':[]}]}]),usage={'input_tokens':1,'output_tokens':1,'total_tokens':2})
   send(('event: response.completed\ndata: '+json.dumps({'type':'response.completed','response':response})+'\n\n').encode());rec['output_sent']=True
   self.wfile.write(b'0\r\n\r\n');self.wfile.flush()
  except (BrokenPipeError,ConnectionResetError):rec['closed_before_output']=True
server=ThreadingHTTPServer(('127.0.0.1',0),Upstream);threading.Thread(target=server.serve_forever,daemon=True).start()
insert('users',id='user-local',username='local-fixture',role='admin')
insert('user_sessions',id='session-local',user_id='user-local',client_device_id='local-fixture',refresh_token_hash='local-fixture',last_seen_at=now,expires_at=now+3600)
insert('api_keys',id='client-local',user_id='user-local',key_hash=hashlib.sha256(client_key.encode()).hexdigest(),name='local-client',is_standalone=0,rate_limit=10000)
insert('providers',id='provider-local',name='local-upstream',provider_type='custom',billing_type='free_tier',stream_first_byte_timeout=float(os.environ.get('DIAG_FIRST_TIMEOUT','0.2')),config=json.dumps({'stream_total_timeout_ms':int(os.environ.get('DIAG_TOTAL_TIMEOUT_MS','5000')),'failover_rules':{'max_attempts':1,'stream_failover_budget_ms':int(os.environ.get('DIAG_BUDGET_MS','5000'))}}))
insert('provider_endpoints',id='endpoint-local',provider_id='provider-local',name='Responses',base_url='http://127.0.0.1:'+str(server.server_port)+'/v1',api_format='openai:responses',api_family='openai',endpoint_kind='responses')
insert('provider_api_keys',id='key-local',provider_id='provider-local',name='synthetic-key',api_key=encrypted,encrypted_key=encrypted,api_formats=json.dumps(['openai:responses']),allowed_models=json.dumps(['gpt-6-astra']))
insert('global_models',id='global-local',name='gpt-6-astra',display_name='local',default_price_per_request=0,supported_capabilities=json.dumps({'streaming':True,'chat':True}))
insert('models',id='model-local',provider_id='provider-local',global_model_id='global-local',provider_model_name='gpt-6-astra',global_model_name='gpt-6-astra',api_format='openai:responses',supports_streaming=1,price_per_request=0)
db.commit();db.close()
s=socket.socket();s.bind(('127.0.0.1',0));port=s.getsockname()[1];s.close()
env={k:os.environ[k] for k in ['HOME','PATH','TMPDIR','LANG'] if k in os.environ}
env.update(ENVIRONMENT='development',AETHER_DATABASE_DRIVER='sqlite',AETHER_DATABASE_URL='sqlite://'+str(DB),AETHER_RUNTIME_BACKEND='memory',AETHER_GATEWAY_AUTO_PREPARE_DATABASE='false',ENCRYPTION_KEY=secret.decode(),JWT_SECRET_KEY='local-reproduction-only-jwt-secret-0123456789',AETHER_LOG_FORMAT='json',AETHER_LOG_DESTINATION='stdout',RUST_LOG='aether_gateway=debug')
log=open(RUN/'gateway.log','w')
binary=str(pathlib.Path(os.environ.get('DIAG_GATEWAY_BINARY','/Applications/Aether.app/Contents/MacOS/aether-gateway')).resolve())
proc=subprocess.Popen([binary,'--app-host','127.0.0.1','--app-port',str(port),'--listener-shards','1','--shutdown-timeout-seconds','2'],env=env,cwd=RUN,stdout=log,stderr=log)
def request(port,path,padding):
 started=time.monotonic(); c=http.client.HTTPConnection('127.0.0.1',port,timeout=float(os.environ.get('DIAG_CLIENT_TIMEOUT','15'))); body=json.dumps({'model':'gpt-6-astra','stream':True,'input':[{'type':'message','role':'user','content':[{'type':'input_text','text':'Summarize this synthetic test.'}]},*([{'type':'compaction_trigger'}] if os.environ.get('DIAG_OPERATION','compact')=='compact' else [])]})
 c.request('POST',path,body,{'Authorization':'Bearer '+client_key,'Content-Type':'application/json'});r=c.getresponse(); chunks=[]; first_output_ms=None; read_error=None
 try:
  while True:
   chunk=r.read1(65536)
   if not chunk:break
   chunks.append(chunk)
   if first_output_ms is None and b'LOCAL_OK' in b''.join(chunks):first_output_ms=round((time.monotonic()-started)*1000)
 except (http.client.IncompleteRead,ConnectionError) as e:
  read_error=type(e).__name__
 body=b''.join(chunks);status=r.status;c.close();return status,body,round((time.monotonic()-started)*1000),first_output_ms,read_error
try:
 for i in range(100):
  if proc.poll() is not None:raise RuntimeError('gateway startup failed; see '+str(RUN/'gateway.log'))
  try:
   c=http.client.HTTPConnection('127.0.0.1',port,timeout=.2);c.request('GET','/health');c.getresponse().read();c.close();break
  except OSError:time.sleep(.1)
 if os.environ.get('DIAG_CONFIG_ONLY')=='1':
  def b64(value):return base64.urlsafe_b64encode(value).decode().rstrip('=')
  signing=b64(json.dumps({'alg':'HS256','typ':'JWT'}).encode())+'.'+b64(json.dumps({'type':'access','exp':now+3600,'user_id':'user-local','role':'admin','session_id':'session-local'}).encode())
  token=signing+'.'+b64(hmac.new(env['JWT_SECRET_KEY'].encode(),signing.encode(),hashlib.sha256).digest())
  def admin(method,path,payload=None):
   c=http.client.HTTPConnection('127.0.0.1',port,timeout=10)
   c.request(method,path,None if payload is None else json.dumps(payload),{'Authorization':'Bearer '+token,'x-client-device-id':'local-fixture','Content-Type':'application/json'})
   r=c.getresponse();data=json.loads(r.read());status=r.status;c.close()
   return status,data
  status,created=admin('POST','/api/admin/providers/',{'name':'config-verification','request_timeout':12,'stream_total_timeout':5.001,'config':{'future':{'enabled':True},'failover_rules':{'stream_failover_budget_ms':1800}}})
  assert status in (200,201),(status,created)
  provider_id=created.get('id') or created.get('data',{}).get('id')
  assert provider_id,created
  path='/api/admin/providers/'+provider_id
  status,saved=admin('GET',path+'/summary')
  assert status==200,(status,saved)
  assert saved['stream_total_timeout']==5.001 and saved['effective_stream_total_timeout']==5.001,saved
  status,renamed=admin('PATCH',path,{'name':'config-renamed'})
  assert status==200,(status,renamed)
  _,renamed=admin('GET',path+'/summary')
  assert renamed['stream_total_timeout']==5.001,renamed
  for invalid in [0,1200.001,1.0001,'5',True]:
   status,data=admin('PATCH',path,{'stream_total_timeout':invalid})
   assert status==400,(invalid,status,data)
  status,cleared=admin('PATCH',path,{'stream_total_timeout':None})
  assert status==200,(status,cleared)
  _,cleared=admin('GET',path+'/summary')
  assert cleared['stream_total_timeout'] is None and cleared['effective_stream_total_timeout']==900 and cleared['effective_stream_total_timeout_source']=='default',cleared
  check=sqlite3.connect(DB)
  config,request_timeout=check.execute('select config,request_timeout from providers where id=?',(provider_id,)).fetchone()
  config=json.loads(config)
  assert 'stream_total_timeout_ms' not in config and config['future']['enabled'] and config['failover_rules']['stream_failover_budget_ms']==1800 and request_timeout==12,config
  report={'saved_seconds':saved['stream_total_timeout'],'cleared_effective_seconds':cleared['effective_stream_total_timeout'],'invalid_writes_rejected':5,'sqlite_preserved_config':config,'passed':True}
  (RUN/'config-results.json').write_text(json.dumps(report,indent=2));print('CONFIG_REPORT',RUN/'config-results.json',flush=True)
  raise SystemExit(0)
 result=[]
 for padding in [0]:
  for label,p in ([('gateway',port)] if os.environ.get('DIAG_ONLY_GATEWAY')=='1' else [('direct',server.server_port),('gateway',port)]):
   status,body,elapsed_ms,first_output_ms,read_error=request(p,'/v1/responses',padding)
   row={'route':label,'first_content_ms':first_output_ms,'read_error':read_error,'elapsed_ms':elapsed_ms,'completed_event':b'response.completed' in body,'padding':padding,'status':status,'has_output':b'LOCAL_OK' in body,'first_timeout':os.environ.get('DIAG_FIRST_TIMEOUT','0.2'),'budget_ms':os.environ.get('DIAG_BUDGET_MS','5000'),'output_kind':os.environ.get('DIAG_OUTPUT_KIND','delta'),'missing_output_error':b'stream_missing_useful_output' in body}
   if status!=200:row['error_summary']=body.decode(errors='replace')[:700]
   print(json.dumps(row),flush=True);result.append(row)
 time.sleep(.4)
 check=sqlite3.connect(DB);check.row_factory=sqlite3.Row
 candidates=[dict(r) for r in check.execute('select status,status_code,error_type,latency_ms from request_candidates')]
 health=[dict(r) for r in check.execute('select health_by_format,circuit_breaker_by_format from provider_api_keys')]
 usage=[dict(r) for r in check.execute('select request_type,status,status_code,error_message,response_time_ms,first_byte_time_ms from usage')]; print('USAGE',json.dumps(usage),flush=True)
 def timing_values(value):
  if isinstance(value,dict):
   for key,item in value.items():
    if key=='stream_timing':yield item
    else:yield from timing_values(item)
  elif isinstance(value,list):
   for item in value:yield from timing_values(item)
 timings=[timing for row in check.execute('select request_metadata from usage') for timing in timing_values(json.loads(row[0] or '{}'))]
 report={'gateway_binary':binary,'usage':usage,'results':result,'upstream_receipts':receipts,'candidates':candidates,'health':health,'stream_timings':timings}
 (RUN/'results.json').write_text(json.dumps(report,indent=2));print('REPORT',RUN/'results.json',flush=True);print('RECEIPTS',json.dumps(receipts),flush=True)
 if os.environ.get('DIAG_SKIP_VALIDATION')=='1':
  raise SystemExit(0)
 if os.environ.get('DIAG_ONLY_GATEWAY')=='1': result.insert(0, {'route':'direct-skipped','has_output':True})
 assert result[0]['has_output'], 'Direct synthetic upstream must succeed'
 expected=os.environ.get('DIAG_EXPECT_SUCCESS')=='1'
 assert result[1]['has_output']==expected, 'Gateway behavior differs from expected timeout reproduction'
 if expected and os.environ.get('DIAG_BROKEN_STREAM')!='1':
  assert result[1]['completed_event'] and any(u['status']=='completed' for u in usage), 'Expected full stream and completed usage'
 if os.environ.get('DIAG_EXPECT_FAILED_TERMINAL')=='1':
  assert any(u['status']=='failed' and u['status_code']==504 for u in usage), 'Expected persisted 504 failed usage'
  assert result[1]['status']==504, 'Expected HTTP 504 timeout response'
 if os.environ.get('DIAG_BROKEN_STREAM')=='1':
  assert not result[1]['completed_event'] and any(u['status']=='failed' for u in usage), 'EOF must not become success'
 if not expected and int(os.environ.get('DIAG_BUDGET_MS','5000'))==5000:
  assert any(c['error_type']=='local_stream_candidate_watchdog_timeout' for c in candidates), 'Expected actual watchdog timeout'
 if os.environ.get('DIAG_EXPECT_PENDING')=='1':
  assert result[1]['status']==504 and any(u['status']=='pending' for u in usage), 'Expected 504 with stuck pending usage'
finally:
 proc.terminate()
 try:proc.wait(timeout=5)
 except subprocess.TimeoutExpired:proc.kill();proc.wait()
 log.close();server.shutdown()
