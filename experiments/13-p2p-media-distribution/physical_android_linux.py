"""Inverse transport gate only. Product SAF/Ready evidence is separate."""
import json,pathlib,subprocess,tempfile,time,re
ROOT=pathlib.Path(__file__).resolve().parents[2]
OUT=pathlib.Path(__file__).resolve().parent
REMOTE='/data/local/tmp/cine-p2p-inverse'
def adb(*args):return subprocess.run(['adb',*args],capture_output=True,check=True,timeout=30)
def main():
 report={'schema_version':1,'scenario':'physical-android-host-linux-transport','status':'FAIL','product_saf_ui_ready':'NOT TESTED','network':'same Wi-Fi LAN TCP/TLS1.3; no relay or adb reverse','bytes':32*1048576,'android_build':'release','linux_build':'debug','device':'SM-J701M API28 armv7'}
 sender=None
 adb('shell','mkdir','-p',REMOTE)
 try:
  ip=re.search(r'inet (\d+\.\d+\.\d+\.\d+)',adb('shell','ip','-4','addr','show','wlan0').stdout.decode())[1]
  with tempfile.TemporaryDirectory(prefix='cine-p2p-inverse-') as tmp:
   tmp=pathlib.Path(tmp);source=tmp/'fixture';config=tmp/'config'
   with source.open('wb') as f:
    block=bytes((i*29)%256 for i in range(1048576))
    for _ in range(32):f.write(block)
   adb('push',str(source),REMOTE+'/fixture')
   adb('push',str(ROOT/'target/armv7-linux-androideabi/release/cine-transfer-spike'),REMOTE+'/spike')
   adb('shell','chmod','700',REMOTE+'/spike')
   sender=subprocess.Popen(['adb','shell',REMOTE+'/spike','send',REMOTE+'/fixture','0.0.0.0:1730',ip+':1730',REMOTE+'/config'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
   # Public listening observation follows completion of hashing/private config creation.
   if json.loads(sender.stdout.readline()).get('state')!='listening':raise RuntimeError('SENDER_SETUP_FAILED')
   adb('pull',REMOTE+'/config',str(config));config.chmod(0o600)
   receiver=subprocess.run([str(ROOT/'target/debug/cine-transfer-spike'),'receive',str(config),str(tmp)],capture_output=True,text=True,timeout=120)
   final=next((json.loads(line) for line in receiver.stdout.splitlines() if '"status"' in line),{})
   sender.wait(timeout=10)
   if receiver.returncode==0 and sender.returncode==0 and final.get('status')=='PASS':
    elapsed=final['elapsed_seconds'];report.update(status='PASS',sha256_final_match=True,block_hash_verified=True,receiver_elapsed_seconds=elapsed,effective_mib_s=32/elapsed)
 except Exception as e:report['failure']=type(e).__name__
 finally:
  if sender and sender.poll() is None:sender.terminate();sender.wait(timeout=5)
  adb('shell','rm','-rf',REMOTE)
  (OUT/'results-android-linux-transport.json').write_text(json.dumps(report,indent=2)+'\n');print('inverse_transport='+report['status'])
if __name__=='__main__':main()
