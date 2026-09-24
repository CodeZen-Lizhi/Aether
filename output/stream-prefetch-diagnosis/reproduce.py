"""Synthetic localhost-only reproduction against the installed gateway binary."""
import base64, hashlib, http.client, json, os, pathlib, socket, sqlite3, subprocess, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from cryptography.fernet import Fernet
ROOT=pathlib.Path(__file__).resolve().parent
RUN=ROOT/('run-'+str(time.time_ns())); RUN.mkdir()
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
  payload=json.loads(raw); text=payload.get('input',''); padding=int(text.split(':')[-1]); rec={'padding':padding,'closed_before_output':False,'output_sent':False}; receipts.append(rec)
  self.send_response(200);self.send_header('Content-Type','text/event-stream');self.send_header('Transfer-Encoding','chunked');self.end_headers()
  def send(data):
   self.wfile.write(('%x\r\n'%len(data)).encode()+data+b'\r\n');self.wfile.flush()
  response={'id':'resp_local','object':'response','created_at':now,'model':'gpt-6-astra','status':'in_progress','output':[], 'instructions':'x'*padding}
  setup=('event: response.created\ndata: '+json.dumps({'type':'response.created','response':response})+'\n\n').encode()
  rec['setup_bytes']=len(setup)
  try:
   send(setup)
   # Upstream stays alive; only client/gateway can close before this output.
   time.sleep(.25)
   self.connection.setblocking(False)
   try: rec['closed_before_output']=self.connection.recv(1,socket.MSG_PEEK)==b''
   except BlockingIOError:pass
   finally:self.connection.setblocking(True)
   send(b'event: response.output_text.delta\ndata: {"type":"response.output_text.delta","delta":"LOCAL_OK","output_index":0,"content_index":0,"item_id":"msg_local"}\n\n')
   response.update(status='completed',output=[{'id':'msg_local','type':'message','role':'assistant','status':'completed','content':[{'type':'output_text','text':'LOCAL_OK','annotations':[]}]}],usage={'input_tokens':1,'output_tokens':1,'total_tokens':2})
   send(('event: response.completed\ndata: '+json.dumps({'type':'response.completed','response':response})+'\n\n').encode());rec['output_sent']=True
   self.wfile.write(b'0\r\n\r\n');self.wfile.flush()
  except (BrokenPipeError,ConnectionResetError):rec['closed_before_output']=True
server=ThreadingHTTPServer(('127.0.0.1',0),Upstream);threading.Thread(target=server.serve_forever,daemon=True).start()
insert('users',id='user-local',username='local-fixture',role='admin')
insert('api_keys',id='client-local',user_id='user-local',key_hash=hashlib.sha256(client_key.encode()).hexdigest(),name='local-client',is_standalone=0,rate_limit=10000)
insert('providers',id='provider-local',name='local-upstream',provider_type='custom',billing_type='free_tier',config=json.dumps({'failover_rules':{'max_attempts':1,'stream_failover_budget_ms':90000}}))
insert('provider_endpoints',id='endpoint-local',provider_id='provider-local',name='Responses',base_url='http://127.0.0.1:'+str(server.server_port)+'/v1',api_format='openai:responses',api_family='openai',endpoint_kind='responses')
insert('provider_api_keys',id='key-local',provider_id='provider-local',name='synthetic-key',api_key=encrypted,encrypted_key=encrypted,api_formats=json.dumps(['openai:responses']),allowed_models=json.dumps(['gpt-6-astra']))
insert('global_models',id='global-local',name='gpt-6-astra',display_name='local',default_price_per_request=0,supported_capabilities=json.dumps({'streaming':True,'chat':True}))
insert('models',id='model-local',provider_id='provider-local',global_model_id='global-local',provider_model_name='gpt-6-astra',global_model_name='gpt-6-astra',api_format='openai:responses',supports_streaming=1,price_per_request=0)
db.commit();db.close()
s=socket.socket();s.bind(('127.0.0.1',0));port=s.getsockname()[1];s.close()
env={k:os.environ[k] for k in ['HOME','PATH','TMPDIR','LANG'] if k in os.environ}
env.update(ENVIRONMENT='development',AETHER_DATABASE_DRIVER='sqlite',AETHER_DATABASE_URL='sqlite://'+str(DB),AETHER_RUNTIME_BACKEND='memory',AETHER_GATEWAY_AUTO_PREPARE_DATABASE='false',ENCRYPTION_KEY=secret.decode(),JWT_SECRET_KEY='local-reproduction-only-jwt-secret-0123456789',AETHER_LOG_FORMAT='json',AETHER_LOG_DESTINATION='stdout',RUST_LOG='aether_gateway=debug')
log=open(RUN/'gateway.log','w')
proc=subprocess.Popen(['/Applications/Aether.app/Contents/MacOS/aether-gateway','--app-host','127.0.0.1','--app-port',str(port),'--listener-shards','1','--shutdown-timeout-seconds','2'],env=env,cwd=RUN,stdout=log,stderr=log)
def request(port,path,padding):
 c=http.client.HTTPConnection('127.0.0.1',port,timeout=15); body=json.dumps({'model':'gpt-6-astra','stream':True,'input':'padding:'+str(padding)})
 c.request('POST',path,body,{'Authorization':'Bearer '+client_key,'Content-Type':'application/json'});r=c.getresponse();body=r.read();status=r.status;c.close();return status,body
try:
 for i in range(100):
  if proc.poll() is not None:raise RuntimeError('gateway startup failed; see '+str(RUN/'gateway.log'))
  try:
   c=http.client.HTTPConnection('127.0.0.1',port,timeout=.2);c.request('GET','/health');c.getresponse().read();c.close();break
  except OSError:time.sleep(.1)
 result=[]
 for padding in [0,20000]:
  for label,p in [('direct',server.server_port),('gateway',port)]:
   status,body=request(p,'/v1/responses',padding)
   row={'route':label,'padding':padding,'status':status,'has_output':b'LOCAL_OK' in body,'missing_output_error':b'stream_missing_useful_output' in body}
   if status!=200:row['error_summary']=body.decode(errors='replace')[:700]
   print(json.dumps(row),flush=True);result.append(row)
 time.sleep(.4)
 check=sqlite3.connect(DB);check.row_factory=sqlite3.Row
 candidates=[dict(r) for r in check.execute('select status,status_code,error_type,latency_ms from request_candidates')]
 health=[dict(r) for r in check.execute('select health_by_format,circuit_breaker_by_format from provider_api_keys')]
 report={'results':result,'upstream_receipts':receipts,'candidates':candidates,'health':health}
 (RUN/'results.json').write_text(json.dumps(report,indent=2));print('REPORT',RUN/'results.json',flush=True);print('RECEIPTS',json.dumps(receipts),flush=True)
finally:
 proc.terminate()
 try:proc.wait(timeout=5)
 except subprocess.TimeoutExpired:proc.kill();proc.wait()
 log.close();server.shutdown()
