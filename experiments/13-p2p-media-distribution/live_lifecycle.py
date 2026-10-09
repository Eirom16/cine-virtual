"""Opt-in real TLS/WSS/CLI lifecycle faults; no mocked peers or exported secrets."""
import json,pathlib,shutil,sys,tempfile,time
from physical_linux_android import ProductProcess as Process,wait
ROOT=pathlib.Path(__file__).resolve().parents[2];OUT=pathlib.Path(__file__).resolve().parent
report={'schema_version':1,'scenario':'linux-real-control-and-transfer-lifecycle','status':'FAIL','network':'loopback WSS and separate direct TCP/TLS1.3','build':'debug','steps':[]}
processes=[]
def step(s):report['steps'].append(s);print('phase='+s,flush=True)
def command(p,s,event):
 r=p.command(s);assert r.get('event')==event,s.split()[0];return r
try:
 with tempfile.TemporaryDirectory(prefix='cine-p2p-lifecycle-') as tmp:
  tmp=pathlib.Path(tmp);source=tmp/'fixture.mp4';shutil.copyfile(ROOT/'test-media/long-duration.mp4',source)
  with source.open('ab') as f:
   while f.tell()<64*1048576:f.write(bytes(min(1048576,64*1048576-f.tell())))
  destination=tmp/'receive';destination.mkdir()
  server=Process([str(ROOT/'target/debug/cine-server'),'--bind','127.0.0.1:1735','--tls']);processes.append(server);endpoint=server.receive()['endpoint']
  clients=[]
  for name in ['Host QA','Receiver QA']:
   p=Process([str(ROOT/'target/debug/cine-client'),'--server',endpoint,'--name',name,'--player','mpv']);processes.append(p);assert p.receive()['event']=='cli_connected';clients.append(p)
  host,peer=clients;invite=command(host,'create','created');command(peer,'join {room_id} {room_epoch} {invite_token}'.format(**invite),'joined')
  command(host,'select '+str(source),'media_selected');command(host,'share 127.0.0.1:1736','offered')
  wait(lambda:peer.command('transfer-state').get('offer'))
  def progress():return peer.command('transfer-state')['progress']
  def request():
   command(peer,'receive '+str(destination),'requested')
   wait(lambda:any(r['state']=='waiting_for_acceptance' for r in host.command('transfer-state')['receivers']))
   return next(r['receiver_id'] for r in host.command('transfer-state')['receivers'] if r['state']=='waiting_for_acceptance')
  request();assert list(destination.glob('cine-*/media.part'));command(peer,'transfer cancel','transfer_action');wait(lambda:not list(destination.iterdir()));step('cancel_waiting_deletes_partial_without_grant')
  receiver=request();command(host,'transfer accept '+receiver,'transfer_action');wait(lambda:progress()['verified_bytes']>=1048576)
  command(peer,'disconnect','disconnected');wait(lambda:progress()['state'] in ['paused','reconnecting']);paused=progress()['verified_bytes'];assert 0<paused<source.stat().st_size;time.sleep(.5);assert progress()['verified_bytes']==paused
  step('receiver_disconnect_stops_real_socket_and_preserves_verified_chunks')
  command(peer,'transfer cancel','transfer_action');wait(lambda:not list(destination.iterdir()));step('offline_cancel_releases_worker_and_deletes_partial')
  command(peer,'resume-room','resumed');wait(lambda:peer.command('state')['connected']);step('room_resume_after_offline_cancel')
  receiver=request();command(host,'transfer accept '+receiver,'transfer_action');wait(lambda:progress()['verified_bytes']>=1048576)
  command(host,'disconnect','disconnected');wait(lambda:progress()['state'] in ['paused','reconnecting']);n=progress()['verified_bytes'];time.sleep(.5);assert progress()['verified_bytes']==n;step('host_disconnect_revokes_grant_and_preserves_partial')
  command(host,'resume-room','resumed');command(peer,'transfer resume','transfer_action')
  wait(lambda:any(r['state']=='waiting_for_acceptance' for r in host.command('transfer-state')['receivers']))
  command(host,'transfer accept '+receiver,'transfer_action');wait(lambda:progress()['state'] in ['completed','failed'],60);assert progress()['state']=='completed';step('host_resume_new_grant_final_sha_complete')
  command(peer,'load-transfer','transfer_loaded');wait(lambda:peer.command('sync-state')['clock_trusted']);command(peer,'ready','ready');command(host,'ready','ready');step('verified_download_loaded_into_libmpv_and_ready')
  # A new offer has a new manifest/ID. Invalidate it while receiving real blocks.
  command(host,'transfer withdraw','transfer_action');wait(lambda:not peer.command('transfer-state').get('offer'))
  command(host,'share 127.0.0.1:1736','offered');wait(lambda:peer.command('transfer-state').get('offer'));receiver=request();command(host,'transfer accept '+receiver,'transfer_action');wait(lambda:progress()['verified_bytes']>=1048576)
  replacement=tmp/'replacement.mp4';shutil.copyfile(ROOT/'test-media/long-duration.mp4',replacement)
  command(host,'select '+str(replacement),'media_selected');wait(lambda:not peer.command('transfer-state').get('offer'));wait(lambda:not list(destination.glob('cine-*/media.part')));assert not peer.command('sync-state')['room_ready'];step('new_media_revision_invalidates_old_manifest_partial_and_ready')
  report.update(status='PASS',source_bytes=source.stat().st_size,disconnect_verified_bytes=paused,host_disconnect_verified_bytes=n)
except Exception as e:report['failure']=type(e).__name__+':'+str(e) if isinstance(e,(RuntimeError,AssertionError)) else type(e).__name__
finally:
 for p in reversed(processes):p.stop()
 (OUT/'results-lifecycle.json').write_text(json.dumps(report,indent=2)+'\n');print('lifecycle='+report['status'],flush=True)
if report['status']!='PASS':sys.exit(1)
