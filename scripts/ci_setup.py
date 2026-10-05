"""Pinned toolchains; no upgrades or credentials. Run from repository root."""
import argparse
import json
import os
import re
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
CONFIG = json.loads((ROOT / '.github/ci/toolchains.json').read_text())


def run(*args, cwd=ROOT):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def parse_machine_version(output):
    # Cold SDK bootstrap writes progress before the final machine JSON.
    for match in re.finditer(r'^\{', output, re.MULTILINE):
        try:
            value, _ = json.JSONDecoder().raw_decode(output[match.start():])
        except ValueError:
            continue
        if isinstance(value, dict) and 'frameworkRevision' in value:
            return value
    raise ValueError('Flutter did not return machine version JSON')


def flutter():
    sdk = Path(os.environ.get('CINE_FLUTTER_ROOT', str(ROOT / '.ci-cache/flutter')))
    if not (sdk / '.git').is_dir():
        sdk.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(['git', 'clone', '--depth', '1', '--branch', CONFIG['flutter_version'],
                        'https://github.com/flutter/flutter.git', str(sdk)], check=True)
    if run('git', 'rev-parse', 'HEAD', cwd=sdk) != CONFIG['flutter_revision']:
        raise SystemExit('Flutter cache revision differs from pinned revision; remove SDK cache')
    executable = str(sdk / 'bin' / ('flutter.bat' if os.name == 'nt' else 'flutter'))
    version = parse_machine_version(run(executable, '--version', '--machine'))
    for key, expected in [('frameworkRevision', CONFIG['flutter_revision']),
                          ('frameworkVersion', CONFIG['flutter_version']),
                          ('dartSdkVersion', CONFIG['dart_version'])]:
        if version[key] != expected:
            raise SystemExit(f'Flutter toolchain mismatch: {key}')
    if 'GITHUB_PATH' in os.environ:
        with open(os.environ['GITHUB_PATH'], 'a', encoding='utf-8') as f:
            f.write(str(sdk / 'bin') + '\n')
    print(json.dumps({k:version[k] for k in ['frameworkVersion', 'frameworkRevision', 'dartSdkVersion']}))


def android():
    sdk = os.environ.get('ANDROID_SDK_ROOT') or os.environ.get('ANDROID_HOME')
    if not sdk:
        raise SystemExit('GitHub-hosted Android SDK required (ANDROID_SDK_ROOT/ANDROID_HOME)')
    sdk = Path(sdk)
    manager = sdk / 'cmdline-tools/latest/bin/sdkmanager'
    if not manager.is_file():
        raise SystemExit('Android command-line tools missing')
    licenses = subprocess.run([str(manager), '--licenses'], input='y\n' * 100, text=True)
    licenses.check_returncode()
    subprocess.run([str(manager), f"platforms;android-{CONFIG['android_api']}",
                    f"build-tools;{CONFIG['android_build_tools']}",
                    f"ndk;{CONFIG['android_ndk']}"], check=True)
    if 'GITHUB_ENV' in os.environ:
        with open(os.environ['GITHUB_ENV'], 'a', encoding='utf-8') as f:
            f.write(f'ANDROID_SDK_ROOT={sdk}\n')
    targets = ['aarch64-linux-android', 'armv7-linux-androideabi', 'x86_64-linux-android']
    subprocess.run(['rustup', 'target', 'add', *targets], check=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('tool', choices=['flutter', 'android'])
    args = parser.parse_args()
    {'flutter': flutter, 'android': android}[args.tool]()


if __name__ == '__main__':
    main()
