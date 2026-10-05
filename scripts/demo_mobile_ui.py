"""Reproduce Spike C on Linux UI or a single Android emulator/device.

Toolchains live outside Git. Android selects the synthetic clip through the real
SAF picker; there is no injected path/content URI. No serial/URI/hash in results.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import re
import select
import subprocess
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
EXP = ROOT/'experiments/02-rust-ui-bridge'
APP = EXP/'app'
TOOLS = Path(os.environ.get('CINE_SPIKE_TOOLS', str(Path.home()/'.local/share/cine-spike-tools')))
PACKAGE = 'dev.cinevirtual.cine_mobile_spike'

def run(args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)

def result_from(line, parts):
    marker = 'CINE_SPIKE_RESULT_PART '
    if marker not in line:
        return None
    numbering, fragment = line.split(marker, 1)[1].strip().split(' ', 1)
    index, count = map(int, numbering.split('/'))
    parts[index] = fragment
    if not all(i in parts for i in range(count)):
        return None
    result = json.loads(base64.b64decode(''.join(parts[i] for i in range(count))))
    text = json.dumps(result)
    if re.search(r'/home/|content://|[a-f0-9]{64}', text):
        raise RuntimeError('Private data in evidence')
    result['versions'] = {'flutter': '3.47.6', 'dart': '3.13.5', 'bridge_api': 1}
    return result

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--platform', choices=['linux', 'android'], required=True)
    parser.add_argument('--no-build', action='store_true')
    args = parser.parse_args()
    parts = {}
    env = os.environ.copy()
    env['PATH'] = str(TOOLS/'flutter/bin')+os.pathsep+str(TOOLS/'bin')+os.pathsep+env['PATH']
    env['ANDROID_SDK_ROOT'] = str(TOOLS/'android')
    env['JAVA_HOME'] = str(TOOLS/'jdk21')
    env['CINE_BRIDGE_LIBRARY'] = str(ROOT/'target/debug/libcine_ui_bridge.so')
    env['GDK_BACKEND'] = 'x11'
    if args.platform == 'linux':
        if not args.no_build:
            run(['cargo', 'build', '-p', 'cine-ui-bridge'], cwd=ROOT, env=env)
            run(['flutter', 'build', 'linux', '--debug', '--dart-define=SPIKE_AUTORUN=true'], cwd=APP, env=env)
        proc = subprocess.Popen([str(APP/'build/linux/x64/debug/bundle/cine_mobile_spike')], env=env,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            deadline = time.monotonic()+40
            while time.monotonic() < deadline:
                if select.select([proc.stdout], [], [], .5)[0]:
                    value = result_from(proc.stdout.readline(), parts)
                    if value is not None:
                        break
            else:
                raise RuntimeError('Linux UI timeout')
        finally:
            proc.terminate()
            proc.wait(timeout=10)
        output = EXP/'results-linux-ui.json'
    else:
        adb = str(TOOLS/'android/platform-tools/adb')
        devices = run([adb, 'devices'], capture_output=True).stdout.splitlines()[1:]
        online = [line.split()[0] for line in devices if line.endswith('\tdevice')]
        if len(online) != 1:
            raise SystemExit('Exactly one authorized Android device/emulator required')
        def device(*command, check=True):
            return subprocess.run([adb, '-s', online[0], *command], check=check, capture_output=True, text=True, timeout=90 if 'install' in command else 20).stdout
        abi = device('shell', 'getprop', 'ro.product.cpu.abi').strip()
        if abi not in ['x86_64', 'arm64-v8a', 'armeabi-v7a']:
            raise SystemExit('This spike builds x86_64, arm64-v8a or armeabi-v7a')
        if not args.no_build:
            run(['python3', 'scripts/build_mobile_bridge.py', '--abi', abi], cwd=ROOT, env=env)
            target = {'x86_64':'android-x64', 'arm64-v8a':'android-arm64',
                      'armeabi-v7a':'android-arm'}[abi]
            run(['flutter', 'build', 'apk', '--debug', '--target-platform='+target,
                 '--dart-define=SPIKE_AUTORUN=true',
                 '--dart-define=SPIKE_SILENT='+str(online[0].startswith('emulator-')).lower()], cwd=APP, env=env)
        if not (ROOT/'test-media/normal.mp4').is_file():
            run(['python3', 'scripts/generate_test_media.py'], cwd=ROOT)
        check_existing = subprocess.run([adb, '-s', online[0], 'shell', 'test', '-e',
                         '/sdcard/Download/cine-spike.mp4'], capture_output=True, timeout=20)
        if check_existing.returncode == 0:
            raise SystemExit('Fixture name already exists; refusing to overwrite a device file')
        device('push', str(ROOT/'test-media/normal.mp4'), '/sdcard/Download/cine-spike.mp4')
        device('shell', 'am', 'broadcast', '-a', 'android.intent.action.MEDIA_SCANNER_SCAN_FILE',
               '-d', 'file:///sdcard/Download/cine-spike.mp4')
        device('install', '-r', str(APP/'build/app/outputs/flutter-apk/app-debug.apk'))
        since = device('shell', "date '+%m-%d %H:%M:%S.000'").strip()
        device('shell', 'input', 'keyevent', 'KEYCODE_WAKEUP')
        device('shell', 'am', 'start', '-n', PACKAGE+'/.MainActivity')
        resumed = False
        picked = False
        system_ui_waits = 0
        deadline = time.monotonic()+120
        value = None
        try:
            while time.monotonic() < deadline:
                logs = device('logcat', '-d', '-T', since, '-s', 'flutter:I', 'CineSpike:I', '*:S')
                for line in logs.splitlines():
                    result = result_from(line, parts)
                    if result is not None:
                        value = result
                        break
                if value is not None:
                    break
                if 'CINE_SPIKE_LIFECYCLE_BACKGROUND' in logs and not resumed:
                    time.sleep(1.0)
                    device('shell', 'am', 'start', '-n', PACKAGE+'/.MainActivity')
                    resumed = True
                if not picked:
                    device('shell', 'uiautomator', 'dump', '/sdcard/cine-ui.xml', check=False)
                    xml = device('shell', 'cat', '/sdcard/cine-ui.xml', check=False)
                    try:
                        nodes = list(ET.fromstring(xml).iter('node'))
                        system_anr = any(n.get('text') == "System UI isn't responding" for n in nodes)
                        if system_anr:
                            choices = [n for n in nodes if n.get('text') == 'Wait']
                            system_ui_waits += 1
                        else:
                            choices = [n for n in nodes if n.get('text') == 'cine-spike.mp4']
                        if not choices and not system_anr and any(n.get('resource-id')=='com.android.documentsui:id/dir_list' for n in nodes):
                            device('shell','input','swipe','300','900','300','250','300')
                            continue
                        if not choices:
                            choices = [n for n in nodes if n.get('text') in ['Downloads', 'Download', 'Descargas'] and n.get('resource-id')!='android:id/title']
                        if not choices:
                            choices = [n for n in nodes if n.get('content-desc') in ['Show roots', 'Open navigation drawer', 'Mostrar raíces', 'Abrir panel de navegación']]
                        if choices:
                            n = choices[-1]
                            coordinates = [int(x) for x in re.findall(r'\d+', n.get('bounds', ''))]
                            if len(coordinates) == 4:
                                device('shell', 'input', 'tap', str((coordinates[0]+coordinates[2])//2), str((coordinates[1]+coordinates[3])//2))
                                picked = n.get('text') == 'cine-spike.mp4'
                    except ET.ParseError:
                        pass
                time.sleep(.25)
            if value is None:
                value = {'platform':'android', 'passed':False,
                         'failure':'ANDROID_SCENARIO_TIMEOUT',
                         'versions':{'flutter':'3.47.6','dart':'3.13.5','bridge_api':1}}
            value['environment'] = {'system_ui_wait_dialogs': system_ui_waits}
            value['device'] = {'android_version': device('shell','getprop','ro.build.version.release').strip(),
                               'api': device('shell','getprop','ro.build.version.sdk').strip(), 'abi': abi,
                               'model': device('shell','getprop','ro.product.model').strip(),
                               'emulator': online[0].startswith('emulator-')}
            value['versions']['media3'] = '1.11.1'
            device('shell', 'input', 'keyevent', 'KEYCODE_BACK')
            time.sleep(.5)
            device('shell', 'input', 'keyevent', 'KEYCODE_BACK')
            released = False
            for _ in range(20):
                logs = device('logcat', '-d', '-T', since, '-s', 'CineSpike:I', '*:S')
                if 'CINE_SPIKE_PLAYER_RELEASED' in logs:
                    released = True
                    break
                time.sleep(.25)
            value['native_player_released_on_exit'] = released
            if not released:
                value['passed'] = False
                value.setdefault('failure', 'PLAYER_RELEASE_NOT_OBSERVED')
        finally:
            device('shell', 'am', 'force-stop', PACKAGE)
            device('shell', 'rm', '/sdcard/cine-ui.xml', '/sdcard/Download/cine-spike.mp4', check=False)
        output = EXP/('results-android.json' if online[0].startswith('emulator-')
                      else 'results-android-device.json')
    output.write_text(json.dumps(value, indent=2)+'\n')
    print(json.dumps(value, indent=2))
    if not value.get('passed'):
        raise SystemExit('Spike scenario failed; evidence retained')

if __name__ == '__main__':
    try:
        main()
    except subprocess.CalledProcessError:
        raise SystemExit('ADB/build command failed; device identifiers omitted') from None
    except subprocess.TimeoutExpired:
        raise SystemExit('ADB command timeout; device identifiers omitted') from None
