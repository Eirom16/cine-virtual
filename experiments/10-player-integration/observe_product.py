"""Read-only debug VM observations; output excludes credentials, paths and hashes.

Usage: python3 observe_product.py http://127.0.0.1:PORT/AUTH_PATH=/
For Android forward its VM port with adb. This does not forward room traffic.
Not a frame test or a production API. Uses the pinned debug build's object model.
"""
import json
import sys
import urllib.parse
import urllib.request

class VM:
 def __init__(self,url,uri,cls):
  self.url=url if url.endswith('/') else url+'/'
  self.isolate=self.rpc('getVM')['isolates'][0]['id']
  libs=self.rpc('getIsolate',isolateId=self.isolate)['libraries']
  self.library=next(l['id'] for l in libs if l['uri'].endswith(uri))
  classes=self.rpc('getObject',isolateId=self.isolate,objectId=self.library)['classes']
  self.cls=next(c['id'] for c in classes if c['name']==cls)
 def rpc(self,method,**args):
  data=json.load(urllib.request.urlopen(self.url+method+'?'+urllib.parse.urlencode(args), timeout=10))
  if 'error' in data: raise RuntimeError(data['error'])
  return data['result']

def instance(v):
 return v.rpc('getInstances',isolateId=v.isolate,objectId=v.cls,limit=10)['instances'][-1]
def field(v,ref,name):
 obj=v.rpc('getObject',isolateId=v.isolate,objectId=ref['id'])
 return next(f['value'] for f in obj['fields'] if f['decl']['name']==name)
def decode(v,ref,depth=0):
 if depth>20:return None
 if ref.get('valueAsStringIsTruncated'):
  ref=v.rpc('getObject',isolateId=v.isolate,objectId=ref['id'],offset=0,count=65536)
 kind=ref.get('kind')
 if 'valueAsString' in ref:
  val=ref['valueAsString']
  if kind=='String': return val
  try:return json.loads(val)
  except: return val
 if kind=='Null': return None
 obj=v.rpc('getObject',isolateId=v.isolate,objectId=ref['id'])
 if 'associations' in obj:
  return {decode(v,a['key'],depth+1):decode(v,a['value'],depth+1) for a in obj['associations']}
 if 'elements' in obj:return [decode(v,e,depth+1) for e in obj['elements']]
 return {f['decl']['name']:decode(v,f['value'],depth+1) for f in obj.get('fields',[])}

def observe(url):
 v=VM(url,'application_controller.dart','ApplicationController')
 obj=instance(v)
 raw=decode(v,field(v,obj,'_raw'))
 n=raw.get('network') or {};p=n.get('presentation') or {};r=p.get('room') or {}
 s=n.get('sync') or {};m=n.get('media') or {};h=n.get('hash') or raw.get('hash') or {}
 timeline=(r.get('playback') or {}).get('current') or {}
 return {'generation':raw.get('generation'),'connected':n.get('connected'),
  'member_id':p.get('member_id'),'room_id':r.get('room_id'),
  'sequence':r.get('sequence'),'media_revision':(r.get('media') or {}).get('media_revision'),
  'timeline':{k:timeline.get(k) for k in ['position_ms','status','anchor_time_ms','duration_ms']},
  'position_ms':s.get('position_ms'),'target_ms':s.get('target_ms'),'playing':s.get('playing'),
  'ready':s.get('ready'),'room_ready':s.get('room_ready'),'snapshot_required':s.get('snapshot_required'),
  'trusted':s.get('clock_trusted'),'seeking':s.get('seeking'),'corrections':s.get('corrections'),
  'identity_match':m.get('identity_match'),'hash_state':h.get('state') or h.get('status'),
  'last_action':n.get('last_action'),'error':n.get('error'),
  'ui_error':decode(v,field(v,obj,'error')),'recovering':decode(v,field(v,obj,'recovering')),
  'sample':{k:(raw.get('sample') or {}).get(k) for k in ['position_ms','playing','loaded','seeking','buffering']}}

if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('Pass the debug VM service URL locally; do not save it in evidence.')
    print(json.dumps(observe(sys.argv[1]), ensure_ascii=False))
