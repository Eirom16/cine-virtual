"""Rootless isolated Linux lab. Run ONLY inside the outer unshare command below.
All links, routes, nft NAT/firewall rules and netem live in disposable namespaces.
No host interfaces, firewall, routers, DNS or public services are modified.
"""
import argparse,json,os,pathlib,subprocess,time
ROOT=pathlib.Path(__file__).resolve().parents[2]

def run(*args,input=None):
    return subprocess.run(args,input=input,text=True,check=True,capture_output=True,timeout=15).stdout

def ns(pid,*args,input=None):return run('nsenter','-t',str(pid),'-n',*args,input=input)

def main():
    p=argparse.ArgumentParser();p.add_argument('--launch',action='store_true');p.add_argument('--parent-netns');p.add_argument('--netem',action='store_true');p.add_argument('--bytes',type=int,default=8*1048576);p.add_argument('--output',type=pathlib.Path);args=p.parse_args()
    if args.launch:
        cmd=['unshare','--user','--map-root-user','--net','python3',str(pathlib.Path(__file__).resolve()),'--parent-netns',os.readlink('/proc/self/ns/net'),'--bytes',str(args.bytes)]
        if args.netem:cmd.append('--netem')
        if args.output:cmd.extend(['--output',str(args.output.resolve())])
        raise SystemExit(subprocess.call(cmd))
    if os.geteuid()!=0 or not args.parent_netns or os.readlink('/proc/self/ns/net')==args.parent_netns or pathlib.Path('/proc/self/uid_map').read_text().split()[1]=='0':
        raise SystemExit('Use --launch for isolated rootless namespaces; refusing host network')
    proc=None;holders=[];report={'schema_version':1,'scenario':'rootless-isolated-two-NAT-lab','real_wan':'NOT TESTED','physical_cgnat':'NOT TESTED','status':'FAIL'}
    try:
        run('ip','link','set','lo','up');run('ip','link','add','wanbr','type','bridge');run('ip','addr','add','10.200.0.1/24','dev','wanbr');run('ip','link','set','wanbr','up')
        base=os.readlink('/proc/self/ns/net')
        for _ in range(4):
            child=subprocess.Popen(['unshare','--net','sleep','180']);holders.append(child)
            deadline=time.monotonic()+5
            while os.readlink(f'/proc/{child.pid}/ns/net')==base:
                if time.monotonic()>deadline:raise RuntimeError('namespace creation timeout')
                time.sleep(.01)
            ns(child.pid,'ip','link','set','lo','up')
        a,b,na,nb=[c.pid for c in holders]
        for idx,(peer,nat) in enumerate([(a,na),(b,nb)],1):
            run('ip','link','add',f'w{idx}','type','veth','peer','name',f'e{idx}');run('ip','link','set',f'e{idx}','netns',str(nat));run('ip','link','set',f'w{idx}','master','wanbr');run('ip','link','set',f'w{idx}','up')
            ns(nat,'ip','addr','add',f'10.200.0.{10+idx}/24','dev',f'e{idx}');ns(nat,'ip','link','set',f'e{idx}','up')
            run('ip','link','add',f'p{idx}','type','veth','peer','name',f'n{idx}');run('ip','link','set',f'p{idx}','netns',str(peer));run('ip','link','set',f'n{idx}','netns',str(nat))
            ns(nat,'ip','addr','add',f'10.20{idx}.0.1/24','dev',f'n{idx}');ns(nat,'ip','link','set',f'n{idx}','up');ns(nat,'sysctl','-qw','net.ipv4.ip_forward=1')
            ns(peer,'ip','addr','add',f'10.20{idx}.0.2/24','dev',f'p{idx}');ns(peer,'ip','link','set',f'p{idx}','up');ns(peer,'ip','route','add','default','via',f'10.20{idx}.0.1')
            rules=f'''table ip lab {{
chain lab_post {{ type nat hook postrouting priority srcnat; policy accept; oifname "e{idx}" counter masquerade; }}
chain lab_forward {{ type filter hook forward priority filter; policy drop; ct state established,related counter accept; iifname "n{idx}" counter accept; }}
chain lab_input {{ type filter hook input priority filter; policy accept; iifname "e{idx}" tcp dport 1730 counter drop; }}
}}'''
            ns(nat,'nft','-f','-',input=rules)
            if args.netem:
                # Apply one impairment per direction on the external NAT links.
                ns(nat,'tc','qdisc','add','dev',f'e{idx}','root','netem','delay','40ms','10ms','loss','0.5%','rate','10mbit')
        started=time.monotonic();samples=[]
        proc=subprocess.Popen([str(ROOT/'target/debug/cine-wan-spike'),str(args.bytes),'10.200.0.1:0',f'/proc/{a}/ns/net',f'/proc/{b}/ns/net','10.200.0.11:1730'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        ticks=os.sysconf('SC_CLK_TCK')
        while proc.poll() is None:
            try:
                folder=pathlib.Path('/proc')/str(proc.pid);stat=(folder/'stat').read_text().rsplit(')',1)[1].split();status={k:v.strip() for k,v in (l.split(':',1) for l in (folder/'status').read_text().splitlines() if ':' in l)}
                samples.append({'elapsed_seconds':time.monotonic()-started,'cpu_seconds':(int(stat[11])+int(stat[12]))/ticks,'rss_kib':int(status['VmRSS'].split()[0]),'threads':int(status['Threads']),'fds':len(list((folder/'fd').iterdir()))})
            except (OSError,KeyError,ValueError):pass
            if time.monotonic()-started>150:proc.kill();raise RuntimeError('lab timeout')
            time.sleep(.1)
        stdout,stderr=proc.communicate(timeout=5)
        if proc.returncode:raise RuntimeError('carrier lab failed: '+stderr.strip())
        result=json.loads(stdout)
        counters=[json.loads(ns(nat,'nft','-j','list','table','ip','lab')) for nat in [na,nb]]
        packets=[]
        for c in counters:
            rules=[r['rule'] for r in c['nftables'] if 'rule' in r]
            snat=sum(e['counter']['packets'] for r in rules if r['chain']=='lab_post' for e in r['expr'] if 'counter' in e)
            dropped=sum(e['counter']['packets'] for r in rules if r['chain']=='lab_input' for e in r['expr'] if 'counter' in e)
            packets.append({'snat_packets':snat,'blocked_direct_packets':dropped})
        assert all(c['snat_packets']>0 for c in packets)
        assert packets[0]['blocked_direct_packets']>0
        assert result['direct_probe']['status']=='EXPECTED_FAILURE'
        assert result['sha_final_match']
        report.update(status='LOCAL_SIMULATION_PASS',transport=result,nat_counters=packets,impairment={'delay_ms':40,'jitter_ms':10,'loss_percent':.5,'rate_mbit_s':10} if args.netem else None,resource_samples=samples,resource_scope='all three carrier workers in one Rust process; no Player/Flutter',wall_seconds=time.monotonic()-started)
    except subprocess.CalledProcessError as e:
        report['error']=e.stderr.strip() or str(e)
    except Exception as e:
        report['error']=str(e)
    finally:
        if proc is not None and proc.poll() is None:
            proc.kill();proc.communicate(timeout=5)
        for child in holders:
            child.terminate()
        for child in holders:
            child.wait(timeout=5)
    if args.output:args.output.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items() if k!='resource_samples'}))
    if report['status']=='FAIL':raise SystemExit(1)
if __name__=='__main__':main()
