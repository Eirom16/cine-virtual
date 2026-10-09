"""Build the experimental C ABI for Android; toolchains are supplied, not installed."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--abi', choices=['x86_64', 'arm64-v8a', 'armeabi-v7a'], default='x86_64')
    args = parser.parse_args()
    sdk = Path(os.environ.get('ANDROID_SDK_ROOT', str(Path.home()/'.local/share/cine-spike-tools/android')))
    ndk = sdk/'ndk/28.2.13676358/toolchains/llvm/prebuilt/linux-x86_64/bin'
    target, compiler = {
        'x86_64': ('x86_64-linux-android', 'x86_64-linux-android24-clang'),
        'arm64-v8a': ('aarch64-linux-android', 'aarch64-linux-android24-clang'),
        'armeabi-v7a': ('armv7-linux-androideabi', 'armv7a-linux-androideabi24-clang'),
    }[args.abi]
    if not (ndk/compiler).is_file():
        raise SystemExit('Required Android NDK 28.2 toolchain missing')
    env = os.environ.copy()
    env['CARGO_TARGET_'+target.upper().replace('-', '_')+'_LINKER'] = str(ndk/compiler)
    env["CC_"+target.replace("-", "_")]=str(ndk/compiler)
    env["AR_"+target.replace("-", "_")]=str(ndk/"llvm-ar")
    subprocess.run(['cargo', 'build', '-p', 'cine-ui-bridge', '--release', '--locked', '--target', target], cwd=ROOT, env=env, check=True)
    out = ROOT/'experiments/02-rust-ui-bridge/app/android/app/src/main/jniLibs'/args.abi
    out.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT/'target'/target/'release/libcine_ui_bridge.so', out/'libcine_ui_bridge.so')
    print(f'Android bridge built: {args.abi}')
if __name__ == '__main__':
    main()
