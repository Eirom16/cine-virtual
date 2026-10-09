"""Reuse Phase 1 real WSS/TLS/libmpv lifecycle without overwriting old evidence.
Own synthetic video (currently 24,300,725 bytes), minimum 16 MiB; loopback product, NOT physical WAN/Android.
"""
import pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[2]
source=ROOT/'experiments/13-p2p-media-distribution/live_lifecycle.py'
s=source.read_text().replace('OUT=pathlib.Path(__file__).resolve().parent','OUT=ROOT/"experiments/14-p2p-wan"').replace('64*1048576','16*1048576').replace("'results-lifecycle.json'","'results-product-lan-loopback.json'")
s=s.replace("command(host,'resume-room','resumed');command(peer,'transfer resume','transfer_action')", "command(host,'resume-room','resumed');step('host_room_resumed');command(peer,'transfer resume','transfer_action');step('receiver_requested_fresh_grant')")
s=s.replace("command(host,'transfer accept '+receiver,'transfer_action');wait(lambda:progress()['state'] in ['completed','failed'],60)", "step('fresh_grant_request_observed');command(host,'transfer accept '+receiver,'transfer_action');step('host_granted_again');wait(lambda:progress()['state'] in ['completed','failed'],60)")
s=s.replace("except Exception as e:report['failure']=", "except Exception as e:\n try:report['safe_progress']=progress();report['safe_receivers']=[{'state':r['state'],'authorized':r['authorized'],'verified_bytes':r['verified_bytes']} for r in host.command('transfer-state').get('receivers',[])]\n except Exception:pass\n report['failure']=")
sys.path.insert(0,str(source.parent))
exec(compile(s,str(source),'exec'),{'__file__':str(source),'__name__':'__main__'})
