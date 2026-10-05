"""Physical Android + Linux room; no emulator, no adb reverse, no media server.

Default 600 seconds of real playback. Tokens/addresses only live in private
subprocess buffers. Failed/preflight-blocked runs are evidence, never PASS.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import tempfile
import subprocess
import threading
import time
import uuid
import xml.etree.ElementTree as ET
from demo_control import Process, wait_state, execution, ROOT
from demo_real_media import fields, checked, percentile, resources
from demo_mobile_ui import TOOLS, APP, PACKAGE

OUT = ROOT/'experiments/07-linux-android-room'
FIXTURE = '/sdcard/Download/cine-room-long.mp4'

def stats(values):
    return {k:percentile(values,q) for k,q in [('p50_ms',.5),('p95_ms',.95),('p99_ms',.99)]} | {'max_ms':max(values,default=None),'count':len(values)}

def valid(sample):
    return sample and sample.get('ready') and sample.get('clock_trusted') and not sample.get('seeking') and not sample.get('buffering') and sample.get('sample_age_ms',999) <= 100 and sample.get('drift_ms') is not None

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--seconds',type=float,default=600)
    parser.add_argument('--no-build',action='store_true')
    parser.add_argument('--mismatch-only',action='store_true',help='Real Android hash of clip + one appended byte; expect MEDIA_MISMATCH')
    parser.add_argument('--port',type=int,default=8765,help='Explicit LAN port; firewall must already allow it')
    parser.add_argument('--server-address',help='Host LAN IPv4 (kept out of evidence)')
    args=parser.parse_args()
    if args.seconds < 25 or args.seconds > 600: raise SystemExit('seconds must be 25..600')
    OUT.mkdir(parents=True,exist_ok=True)
    report={'schema_version':1,'scenario':'linux-android-room','timestamp_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'requested_wall_seconds':args.seconds,'passed':False,'network':'controlled Wi-Fi/LAN ws; no tunnel',
            'source_base_commit':subprocess.check_output(['git','rev-parse','--short','HEAD'],cwd=ROOT,text=True).strip()}
    output=OUT/('results-mismatch.json' if args.mismatch_only else 'results-lan.json' if args.seconds>=590 else 'results-smoke.json')
    def save():
        data=json.dumps(report,indent=2)
        if re.search(r'/home/|content://|[a-f0-9]{64}|(?:\d{1,3}\.){3}\d{1,3}',data):raise RuntimeError('Sensitive data in evidence')
        output.write_text(data+'\n')
    adb=str(TOOLS/'android/platform-tools/adb')
    online=[s.split()[0] for s in subprocess.check_output([adb,'devices'],text=True).splitlines()[1:] if s.endswith('\tdevice')]
    if len(online)!=1 or online[0].startswith('emulator-'):
        report.update(status='BLOCKED',failure='REQUIRED_AUTHORIZED_PHYSICAL_ANDROID_NOT_AVAILABLE',runtime_executed=False)
        save();raise SystemExit('Physical Android unavailable; blocked evidence saved (no runtime metrics).')
    def device(*cmd,check=True):
        return subprocess.run([adb,'-s',online[0],*cmd],capture_output=True,text=True,check=check,timeout=120 if cmd[0] in ['install','push'] else 25).stdout
    processes=[]; logs=None; fixture_owned=False; installed=False
    cleanup_errors=[];modified_fixture=None
    def cleanup_device(*cmd):
        try: device(*cmd,check=False)
        except (subprocess.SubprocessError,OSError):cleanup_errors.append('ADB_CLEANUP_UNAVAILABLE')
    android_samples=[]; android_events=[]; android_resources=[]; markers=[]; readers=[]
    run_id=uuid.uuid4().hex
    def read_logs():
        started=False
        for line in logs.stdout:
            if 'CINE_ROOM_BEGIN '+run_id in line:started=True
            if not started:continue
            if 'CINE_ROOM_SAMPLE ' in line:
                try: android_samples.append(json.loads(line.split('CINE_ROOM_SAMPLE ',1)[1]))
                except json.JSONDecodeError: pass
            elif 'CINE_ROOM_RESOURCE ' in line:
                try:android_resources.append(json.loads(line.split('CINE_ROOM_RESOURCE ',1)[1]))
                except json.JSONDecodeError:pass
            elif 'CINE_ROOM_' in line:
                markers.append(line.split('CINE_ROOM_',1)[1].strip())
            elif 'CineRoom' in line and '{' in line:
                try: android_events.append(json.loads(line[line.index('{'):]))
                except json.JSONDecodeError: pass
            elif 'CINE_SPIKE_' in line: markers.append(line.split('CINE_SPIKE_',1)[1].strip())
    def wait(predicate,timeout=20):
        start=time.monotonic()
        while time.monotonic()-start<timeout:
            if predicate():return (time.monotonic()-start)*1000
            time.sleep(.05)
        raise RuntimeError('PHYSICAL_SCENARIO_TIMEOUT')
    def tap(label):
        for _ in range(8):
            device('shell','uiautomator','dump','/sdcard/cine-room-ui.xml',check=False)
            xml=device('shell','cat','/sdcard/cine-room-ui.xml',check=False)
            try:nodes=[n for n in ET.fromstring(xml).iter('node') if n.get('text')==label or n.get('content-desc','').strip()==label]
            except ET.ParseError:nodes=[]
            if nodes:
                bounds=list(map(int,re.findall(r'\d+',nodes[0].get('bounds',''))))
                if len(bounds)==4 and bounds[2]>bounds[0] and bounds[3]>bounds[1]:
                    device('shell','input','tap',str((bounds[0]+bounds[2])//2),str((bounds[1]+bounds[3])//2))
                    return
            device('shell','input','swipe','300','900','300','250','300')
        raise RuntimeError('UI_ACTION_NOT_VISIBLE')
    try:
        abi=device('shell','getprop','ro.product.cpu.abi').strip()
        report['device']={'android':device('shell','getprop','ro.build.version.release').strip(),'abi':abi,'physical':True}
        env=os.environ.copy();env.update(ANDROID_SDK_ROOT=str(TOOLS/'android'),JAVA_HOME=str(TOOLS/'jdk21'))
        env['PATH']=str(TOOLS/'flutter/bin')+os.pathsep+str(TOOLS/'bin')+os.pathsep+env['PATH']
        if not args.no_build:
            subprocess.run(['cargo','build','--workspace'],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
            subprocess.run(['python3','scripts/build_mobile_bridge.py','--abi',abi],cwd=ROOT,env=env,check=True,stdout=subprocess.DEVNULL)
            target={'armeabi-v7a':'android-arm','arm64-v8a':'android-arm64','x86_64':'android-x64'}[abi]
            subprocess.run(['flutter','build','apk','--debug','--target-platform='+target,'--dart-define=ROOM_MODE=true','--dart-define=ROOM_AUTORUN=true'],cwd=APP,env=env,check=True,stdout=subprocess.DEVNULL)
        media=ROOT/'test-media/long-duration.mp4'
        if not media.exists():subprocess.run(['python3','scripts/generate_test_media.py'],cwd=ROOT,check=True)
        if subprocess.run([adb,'-s',online[0],'shell','test','-e',FIXTURE],capture_output=True).returncode==0:raise RuntimeError('FIXTURE_ALREADY_EXISTS_REFUSE_OVERWRITE')
        phone_media=media
        if args.mismatch_only:
            with tempfile.NamedTemporaryFile(dir=media.parent,prefix='cine-mismatch-',suffix='.mp4',delete=False) as dest:
                modified_fixture=Path(dest.name)
                with media.open('rb') as source:shutil.copyfileobj(source,dest,1024*1024)
                dest.write(b'\0')
            phone_media=modified_fixture
        fixture_owned=True;device('push',str(phone_media),FIXTURE)
        device('shell','am','broadcast','-a','android.intent.action.MEDIA_SCANNER_SCAN_FILE','-d','file://'+FIXTURE)
        device('install','-r',str(APP/'build/app/outputs/flutter-apk/app-debug.apk'));installed=True
        server=Process([str(ROOT/'target/debug/cine-server'),'--bind',f'0.0.0.0:{args.port}','--allow-lan']);processes.append(server)
        wait(lambda:bool(fields(server,'server_started')),5)
        port=int(fields(server,'server_started')[0]['bind'].rsplit(':',1)[1])
        if args.server_address:address=args.server_address
        else:
            routes=json.loads(subprocess.check_output(['ip','-j','-4','route','get','1.1.1.1'],text=True))
            address=routes[0]['prefsrc']
        host=Process([str(ROOT/'target/debug/cine-client'),'--server',f'ws://127.0.0.1:{port}','--name','Linux','--player','mpv']);processes.append(host)
        assert host.receive()['event']=='cli_connected';invitation=checked(host,'create','created')
        checked(host,'select '+str(media),'media_selected');checked(host,'ready','ready')
        report['linux_hash']=host.command('hash-status')
        private={k:invitation[k] for k in ['room_id','room_epoch','invite_token']}
        since=device('shell',"date '+%m-%d %H:%M:%S.000'").strip()
        logs=subprocess.Popen([adb,'-s',online[0],'logcat','-T',since,'-s','flutter:I','CineRoom:I','CineSpike:I','*:S'],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
        reader=threading.Thread(target=read_logs,daemon=True);reader.start();readers.append(reader)
        print('phase=android_start',flush=True)
        device('shell','input','keyevent','KEYCODE_WAKEUP')
        # Extras contain private invitation, never printed/exported. No adb tunnel.
        device('shell','am','start','-n',PACKAGE+'/.MainActivity','--es','cine_server',f'ws://{address}:{port}','--es','cine_invite',shlex.quote(json.dumps(private,separators=(',',':'))),'--es','cine_run_id',run_id)
        selected=False;deadline=time.monotonic()+180
        while time.monotonic()<deadline and not any(x.startswith('READY') for x in markers):
            if args.mismatch_only and any(x.startswith('FAILURE MEDIA_MISMATCH') for x in markers):break
            if any(x.startswith('FAILURE') for x in markers):raise RuntimeError('ANDROID_APPLICATION_FAILED')
            if not selected:
                device('shell','uiautomator','dump','/sdcard/cine-room-ui.xml',check=False)
                xml=device('shell','cat','/sdcard/cine-room-ui.xml',check=False)
                try:
                    nodes=list(ET.fromstring(xml).iter('node'))
                    choices=[n for n in nodes if n.get('text') in ['cine-room-long.mp4','cine-room-long'] and n.get('class')!='android.widget.EditText']
                    if not choices and any(n.get('resource-id')=='com.android.documentsui:id/dir_list' for n in nodes):
                        device('shell','input','swipe','300','900','300','250','300')
                        continue
                    if not choices:choices=[n for n in nodes if n.get('text') in ['Downloads','Download','Descargas'] and n.get('resource-id')!='android:id/title']
                    if not choices:choices=[n for n in nodes if n.get('content-desc') in ['Show roots','Open navigation drawer','Mostrar raíces','Abrir panel de navegación']]
                    if choices:
                        n=choices[-1];b=list(map(int,re.findall(r'\d+',n.get('bounds',''))))
                        device('shell','input','tap',str((b[0]+b[2])//2),str((b[1]+b[3])//2));selected=n.get('text') in ['cine-room-long.mp4','cine-room-long']
                except ET.ParseError:pass
            time.sleep(.25)
        if args.mismatch_only:
            assert any(x.startswith('FAILURE MEDIA_MISMATCH') for x in markers)
            room=host.command('state')
            assert len(room['members'])==2 and all(not m['ready'] for m in room['members'] if m['member_id']!=room['host_id'])
            report.update(status='PASS',passed=True,scenario='android_real_media_mismatch',expected_error='MEDIA_MISMATCH',participant_ready=False,media_bytes_original=media.stat().st_size,media_bytes_android=phone_media.stat().st_size,android_samples=android_samples.copy(),room_preserved=True)
            print('physical_mismatch=PASS',flush=True)
            return
        wait(lambda:any(x.startswith('READY') for x in markers),3)
        print('phase=both_ready',flush=True)
        room=wait_state(host,lambda s:len(s['members'])==2 and all(m['ready'] for m in s['members']))
        report['identity_match']=True;report['ready']=True;report['media_size_bytes']=media.stat().st_size
        android_member=next(m['member_id'] for m in room['members'] if m['member_id']!=room['host_id'])
        # Participant attempts a real request; server rejection must preserve sequence.
        rejected_before=len(android_samples);before_seq=room['sequence']
        device('shell','input','swipe','250','800','250','250','300');tap('play')
        wait(lambda:any(x.get('last_action')=='play' and x.get('error')=='NOT_AUTHORIZED' for x in android_samples[rejected_before:]),10)
        assert host.command('state')['sequence']==before_seq
        report['participant_authority_rejected']=True
        def convergence(start_index):
            try:return wait(lambda:len([x for x in android_samples[start_index:] if valid(x.get('sync')) and abs(x['sync']['drift_ms'])<=150])>=3,20)
            except RuntimeError:return None  # Experimental precision failure is evidence, not a fabricated PASS.
        controls=[]
        def control(command):
            reply=checked(host,command,'accepted');e=execution(host,reply['sequence'])
            wait(lambda:any(x.get('sequence')==reply['sequence'] and x.get('reason')=='scheduled' for x in android_events),10)
            controls.append({'command':command,'sequence':reply['sequence'],'execute_at_server_ms':e['expected_server_ms'],'linux_scheduler_lateness_ms':e['lateness_ms']})
        control('play 5000');began=time.monotonic();actions=set();recovery={};resource_samples=[];last_progress=-1
        while (elapsed:=time.monotonic()-began)<args.seconds or len(actions)<4:
            if elapsed>=min(10,args.seconds*.15) and 'paused_seek' not in actions:
                control('pause');control('seek 10000');control('play 10000');actions.add('paused_seek')
            if elapsed>=min(35,args.seconds*.25) and 'playing_seek' not in actions:
                current=host.command('sync-state')['position_ms'];control('seek '+str(current+3000));actions.add('playing_seek')
            if elapsed>=min(120,args.seconds*.4) and 'background' not in actions:
                count=sum(x.startswith('RECOVERED') for x in markers)
                resume_samples=len(android_samples)
                device('shell','input','keyevent','KEYCODE_HOME');wait(lambda:any(x.startswith('SUSPENDED') for x in markers),5)
                time.sleep(3);device('shell','am','start','-n',PACKAGE+'/.MainActivity')
                recovery['foreground_ms']=wait(lambda:sum(x.startswith('RECOVERED') for x in markers)>count,20)
                wait_state(host,lambda s:all(m['ready'] for m in s['members']))
                recovery['foreground_post_ready_convergence_ms']=convergence(resume_samples)
                recovered_samples=[x['sync'] for x in android_samples[resume_samples:] if valid(x.get('sync'))]
                recovery['foreground_initial_drift_ms']=recovered_samples[0]['drift_ms'] if recovered_samples else None
                actions.add('background')
            if elapsed>=min(240,args.seconds*.65) and 'disconnect' not in actions:
                # UI buttons must be reachable; use scroll if necessary on small phone.
                device('shell','input','swipe','250','800','250','250','300');tap('disconnect')
                wait_state(host,lambda s:any(not m['connected'] for m in s['members']))
                time.sleep(2);start=time.monotonic();resume_samples=len(android_samples);tap('reconnect')
                wait_state(host,lambda s:all(m['connected'] and m['ready'] for m in s['members']))
                recovery['network_resume_ms']=(time.monotonic()-start)*1000
                recovery['network_post_ready_convergence_ms']=convergence(resume_samples)
                recovery['same_member_after_resume']=any(m['member_id']==android_member and m['connected'] for m in host.command('state')['members'])
                assert recovery['same_member_after_resume']
                actions.add('disconnect')
            bucket=int(elapsed)//30
            if bucket!=last_progress:print('progress_wall_s='+str(round(elapsed)),flush=True);last_progress=bucket
            resource=resources(host)
            if resource:resource['sample_at_wall_ms']=(time.monotonic()-began)*1000;resource_samples.append(resource)
            time.sleep(.25)
        report['real_wall_seconds']=time.monotonic()-began
        control('pause');time.sleep(1)
        ls=fields(host,'sync_sample');a=[x for x in ls if valid(x)];bs=[x['sync'] for x in android_samples if valid(x.get('sync'))]
        report['linux_drift']=stats([abs(x['drift_ms']) for x in a]);report['android_drift']=stats([abs(x['drift_ms']) for x in bs])
        pairs=[]
        for b in bs:
            if not a:break
            aa=min(a,key=lambda x:abs(x['server_ms']-b['server_ms']))
            dt=b['server_ms']-aa['server_ms']
            if abs(dt)<=600 and aa['sequence']==b.get('sequence',aa['sequence']) and aa['playing']==b['playing']:
                projected=aa['position_ms']+(dt*aa['rate'] if aa['playing'] else 0)
                pairs.append({'server_ms':b['server_ms'],'difference_ms':abs(projected-b['position_ms']),'sample_delta_ms':dt})
        report['steady_room_ready_drift']={'linux':stats([abs(x['drift_ms']) for x in a if x.get('room_ready') and not x.get('snapshot_required',False)]),'android':stats([abs(x['drift_ms']) for x in bs if x.get('room_ready') and not x.get('snapshot_required',False)])}
        report['cross_device_difference']=stats([x['difference_ms'] for x in pairs]);report['pair_samples']=pairs
        minutes=report['real_wall_seconds']/60
        linux_counts=host.command('sync-state')['corrections']
        android_counts={k:max((x.get('sync',{}).get('corrections',{}).get(k,0) for x in android_samples if x.get('sync')),default=0) for k in ['rate','restore','seek']}
        android_dispatches={x['sequence']:x for x in android_events if x.get('event')=='native_dispatch' and x.get('reason')=='scheduled' and x.get('deadline_server_ms',0)>0}
        report['corrections']={label:{'counts':counts,'soft_per_minute':counts['rate']/minutes,'hard_per_minute':counts['seek']/minutes} for label,counts in [('linux',linux_counts),('android',android_counts)]}
        report['dispatch_lateness']={'linux':stats([abs(x['linux_scheduler_lateness_ms']) for x in controls]),'android':stats([abs(x['lateness_ms']) for x in android_dispatches.values()]),'missed_definition_ms':50,'linux_missed':sum(x['linux_scheduler_lateness_ms']>50 for x in controls),'android_missed':sum(x['lateness_ms']>50 for x in android_dispatches.values())}
        report['android_seek_ready']=[{k:x[k] for k in ['sequence','latency_ms','target_ms','position_ms','server_ms'] if k in x} for x in android_events if x.get('event')=='native_seek_ready']
        report['clock_android']=[x['clock'] for x in android_samples if x.get('clock')]
        report['linux_samples']=ls;report['android_samples']=android_samples.copy();report['android_events']=android_events;report['controls']=controls.copy();report['recovery']=recovery;report['linux_resources']=resource_samples;report['android_resources']=android_resources
        def resource_summary(items,memory_key,time_key,ticks_per_second):
            good=[x for x in items if all(k in x for k in [memory_key,time_key,'cpu_ticks','threads','fds'])]
            if not good:return {'measured':False}
            elapsed=(good[-1][time_key]-good[0][time_key])/1000
            return {'measured':True,'memory_metric':memory_key,'initial_kib':good[0][memory_key],'final_kib':good[-1][memory_key],'max_kib':max(x[memory_key] for x in good),
                    'threads_min':min(x['threads'] for x in good),'threads_max':max(x['threads'] for x in good),'fds_min':min(x['fds'] for x in good),'fds_max':max(x['fds'] for x in good),
                    'cpu_percent_one_core':(good[-1]['cpu_ticks']-good[0]['cpu_ticks'])/ticks_per_second/elapsed*100 if elapsed>0 else None}
        report['resources_summary']={'linux':resource_summary(resource_samples,'rss_kib','sample_at_wall_ms',os.sysconf('SC_CLK_TCK')),
            'android':resource_summary(android_resources,'pss_after_hash_and_load_kib','sample_monotonic_ms',android_resources[0].get('clock_ticks_per_second',100) if android_resources else 100)}
        report['metrics_policy']={'all_loaded_trusted_samples':'Includes room-not-ready recovery/preparation transients; excludes clock untrusted, seek/buffer, stale SDK samples','steady_room_ready':'Additional subset only; does not replace primary metrics or hide recovery','convergence_goal_ms':150,'convergence_requires_samples':3,'convergence_timeout_seconds':20}
        report['p95_goal_met']=all(report[k]['p95_ms'] is not None and report[k]['p95_ms']<=150 for k in ['linux_drift','android_drift'])
        report['passed']=bool(a and bs and pairs and len(actions)==4);report['status']='PASS' if report['passed'] else 'FAIL'
        OUT.joinpath('results-background.json' if args.seconds>=590 else 'results-background-smoke.json').write_text(json.dumps({'schema_version':1,'executed':True,'session_wall_seconds':report['real_wall_seconds'],'recovery':recovery,'background_markers_count':sum(x.startswith('SUSPENDED') for x in markers)},indent=2)+'\n')
    except (RuntimeError,AssertionError,subprocess.SubprocessError,KeyError) as e:
        report.update(status='FAIL',failure=str(e) if isinstance(e,RuntimeError) else type(e).__name__,runtime_executed=bool(android_samples),android_samples=android_samples[-50:],android_events=android_events)
        # Never propagate adb command/serial, private arguments or full SDK errors.
    finally:
        if installed:
            cleanup_device('shell','input','keyevent','KEYCODE_BACK')
            time.sleep(.5)
            cleanup_device('shell','input','keyevent','KEYCODE_BACK')
            try:wait(lambda:'RUST_RELEASED' in markers and 'PLAYER_RELEASED' in markers,8);report['teardown_observed']=True
            except RuntimeError:report['teardown_observed']=False
            cleanup_device('shell','am','force-stop',PACKAGE)
            try:report['android_process_stopped']=not bool(device('shell','pidof',PACKAGE,check=False).strip())
            except (subprocess.SubprocessError,OSError):report['android_process_stopped']=False
        if logs:
            logs.terminate()
            try:logs.wait(timeout=5)
            except subprocess.TimeoutExpired:logs.kill();logs.wait(timeout=5)
        for reader in readers:reader.join(timeout=2)
        if fixture_owned:cleanup_device('shell','rm',FIXTURE,'/sdcard/cine-room-ui.xml')
        report['cleanup_errors']=cleanup_errors
        if modified_fixture:modified_fixture.unlink(missing_ok=True)
        for p in reversed(processes):p.stop()
        report['processes_reaped']=all(p.process.poll() is not None for p in processes)
        if report['passed'] and (not report.get('teardown_observed') or not report['processes_reaped'] or cleanup_errors or not report.get('android_process_stopped')):
            report.update(passed=False,status='FAIL',failure='TEARDOWN_NOT_CONFIRMED')
            save()
            raise SystemExit('Functional scenario finished but teardown was not confirmed.')
        save()
    print(json.dumps({k:report.get(k) for k in ['status','real_wall_seconds','linux_drift','android_drift','cross_device_difference','teardown_observed']},indent=2))
    if not report['passed']:raise SystemExit('Cross-platform scenario did not pass; sanitized evidence saved.')

if __name__=='__main__':main()
