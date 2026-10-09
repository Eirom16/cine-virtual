"""Physical transport-only metrics; no room or Player claims. Own synthetic corpus.
Private spike configs are deleted and never included in exported observations.
"""
import json,pathlib,subprocess,tempfile,time,threading,os
ROOT=pathlib.Path(__file__).resolve().parents[2]
OUT=pathlib.Path(__file__).resolve().parent
BINARY='/data/local/tmp/cine-transfer-spike'
def adb(*args):return subprocess.run(['adb',*args],capture_output=True,check=True,timeout=30)
def resources(pid):
 try:
  p=pathlib.Path('/proc')/str(pid);stat=(p/'stat').read_text().rsplit(')',1)[1].split()
  status={k:v.strip() for k,v in (line.split(':',1) for line in (p/'status').read_text().splitlines() if ':' in line)}
  return {'cpu_seconds':(int(stat[11])+int(stat[12]))/os.sysconf('SC_CLK_TCK'),'rss_kib':int(status['VmRSS'].split()[0]),'threads':int(status['Threads']),'fds':len(list((p/'fd').iterdir()))}
 except (OSError,ValueError,KeyError):return None
def main():
 ip=json.loads(subprocess.check_output(['ip','-j','-4','route','get','1.1.1.1'],text=True))[0]['prefsrc']
 report={'schema_version':1,'scenario':'physical-transport-size-ladder','network':'same Wi-Fi LAN; TCP/TLS1.3; no adb reverse or relay','linux_build':'debug','android_build':'release','device':'SM-J701M API28 armv7','sender_cap_mib_s':8,'product_integration':'separate evidence','runs':[]}
 adb('push',str(ROOT/'target/armv7-linux-androideabi/release/cine-transfer-spike'),BINARY);adb('shell','chmod','700',BINARY)
 try:
  with tempfile.TemporaryDirectory(prefix='cine-p2p-measure-') as tmp:
   tmp=pathlib.Path(tmp)
   for size in [8192,32*1048576,128*1048576]:
    source=tmp/'fixture';config=tmp/'config';config.unlink(missing_ok=True)
    with source.open('wb') as f:
     block=bytes((x*37)%256 for x in range(1048576))
     for offset in range(0,size,len(block)):f.write(block[:min(len(block),size-offset)])
    sender=subprocess.Popen([str(ROOT/'target/debug/cine-transfer-spike'),'send',str(source),'0.0.0.0:1730',ip+':1730',str(config)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    samples=[];stop=threading.Event();tid=None
    def sample():
     while not stop.wait(.25):
      r=resources(sender.pid)
      if r:samples.append({'elapsed_seconds':time.monotonic()-begin,**r})
    begin=time.monotonic();reader=threading.Thread(target=sample);reader.start()
    run={'bytes':size,'direction':'Linux to Android','status':'FAIL'}
    try:
     while not config.exists():
      if sender.poll() is not None:raise RuntimeError('SENDER_FAILED')
      time.sleep(.05)
     tid=json.loads(config.read_text())['manifest']['transfer_id']
     adb('push',str(config),'/data/local/tmp/cine-p2p-config');adb('shell','chmod','600','/data/local/tmp/cine-p2p-config')
     recv=subprocess.run(['adb','shell',BINARY,'receive','/data/local/tmp/cine-p2p-config','/data/local/tmp'],capture_output=True,text=True,timeout=120)
     receiver=[json.loads(line) for line in recv.stdout.splitlines()]
     sout,_=sender.communicate(timeout=10);events=[json.loads(line) for line in sout.splitlines()]
     final=next((e for e in receiver if e.get('status')=='PASS'),None)
     if recv.returncode==0 and sender.returncode==0 and final:
      elapsed=final['elapsed_seconds'];run.update(status='PASS',receiver_elapsed_seconds=elapsed,effective_mib_s=size/1048576/elapsed,sha256_final_match=True,block_hash_verified=True,sender_events=events)
     else:run['error']='TRANSPORT_FAILED'
    finally:
     stop.set();reader.join(timeout=2)
     if sender.poll() is None:sender.terminate();sender.wait(timeout=5)
     if tid:adb('shell','rm','-rf','/data/local/tmp/cine-'+tid)
     run['linux_resource_samples']=samples
     run['android_cpu_memory']='NOT MEASURED in shell transport run; product PSS collected separately'
     report['runs'].append(run);print('measured_bytes='+str(size)+' status='+run['status'],flush=True)
 finally:
  adb('shell','rm','-f',BINARY,'/data/local/tmp/cine-p2p-config')
  (OUT/'results-performance.json').write_text(json.dumps(report,indent=2)+'\n')
if __name__=='__main__':main()
