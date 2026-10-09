"""Real Linux product Host visual harness. Uses XTest, not widget fixtures.
Private room invite stays in a temporary mode-0600 file; only own app screenshots.
"""
import ctypes,ctypes.util,json,os,pathlib,subprocess,tempfile,threading,time,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'scripts'))
from demo_control import Process
OUT=pathlib.Path(__file__).resolve().parent
APP=ROOT/'experiments/02-rust-ui-bridge/app'
class X:
 def __init__(self):
  self.x=ctypes.CDLL(ctypes.util.find_library('X11'));self.t=ctypes.CDLL(ctypes.util.find_library('Xtst'))
  self.x.XOpenDisplay.restype=ctypes.c_void_p;self.d=self.x.XOpenDisplay(None)
  if not self.d:raise RuntimeError('DISPLAY_UNAVAILABLE')
  self.x.XDefaultRootWindow.argtypes=[ctypes.c_void_p];self.x.XDefaultRootWindow.restype=ctypes.c_ulong;self.root=self.x.XDefaultRootWindow(self.d)
  self.x.XQueryTree.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.POINTER(ctypes.c_ulong),ctypes.POINTER(ctypes.c_ulong),ctypes.POINTER(ctypes.POINTER(ctypes.c_ulong)),ctypes.POINTER(ctypes.c_uint)]
  self.x.XFetchName.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.POINTER(ctypes.c_char_p)]
  self.x.XFree.argtypes=[ctypes.c_void_p];self.x.XFlush.argtypes=[ctypes.c_void_p]
  self.x.XRaiseWindow.argtypes=[ctypes.c_void_p,ctypes.c_ulong];self.x.XSetInputFocus.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.c_int,ctypes.c_ulong]
  self.x.XTranslateCoordinates.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.c_ulong,ctypes.c_int,ctypes.c_int,ctypes.POINTER(ctypes.c_int),ctypes.POINTER(ctypes.c_int),ctypes.POINTER(ctypes.c_ulong)]
  self.x.XStringToKeysym.argtypes=[ctypes.c_char_p];self.x.XStringToKeysym.restype=ctypes.c_ulong
  self.x.XKeysymToKeycode.argtypes=[ctypes.c_void_p,ctypes.c_ulong];self.x.XKeysymToKeycode.restype=ctypes.c_uint
  self.t.XTestFakeKeyEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]
  self.t.XTestFakeButtonEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]
  self.t.XTestFakeMotionEvent.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_ulong]
 def windows(self,parent=None):
  root=ctypes.c_ulong();par=ctypes.c_ulong();children=ctypes.POINTER(ctypes.c_ulong)();n=ctypes.c_uint();out=[]
  self.x.XQueryTree(self.d,parent or self.root,ctypes.byref(root),ctypes.byref(par),ctypes.byref(children),ctypes.byref(n))
  ids=[children[i] for i in range(n.value)];self.x.XFree(children)
  for w in ids:
   title=ctypes.c_char_p();self.x.XFetchName(self.d,w,ctypes.byref(title))
   name=title.value.decode(errors='replace') if title.value else '';self.x.XFree(title)
   out.append((w,name));out.extend(self.windows(w))
  return out
 def window(self):
  return next(w for w,n in self.windows() if n=='Cine Virtual')
 def focus(self,w):self.x.XRaiseWindow(self.d,w);self.x.XSetInputFocus(self.d,w,1,0);self.x.XFlush(self.d)
 def click(self,w,x,y):
  px=ctypes.c_int();py=ctypes.c_int();child=ctypes.c_ulong();self.x.XTranslateCoordinates(self.d,w,self.root,0,0,ctypes.byref(px),ctypes.byref(py),ctypes.byref(child))
  self.t.XTestFakeMotionEvent(self.d,-1,px.value+x,py.value+y,0)
  self.t.XTestFakeButtonEvent(self.d,1,1,0);self.t.XTestFakeButtonEvent(self.d,1,0,0);self.x.XFlush(self.d);time.sleep(.3)
 def key(self,name,down=True):
  code=self.x.XKeysymToKeycode(self.d,self.x.XStringToKeysym(name.encode()));self.t.XTestFakeKeyEvent(self.d,code,int(down),0);self.x.XFlush(self.d)
 def press(self,name):self.key(name);self.key(name,False)
 def text(self,value):
  names={'/':'slash','.':'period','-':'minus',':':'colon','_':'underscore',' ':'space'}
  for c in value:self.press(names.get(c,c));time.sleep(.001)
 def screenshot(self,w,name):
  time.sleep(.5);subprocess.run(['import','-window',str(w),str(OUT/'screenshots'/('linux-'+name+'.png'))],capture_output=True,check=True)
