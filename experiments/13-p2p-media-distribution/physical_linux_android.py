"""Opt-in physical product test. Private invitations stay in pipes/ADB extras.
P2P_QA only joins a real room and emits sanitized observations. Consent, pause,
resume and Ready are exercised through actual Android UI input, never a mock.
"""
import json,os,pathlib,queue,re,shlex,subprocess,sys,threading,time,xml.etree.ElementTree as ET
ROOT=pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
from demo_control import Process,wait_state
from demo_real_media import resources
class ProductProcess(Process):
 def receive(self):return self.output.get(timeout=60)
Process=ProductProcess
OUT=pathlib.Path(__file__).resolve().parent
PACKAGE='dev.cinevirtual.cine_mobile_spike'
APP=ROOT/'experiments/02-rust-ui-bridge/app'
def adb(*args):return subprocess.run(['adb',*args],capture_output=True,check=True,timeout=30)
def nodes():
 adb('shell','uiautomator','dump','/sdcard/cine-p2p-ui.xml')
 return list(ET.fromstring(adb('shell','cat','/sdcard/cine-p2p-ui.xml').stdout).iter('node'))
def tap(label,scroll=True):
 for _ in range(6):
  ns=nodes();found=[n for n in ns if n.get('text')==label or set(n.get('content-desc','').strip().splitlines())=={label}]
  if found:
   b=[int(x) for x in re.findall(r'\d+',found[0].get('bounds',''))]
   if len(b)==4 and b[2]>b[0] and b[3]>b[1]:adb('shell','input','tap',str((b[0]+b[2])//2),str((b[1]+b[3])//2));return
  if scroll:adb('shell','input','swipe','280','930','280','250','250')
 raise RuntimeError('UI_LABEL_NOT_VISIBLE')
def shot(name):
 time.sleep(.5)
 data=adb('exec-out','screencap','-p').stdout
 (OUT/'screenshots'/('android-'+name+'.png')).write_bytes(data)
def wait(predicate,timeout=30):
 end=time.monotonic()+timeout
 while time.monotonic()<end:
  if predicate():return
  time.sleep(.1)
 raise RuntimeError('SCENARIO_TIMEOUT')
def main():
 report={'schema_version':1,'scenario':'physical-linux-host-android-product','status':'FAIL','source_base_commit':subprocess.check_output(['git','rev-parse','--short','HEAD'],cwd=ROOT,text=True).strip(),'network':'direct Wi-Fi LAN TCP/TLS1.3 and separate WSS; no adb reverse/relay','build_modes':{'linux':'debug','android_rust':'release','android_flutter':'debug'},'device':'SM-J701M API28 armv7','steps':[],'samples':[]}
 processes=[];logs=None;latest={};samples=report['samples']
 def step(name):report['steps'].append(name);print('phase='+name,flush=True)
 def measure(name):
  memory=adb('shell','dumpsys','meminfo',PACKAGE).stdout.decode()
  pss=re.search(r'TOTAL PSS:\s*(\d+)',memory)
  if not pss:pss=re.search(r'\bTOTAL\s+(\d+)\s+',memory)
  report.setdefault('resource_snapshots',[]).append({'phase':name,'linux':resources(host),'android_pss_kib':int(pss[1]) if pss else None})
 def collect():
  for raw in logs.stdout:
   if 'CINE_P2P_SAMPLE ' not in raw:continue
   try:s=json.loads(raw.split('CINE_P2P_SAMPLE ',1)[1]);latest.clear();latest.update(s);samples.append(s)
   except ValueError:pass
 try:
  subprocess.run(['adb','install','-r',str(APP/'build/app/outputs/flutter-apk/app-debug.apk')],capture_output=True,check=True,timeout=90)
  adb('shell','am','force-stop',PACKAGE);adb('shell','input','keyevent','KEYCODE_WAKEUP')
  ip=json.loads(subprocess.check_output(['ip','-j','-4','route','get','1.1.1.1'],text=True))[0]['prefsrc']
  server=Process([str(ROOT/'target/debug/cine-server'),'--bind',ip+':1729','--allow-lan','--tls']);processes.append(server)
  endpoint=server.receive()['endpoint']
  host=Process([str(ROOT/'target/debug/cine-client'),'--server',endpoint,'--name','Linux QA','--player','mpv','--allow-lan','true']);processes.append(host);assert host.receive()['event']=='cli_connected'
  invite=host.command('create');assert invite['event']=='created';step('host_created')
  media=ROOT/'test-media/p2p-authorized.mp4'
  if not media.exists():
   import shutil
   shutil.copyfile(ROOT/'test-media/long-duration.mp4',media)
   # Trailing zeros enlarge our own synthetic MP4 without duplicating memory.
  with media.open('ab') as f:
   while f.tell()<256*1024*1024:f.write(bytes(min(1048576,256*1024*1024-f.tell())))
  assert host.command('select '+str(media))['event']=='media_selected';assert host.command('ready')['event']=='ready';step('host_media_selected_and_ready')
  assert host.command('share '+ip+':1730')['event']=='offered';step('host_offered')
  private=json.dumps({'server':endpoint,'invitation':{k:invite[k] for k in ['room_id','room_epoch','invite_token']}},separators=(',',':'))
  since=adb('shell',"date '+%m-%d %H:%M:%S.000'").stdout.decode().strip()
  logs=subprocess.Popen(['adb','logcat','-T',since,'-s','flutter:I','*:S'],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
  reader=threading.Thread(target=collect,daemon=True);reader.start()
  adb('shell','am','start','-n',PACKAGE+'/.MainActivity','--es','cine_server',endpoint,'--es','cine_invite',shlex.quote(private))
  wait(lambda:latest.get('connected') and (latest.get('transfer') or {}).get('offer'));step('participant_joined_offer_visible');shot('offer');measure('before_download')
  tap('Recibir archivo del anfitrión');wait(lambda:any(n.get('content-desc')=='Aceptar descarga' for n in nodes()));shot('consent');tap('Aceptar descarga');step('recipient_consented')
  receiver=None
  def waiting():
   nonlocal receiver
   t=host.command('transfer-state');rs=t.get('receivers') or []
   if rs and rs[0]['state']=='waiting_for_acceptance':receiver=rs[0]['receiver_id'];return True
   return False
  wait(waiting);step('recipient_requested')
  tap('Cancelar y eliminar parcial');wait(lambda:latest.get('transfer',{}).get('progress',{}).get('state')=='cancelled')
  transfer_id=latest['transfer']['offer']['transfer_id']
  wait(lambda:('cine-'+transfer_id) not in adb('shell','run-as',PACKAGE,'ls','files/cine-transfers').stdout.decode().split())
  step('android_ui_cancel_waiting_deletes_partial')
  tap('Recibir archivo del anfitrión');tap('Aceptar descarga');wait(waiting);step('recipient_requested_again_after_cancel')
  adb('shell','input','swipe','280','930','280','250','250');time.sleep(.5);shot('waiting')
  assert host.command('transfer accept '+receiver)['event']=='transfer_action';step('host_authorized');start=time.monotonic()
  wait(lambda:(latest.get('transfer') or {}).get('progress',{}).get('state')=='transferring');shot('progress')
  # UIAutomator waits for idle while progress changes; use screenshot-verified
  # visible coordinates for this single physical 720x1280 device instead.
  position=pathlib.Path(os.environ.get('CINE_P2P_PAUSE_POSITION',str(pathlib.Path.home()/'.local/share/cine-spike-tools/p2p-pause-position.json')))
  wait(lambda:position.exists(),20)
  coords=json.loads(position.read_text());adb('shell','input','tap',str(coords[0]),str(coords[1]))
  wait(lambda:latest.get('transfer',{}).get('progress',{}).get('state')=='paused');paused=latest['transfer']['progress']['verified_bytes'];assert 0<paused<media.stat().st_size;report['paused_verified_bytes']=paused;shot('paused');step('paused_real_transfer_before_all_chunks')
  time.sleep(1);assert latest['transfer']['progress']['verified_bytes']==paused;measure('paused')
  tap('Reanudar transferencia');wait(waiting);assert host.command('transfer accept '+receiver)['event']=='transfer_action';step('resumed_new_authorization')
  baseline_social=latest.get('social_count',0)
  assert host.command('chat QA durante transferencia')['event']=='chat_sent';wait(lambda:latest.get('social_count',0)>baseline_social);step('chat_during_transfer')
  assert host.command('reaction 🔥')['event']=='reaction_sent';wait(lambda:latest.get('reaction_count',0)>0);step('reaction_during_transfer')
  before_loss=latest['transfer']['progress']['verified_bytes'];assert before_loss<media.stat().st_size
  adb('shell','svc','wifi','disable');step('wifi_disabled_during_real_transfer')
  try:
   wait(lambda:latest.get('transfer',{}).get('progress',{}).get('state') in ['paused','reconnecting'],30)
   report['verified_bytes_before_wifi_loss']=before_loss
   report['verified_bytes_after_wifi_loss']=latest['transfer']['progress']['verified_bytes'];shot('connection-interrupted')
  finally:adb('shell','svc','wifi','enable')
  time.sleep(8)
  if not latest.get('connected'):tap('Reconectar a la sala')
  wait(lambda:latest.get('connected') and latest.get('trusted'),45);step('room_reconnected_after_wifi_loss')
  tap('Reanudar transferencia');wait(waiting);assert host.command('transfer accept '+receiver)['event']=='transfer_action';step('missing_chunks_resumed_after_wifi_loss')
  verifying_captured=False
  def ended():
   nonlocal verifying_captured
   state=latest.get('transfer',{}).get('progress',{}).get('state')
   if state=='verifying' and not verifying_captured:verifying_captured=True;shot('verifying')
   return state in ['completed','failed']
  wait(ended,180)
  assert latest['transfer']['progress']['state']=='completed';report['download_elapsed_with_pause_seconds']=time.monotonic()-start;shot('completed');step('all_chunks_and_final_sha_verified')
  wait(lambda:latest.get('hash_state')=='complete' and latest.get('player_ready') and latest.get('identity_match'),90);step('existing_media_pipeline_loaded');shot('loaded')
  tap('Estoy listo');wait(lambda:latest.get('room_ready'));shot('ready');step('participant_ready')
  wait_state(host,lambda s:all(m['ready'] for m in s['members']));assert host.command('play')['event']=='accepted';wait(lambda:latest.get('playing'));shot('player');step('both_real_players_play')
  time.sleep(3);assert host.command('pause')['event']=='accepted';wait(lambda:not latest.get('playing'));step('pause_control')
  wait(lambda:host.command('sync-state')['ready'] and latest.get('player_ready'))
  time.sleep(1)
  seek_reply=host.command('seek 15000');report['seek_reply']={k:v for k,v in seek_reply.items() if k in ['event','type','sequence']};assert seek_reply['event']=='accepted';wait(lambda:abs((latest.get('position_ms') or 0)-15000)<1000);step('seek_control')
  adb('shell','input','tap','675','1235');tap('Pantalla completa',False);shot('fullscreen');step('fullscreen_real_player')
  tap('Salir de pantalla completa',False);tap('Volver a la sala',False);wait(lambda:latest.get('page')=='lobby');step('player_to_lobby')
  adb('shell','input','keyevent','KEYCODE_HOME');time.sleep(2)
  adb('shell','am','start','-n',PACKAGE+'/.MainActivity');wait(lambda:latest.get('trusted') and latest.get('room_ready') and latest.get('identity_match'),90);step('background_foreground_revalidated')
  measure('after_playback_smoke')
  report.update(status='PASS',bytes=media.stat().st_size,final_sha_verified=True,identity_match=True,participant_ready=True)
 except Exception as e:
  report['failure']=type(e).__name__+':'+str(e) if isinstance(e,(AssertionError,RuntimeError)) else type(e).__name__
  print('physical_status=FAIL',flush=True)
 finally:
  if logs:logs.terminate();logs.wait(timeout=5)
  for p in reversed(processes):p.stop()
  adb('shell','am','force-stop',PACKAGE)
  # Only self-created app-private download directories are retained by product policy.
  text=json.dumps(report,indent=2)
  if re.search(r'content://|/home/|"secret"|"certificate"|"invite_token"|"resume_token"',text):raise RuntimeError('EVIDENCE_NOT_REDACTED')
  (OUT/'results-product-linux-android.json').write_text(text+'\n')
  print('physical_status='+report['status'],flush=True)
 if report['status']!='PASS':sys.exit(1)
if __name__=='__main__':main()
