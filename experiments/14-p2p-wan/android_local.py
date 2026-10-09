"""Opt-in on-device synthetic loopback relay: never uses a mobile socket.
Build the ARMv7/ARM64 release cine-wan-spike first; do not use a Phase 1 binary.
"""
import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
REMOTE = '/data/local/tmp/cine-wan-local-spike'


def adb(*args):
    return subprocess.run(['adb', *args], capture_output=True, text=True,
                          check=True, timeout=120).stdout.strip()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path,
                        default=Path(__file__).with_name('results-android-local-relay.json'))
    args = parser.parse_args()
    report = {'schema_version': 1, 'scenario': 'Android on-device loopback relay carrier',
              'network': 'on-device loopback; no mobile data; not WAN',
              'device': {'model': adb('shell', 'getprop', 'ro.product.model'),
                         'api': adb('shell', 'getprop', 'ro.build.version.sdk'),
                         'abi': adb('shell', 'getprop', 'ro.product.cpu.abi')},
              'build': 'release Rust', 'binary_bytes': args.binary.stat().st_size,
              'runs': []}
    try:
        adb('push', str(args.binary.resolve()), REMOTE)
        adb('shell', 'chmod', '700', REMOTE)
        for size in (8192, 8 * 1048576):
            result = json.loads(adb('shell', REMOTE, str(size)))
            report['runs'].append(result)
            assert result['status'] == 'LOCAL_SPIKE_PASS' and result['sha_final_match']
    finally:
        adb('shell', 'rm', '-f', REMOTE)
        args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': 'LOCAL_SPIKE_PASS', 'real_wan': 'NOT TESTED',
                      'runs': len(report['runs'])}))


if __name__ == '__main__':
    main()