if __name__=='__main__':
 x=X();server=Process([str(ROOT/'target/debug/cine-server'),'--bind','127.0.0.1:1728','--tls'])
 endpoint=server.receive()['endpoint']
 with tempfile.TemporaryDirectory(prefix='cine-p2p-desktop-') as tmp:
  invite=pathlib.Path(tmp)/'invite';invite.touch(mode=0o600)
  env=os.environ.copy();env.update(CINE_BRIDGE_LIBRARY=str(ROOT/'target/debug/libcine_ui_bridge.so'),GDK_BACKEND='x11',GTK_USE_PORTAL='0',CINE_P2P_QA_SERVER=endpoint,CINE_P2P_QA_INVITATION_FILE=str(invite),CINE_P2P_QA_SOURCE=str(ROOT/'test-media/long-duration.mp4'))
  app=subprocess.Popen([str(APP/'build/linux/x64/debug/bundle/cine_mobile_spike')],env=env,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
  observations=[]
  def collect():
   for line in app.stdout:
    if 'CINE_P2P_SAMPLE ' in line:
     try:observations.append(json.loads(line.split('CINE_P2P_SAMPLE ',1)[1]))
     except ValueError:pass
  reader=threading.Thread(target=collect,daemon=True);reader.start()
  try:
   end=time.monotonic()+30
   while not invite.stat().st_size and time.monotonic()<end:time.sleep(.1)
   if not invite.stat().st_size:raise RuntimeError('DESKTOP_JOIN_FAILED')
   time.sleep(2);w=x.window();x.focus(w);x.screenshot(w,'host-empty')
   print('desktop_window='+str(w),flush=True)
   print('Host UI ready; harness commands from stdin: click x y, key NAME, text VALUE, shot NAME, join, receive, state, quit',flush=True)
   peer=None
   for line in sys.stdin:
    parts=line.strip().split(' ',2)
    if not parts:continue
    if parts[0]=='click':x.click(w,int(parts[1]),int(parts[2]))
    elif parts[0]=='key':x.press(parts[1])
    elif parts[0]=='text':x.text(line.strip()[5:])
    elif parts[0]=='shot':x.screenshot(w,parts[1])
    elif parts[0]=='join':
     private=json.loads(invite.read_text());c=private['invitation'];peer=Process([str(ROOT/'target/debug/cine-client'),'--server',endpoint,'--name','Participant QA','--player','mpv']);peer.receive();peer.command('join {room_id} {room_epoch} {invite_token}'.format(**c));print('peer_joined',flush=True)
    elif parts[0]=='receive':print(peer.command('receive '+tmp)['event'],flush=True)
    elif parts[0]=='state':print(peer.command('transfer-state'),flush=True)
    elif parts[0]=='quit':break
    print('done',flush=True)
  finally:
   if peer:peer.stop()
   app.terminate();app.wait(timeout=10);server.stop()
   (OUT/'results-desktop-visual.json').write_text(json.dumps({'schema_version':1,'scenario':'physical-linux-product-host','observations':observations},indent=2)+'\n')
