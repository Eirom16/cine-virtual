"""Opt-in small physical LAN regression. No network changes or mobile data.
Phase 1 ARMv7 binary vs current TCP/TLS sender; private config only in temp/ADB.
"""
import json,pathlib,subprocess,tempfile,time,os
ROOT=pathlib.Path(__file__).resolve().parents[2];OUT=pathlib.Path(__file__).resolve().parent
BINARY='/data/local/tmp/cine-phase2-baseline'
def adb(*args):return subprocess.run(['adb',*args],capture_output=True,check=True,timeout=30)
def main():
    report={'schema_version':1,'status':'FAIL','scenario':'physical-LAN-v1-compatibility-baseline','real_wan':'NOT TESTED','network':'same Wi-Fi; direct TCP/TLS1.3; private ADB config; no reverse/forward','linux_build':os.environ.get('CINE_BASELINE_SOURCE','current worktree')+' debug','android_build':'existing Phase 1 release ARMv7 carrier','product_ready_player':'separate loopback product baseline','runs':[]}
    model=adb('shell','getprop','ro.product.model').stdout.decode().strip();api=adb('shell','getprop','ro.build.version.sdk').stdout.decode().strip();abi=adb('shell','getprop','ro.product.cpu.abi').stdout.decode().strip();report['device']={'model':model,'api':api,'abi':abi};report['source_label']=os.environ.get('CINE_BASELINE_SOURCE','current worktree')
    routes=adb('shell','ip','route').stdout.decode()
    if 'dev wlan0' not in routes:raise SystemExit('LAN prerequisite absent; refusing mobile test')
    ip=json.loads(subprocess.check_output(['ip','-j','-4','route','get','1.1.1.1'],text=True))[0]['prefsrc']
    adb('push',str(ROOT/'target/armv7-linux-androideabi/release/cine-transfer-spike'),BINARY);adb('shell','chmod','700',BINARY)
    try:
        with tempfile.TemporaryDirectory(prefix='cine-phase2-lan-') as tmp:
            tmp=pathlib.Path(tmp)
            for size in [8192,8*1048576]:
                config=tmp/'private-config';config.unlink(missing_ok=True);source=tmp/'synthetic';source.write_bytes(bytes((i%251) for i in range(size)))
                sender=subprocess.Popen([os.environ.get('CINE_BASELINE_SENDER',str(ROOT/'target/debug/cine-transfer-spike')),'send',str(source),'0.0.0.0:1730',ip+':1730',str(config)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
                tid=None
                try:
                    until=time.monotonic()+5
                    while not config.exists():
                        if sender.poll() is not None or time.monotonic()>until:raise RuntimeError('sender setup failed')
                        time.sleep(.02)
                    while True:
                        try:
                            tid=json.loads(config.read_text())['manifest']['transfer_id'];break
                        except json.JSONDecodeError:
                            if time.monotonic()>until:raise RuntimeError('config did not finish writing')
                            time.sleep(.02)
                    adb('push',str(config),'/data/local/tmp/cine-phase2-private-config');adb('shell','chmod','600','/data/local/tmp/cine-phase2-private-config')
                    receiver=subprocess.run(['adb','shell',BINARY,'receive','/data/local/tmp/cine-phase2-private-config','/data/local/tmp'],capture_output=True,text=True,timeout=30)
                    stdout,_=sender.communicate(timeout=5)
                    events=[json.loads(l) for l in receiver.stdout.splitlines()];final=next((e for e in events if e.get('status')=='PASS'),None)
                    if not (final and receiver.returncode==0 and sender.returncode==0):
                        report['safe_failure']={'receiver_exit':receiver.returncode,'sender_exit':sender.returncode,'receiver_verified_sha':bool(final),'sender_codes':[json.loads(l).get('code') for l in _.splitlines() if l.startswith('{')],'receiver_codes':[e.get('code') for e in events if e.get('code')]}
                        raise RuntimeError('transport did not complete')
                    report['runs'].append({'bytes':size,'status':'PASS','route':'DIRECT_LAN','elapsed_seconds':final['elapsed_seconds'],'effective_mib_s':size/1048576/final['elapsed_seconds'],'sha_final_match':final['sha256_match'],'chunks_verified':True})
                finally:
                    if sender.poll() is None:sender.terminate();sender.wait(timeout=5)
                    if tid:adb('shell','rm','-rf','/data/local/tmp/cine-'+tid)
        report['status']='PASS'
    except Exception as e:report['error']=type(e).__name__
    finally:
        adb('shell','rm','-f',BINARY,'/data/local/tmp/cine-phase2-private-config');(OUT/os.environ.get('CINE_BASELINE_RESULT','results-lan.json')).write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report))
    if report['status']!='PASS':raise SystemExit(1)
if __name__=='__main__':main()
