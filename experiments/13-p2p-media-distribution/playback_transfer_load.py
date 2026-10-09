"""Engineering load: a second authorized synthetic file while two real players run.
Independent transport spike, not a second offer in the same single-media room.
"""
import json,pathlib,subprocess,tempfile,threading,time
from measure_transport import adb,resources,BINARY,ROOT

def run(peer, latest):
 report={'classification':'TESTED / MEASURED','scope':'independent authenticated TLS carrier, own 32 MiB corpus; active-room media and manifest unchanged','bytes':32*1048576,'status':'FAIL','samples':[],'builds':{'linux':'debug','android_carrier':'release','android_app':'debug'}}
 sender=None;worker=None;tid=None
 with tempfile.TemporaryDirectory(prefix='cine-p2p-active-load-') as tmp:
  tmp=pathlib.Path(tmp);source=tmp/'fixture';config=tmp/'config'
  with source.open('wb') as f:
   for _ in range(32):f.write(bytes([37])*1048576)
  ip=json.loads(subprocess.check_output(['ip','-j','-4','route','get','1.1.1.1'],text=True))[0]['prefsrc']
  adb('push',str(ROOT/'target/armv7-linux-androideabi/release/cine-transfer-spike'),BINARY);adb('shell','chmod','700',BINARY)
  start=time.monotonic()
  try:
   sender=subprocess.Popen([str(ROOT/'target/debug/cine-transfer-spike'),'send',str(source),ip+':1731',ip+':1731',str(config)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
   deadline=time.monotonic()+30
   while not config.exists():
    if sender.poll() is not None or time.monotonic()>deadline:raise RuntimeError('LOAD_SETUP_FAILED')
    time.sleep(.05)
   tid=json.loads(config.read_text())['manifest']['transfer_id'];adb('push',str(config),'/data/local/tmp/cine-p2p-load-config');adb('shell','chmod','600','/data/local/tmp/cine-p2p-load-config')
   worker=subprocess.Popen(['adb','shell',BINARY,'receive','/data/local/tmp/cine-p2p-load-config','/data/local/tmp'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
   while worker.poll() is None:
    s=peer.command('sync-state');android=dict(latest)
    report['samples'].append({'elapsed_seconds':time.monotonic()-start,'linux_sender':resources(sender.pid),'linux_player':resources(peer.process.pid),'linux_playing':s.get('playing'),'linux_position_ms':s.get('position_ms'),'android_playing':android.get('playing'),'android_position_ms':android.get('position_ms'),'android_resources':android.get('resources')})
    time.sleep(.5)
   output,_=worker.communicate(timeout=5);sender.communicate(timeout=10)
   final=next((json.loads(l) for l in output.splitlines() if '"status"' in l),{})
   assert worker.returncode==0 and sender.returncode==0 and final.get('status')=='PASS'
   assert all(s['linux_playing'] and s['android_playing'] for s in report['samples'])
   assert report['samples'][-1]['linux_position_ms']>report['samples'][0]['linux_position_ms']
   assert report['samples'][-1]['android_position_ms']>report['samples'][0]['android_position_ms']
   report.update(status='PASS',sha256_match=True,receiver_elapsed_seconds=final['elapsed_seconds'],effective_mib_s=32/final['elapsed_seconds'])
  finally:
   for p in [worker,sender]:
    if p and p.poll() is None:p.terminate();p.wait(timeout=5)
   if tid:adb('shell','rm','-rf','/data/local/tmp/cine-'+tid)
   adb('shell','rm','-f',BINARY,'/data/local/tmp/cine-p2p-load-config')
 return report
