"""Android SAF Host -> Linux real libmpv participant; genuine room/UI grants."""
import json,pathlib,shlex,subprocess,tempfile,threading,time,sys,shutil
from physical_linux_android import adb,tap,shot,wait,PACKAGE,APP,ProductProcess as Process
ROOT=pathlib.Path(__file__).resolve().parents[2];OUT=pathlib.Path(__file__).resolve().parent
report={'schema_version':1,'scenario':'physical-android-saf-host-linux-product','status':'FAIL','network':'same Wi-Fi LAN WSS + direct TCP/TLS1.3; no relay or adb reverse','device':'SM-J701M API28 armv7','build_modes':{'android_rust':'release','android_flutter':'debug','linux':'debug'},'steps':[],'samples':[]}
latest={};logs=None;processes=[]
def step(name):report['steps'].append(name);print('phase='+name,flush=True)
def collect():
 for raw in logs.stdout:
  if 'CINE_P2P_SAMPLE ' in raw:
   try:s=json.loads(raw.split('CINE_P2P_SAMPLE ',1)[1]);latest.clear();latest.update(s);report['samples'].append(s)
   except ValueError:pass
try:
 with tempfile.TemporaryDirectory(prefix='cine-p2p-inverse-product-') as tmp:
  tmp=pathlib.Path(tmp);source=tmp/'authorized.mp4';shutil.copyfile(ROOT/'test-media/long-duration.mp4',source)
  with source.open('ab') as f:
   while f.tell()<128*1048576:f.write(bytes(min(1048576,128*1048576-f.tell())))
  adb('push',str(source),'/sdcard/Download/cine-p2p-authorized.mp4')
  ip=json.loads(subprocess.check_output(['ip','-j','-4','route','get','1.1.1.1'],text=True))[0]['prefsrc']
  server=Process([str(ROOT/'target/debug/cine-server'),'--bind',ip+':1729','--allow-lan','--tls']);processes.append(server);endpoint=server.receive()['endpoint']
  adb('shell','am','force-stop',PACKAGE)
  adb('shell','run-as',PACKAGE,'rm','-f','cache/cine-p2p-qa-invitation')
  since=adb('shell',"date '+%m-%d %H:%M:%S.000'").stdout.decode().strip()
  logs=subprocess.Popen(['adb','logcat','-T',since,'-s','flutter:I','*:S'],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True);threading.Thread(target=collect,daemon=True).start()
  adb('shell','am','start','-n',PACKAGE+'/.MainActivity','--es','cine_server',endpoint,'--ez','cine_qa_host','true')
  wait(lambda:latest.get('host') and latest.get('connected'));step('android_host_created')
  invitation=json.loads(adb('shell','run-as',PACKAGE,'cat','cache/cine-p2p-qa-invitation').stdout)['invitation']
  peer=Process([str(ROOT/'target/debug/cine-client'),'--server',endpoint,'--name','Linux QA','--player','mpv','--allow-lan','true']);processes.append(peer);peer.receive();assert peer.command('join {room_id} {room_epoch} {invite_token}'.format(**invitation))['event']=='joined';step('linux_joined')
  tap('Seleccionar archivo');time.sleep(1)
  # Provider UI navigates to our own pushed test fixture; no URI injection.
  try:tap('Show roots',False);tap('Downloads',False)
  except RuntimeError:pass
  tap('cine-p2p-authorized.mp4',False)
  wait(lambda:latest.get('hash_state')=='complete' and latest.get('player_ready') and latest.get('identity_match'),90);step('saf_source_validated_in_media3');shot('host-saf-loaded')
  tap('Estoy listo');wait(lambda:latest.get('room_ready'));step('android_host_ready')
  tap('Compartir archivo');tap('Tengo autorización para distribuir este archivo',False);tap('Ofrecer archivo',False)
  wait(lambda:latest.get('transfer',{}).get('offer'));step('host_offered_with_distribution_consent');shot('host-offered')
  tcp=adb('shell','run-as',PACKAGE,'cat','/proc/net/tcp','/proc/net/tcp6').stdout.decode()
  report['android_listener_present']=any(':06C2' in line and ' 0A ' in line for line in tcp.splitlines())
  assert peer.command('receive '+str(tmp))['event']=='requested';step('linux_consented_and_requested')
  tap('Autorizar');step('host_authorized')
  def progress():return peer.command('transfer-state')['progress']
  wait(lambda:progress().get('verified_bytes',0)>=1048576);p=progress();assert p['verified_bytes']<source.stat().st_size
  assert peer.command('transfer pause')['event']=='transfer_action';wait(lambda:progress()['state']=='paused');paused=progress()['verified_bytes'];report['paused_verified_bytes']=paused;step('linux_paused_with_missing_chunks')
  time.sleep(1);assert progress()['verified_bytes']==paused
  assert peer.command('transfer resume')['event']=='transfer_action';tap('Autorizar');step('new_host_grant_resume')
  wait(lambda:progress()['state'] in ['completed','failed'],120);assert progress()['state']=='completed';step('linux_final_sha_verified')
  assert peer.command('load-transfer')['event']=='transfer_loaded';wait(lambda:peer.command('sync-state').get('clock_trusted'),30);assert peer.command('ready')['event']=='ready';step('linux_real_player_ready')
  tap('Comenzar película');wait(lambda:latest.get('playing'));wait(lambda:peer.command('sync-state').get('playing'));step('both_real_players_play');shot('host-player')
  before=latest.get('social_count',0);assert peer.command('chat QA desde Linux')['event']=='chat_sent';wait(lambda:latest.get('social_count',0)>before);step('chat_received')
  from playback_transfer_load import run as playback_load
  report['playback_transfer_load']=playback_load(peer,latest);assert report['playback_transfer_load']['status']=='PASS';step('second_synthetic_transfer_while_both_real_players_play')
  # Portrait Player control locations verified in android-host-player.png;
  # do not let UIAutomator's idle wait outlast the overlay timer.
  adb('shell','input','tap','675','1235');time.sleep(.2);adb('shell','input','tap','75','1220')
  wait(lambda:not latest.get('playing'));wait(lambda:not peer.command('sync-state').get('playing'));step('android_host_pause_controls_both')
  report.update(status='PASS',bytes=source.stat().st_size,final_sha_verified=True,identity_match=True,real_saf=True,real_media3=True,real_libmpv=True)
except Exception as e:
 report['failure']=type(e).__name__+':'+str(e) if isinstance(e,(RuntimeError,AssertionError)) else type(e).__name__
 if 'peer' in locals():report['last_receiver_progress']=peer.command('transfer-state')['progress']
finally:
 if logs:logs.terminate();logs.wait(timeout=5)
 for p in reversed(processes):p.stop()
 adb('shell','am','force-stop',PACKAGE);adb('shell','rm','-f','/sdcard/Download/cine-p2p-authorized.mp4');adb('shell','run-as',PACKAGE,'rm','-f','cache/cine-p2p-qa-invitation')
 text=json.dumps(report,indent=2)
 if any(x in text for x in ['content://','/home/','"secret"','"certificate"','"invite_token"','"resume_token"']):raise RuntimeError('EVIDENCE_NOT_REDACTED')
 (OUT/'results-product-android-linux.json').write_text(text+'\n');print('inverse_product='+report['status'],flush=True)
if report['status']!='PASS':sys.exit(1)
