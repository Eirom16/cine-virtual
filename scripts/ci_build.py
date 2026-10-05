"""Build and package experimental outputs. This script never publishes or signs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / 'experiments/02-rust-ui-bridge/app'
CONFIG = json.loads((ROOT / '.github/ci/toolchains.json').read_text())
APPLE_TARGETS = {'device': 'aarch64-apple-ios', 'simulator': 'aarch64-apple-ios-sim'}


def run(*args, cwd=ROOT, env=None):
    if args[0] == 'flutter':
        args = (shutil.which('flutter') or 'flutter', *args[1:])
    subprocess.run(args, cwd=cwd, env=env, check=True)


def capture(*args):
    if args[0] == 'flutter':
        args = (shutil.which('flutter') or 'flutter', *args[1:])
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def bridge_path(release=False):
    name = {'Windows': 'cine_ui_bridge.dll', 'Darwin': 'libcine_ui_bridge.dylib',
            'Linux': 'libcine_ui_bridge.so'}[platform.system()]
    return ROOT / 'target' / ('release' if release else 'debug') / name


def flutter_checks():
    run('flutter', '--version')
    run('flutter', 'pub', 'get', '--enforce-lockfile', cwd=APP)
    run('flutter', 'analyze', cwd=APP)
    env = os.environ.copy()
    env['CINE_BRIDGE_LIBRARY'] = str(bridge_path())
    run('flutter', 'test', cwd=APP, env=env)


def rust_build(args):
    if args.platform == 'ios':
        target = APPLE_TARGETS[args.variant]
        if platform.system() != 'Darwin':
            raise SystemExit('Apple compile requires macOS/Xcode; no cross-build hacks')
        run('rustup', 'target', 'add', target)
        run('xcodebuild', '-version')
        run('cargo', 'rustc', '--locked', '--release', '--lib', '-p', 'cine-ui-bridge',
            '--target', target, '--crate-type', 'staticlib')
        directory = APP / 'ios/Rust'
        directory.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / f'target/{target}/release/libcine_ui_bridge.a', directory)
    elif args.platform == 'android':
        run('python3', 'scripts/build_mobile_bridge.py', '--abi', args.arch)
    else:
        run('cargo', 'build', '--workspace', '--locked', '--release')
        run('cargo', 'test', '--workspace', '--locked')


def flutter_build(args):
    run('flutter', 'pub', 'get', '--enforce-lockfile', cwd=APP)
    if args.platform == 'ios':
        mode = '--release' if args.variant == 'device' else '--debug'
        command = ['flutter', 'build', 'ios', mode, '--no-codesign', '--no-pub']
        if args.variant == 'simulator':
            command.append('--simulator')
        run(*command, cwd=APP)
        binary = APP / ('build/ios/iphoneos/Runner.app/Runner' if args.variant == 'device'
                        else 'build/ios/iphonesimulator/Runner.app/Runner')
        symbols = capture('nm', '-g', str(binary))
        for symbol in ['cine_bridge_create', 'cine_bridge_call', 'cine_bridge_destroy', 'cine_bridge_hash_fd']:
            if '_' + symbol not in symbols:
                raise SystemExit('Missing linked iOS C ABI symbol: ' + symbol)
    elif args.platform == 'android':
        target = {'arm64-v8a': 'android-arm64', 'armeabi-v7a': 'android-arm',
                  'x86_64': 'android-x64'}[args.arch]
        # Debug SDK signing only; no Play/upload/release key. No autorun/private extras.
        run('flutter', 'build', 'apk', '--debug', '--no-pub', '--target-platform=' + target,
            '--dart-define=ROOM_MODE=true', cwd=APP)
    else:
        run('flutter', 'build', args.platform, '--release', '--no-pub', cwd=APP)
        if args.platform == 'linux':
            dest = APP / 'build/linux/x64/release/bundle/lib'
        elif args.platform == 'windows':
            dest = APP / 'build/windows/x64/runner/Release'
        else:
            dest = APP / 'build/macos/Build/Products/Release/cine_mobile_spike.app/Contents/Frameworks'
        dest.mkdir(parents=True, exist_ok=True)
        shutil.copy2(bridge_path(True), dest)


def make_archive(stage, archive):
    """Preserve executable bits/symlinks on Apple/Linux, ZIP on Windows/Android."""
    if archive.suffix == '.zip':
        with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as out:
            for path in sorted(stage.rglob('*')):
                if path.is_file():
                    out.write(path, path.relative_to(stage))
    else:
        run('tar', '-czf', str(archive), '-C', str(stage), '.')
    with archive.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    archive.with_name('CHECKSUMS.txt').write_text(f'{digest}  {archive.name}\n', encoding='utf-8')


def package(args):
    commit = capture('git', 'rev-parse', 'HEAD')
    suffix = '-unsigned' if args.platform == 'ios' else ''
    variant = '-' + args.variant if args.platform == 'ios' else ''
    name = f'cine-virtual-{args.platform}{suffix}-{args.arch}{variant}-{commit[:7]}'
    out = ROOT / 'dist' / name
    stage = out / 'payload'
    if out.exists():
        shutil.rmtree(out)
    stage.mkdir(parents=True)
    mode = 'release'
    if args.platform in ['linux', 'windows', 'macos']:
        executables = ['cine-server', 'cine-client', 'cine-player-spike']
        binary_dir = stage / 'cli'
        binary_dir.mkdir()
        for executable in executables:
            source = ROOT / 'target/release' / (executable + ('.exe' if args.platform == 'windows' else ''))
            shutil.copy2(source, binary_dir)
        if args.platform == 'linux':
            source = APP / 'build/linux/x64/release/bundle'
        elif args.platform == 'windows':
            source = APP / 'build/windows/x64/runner/Release'
        else:
            source = APP / 'build/macos/Build/Products/Release/cine_mobile_spike.app'
        shutil.copytree(source, stage / ('cine_mobile_spike.app' if args.platform == 'macos' else 'flutter'), symlinks=True)
    elif args.platform == 'android':
        mode = 'debug'
        apk = APP / 'build/app/outputs/flutter-apk/app-debug.apk'
        with zipfile.ZipFile(apk) as contents:
            members = set(contents.namelist())
            for library in ['libflutter.so', 'libcine_ui_bridge.so']:
                if f'lib/{args.arch}/{library}' not in members:
                    raise SystemExit('APK missing requested native ABI/library: ' + args.arch + '/' + library)
        shutil.copy2(apk, stage / 'cine-virtual-android.apk')
    else:
        mode = 'release' if args.variant == 'device' else 'debug'
        source = APP / ('build/ios/iphoneos/Runner.app' if args.variant == 'device'
                        else 'build/ios/iphonesimulator/Runner.app')
        # Must be unsigned; do not silently accept automatic/ad-hoc signing.
        if subprocess.run(['codesign', '-dv', str(source)], capture_output=True).returncode == 0:
            raise SystemExit('Unexpected signed iOS product; refusing unsigned artifact label')
        shutil.copytree(source, stage / 'Runner.app', symlinks=True)
        shutil.copy2(APP / 'ios/Rust/libcine_ui_bridge.a', stage)
    if args.platform == 'macos':
        expected = 'arm64' if args.arch == 'arm64' else 'x86_64'
        binaries = [stage / 'cine_mobile_spike.app/Contents/MacOS/cine_mobile_spike',
                    stage / 'cine_mobile_spike.app/Contents/Frameworks/libcine_ui_bridge.dylib']
        binaries += [stage / 'cli' / name for name in ['cine-server', 'cine-client', 'cine-player-spike']]
        for binary in binaries:
            architectures = capture('lipo', '-archs', str(binary))
            if architectures != expected:
                raise SystemExit('macOS component architecture differs from runner: ' + binary.name + ':' + architectures)
    metadata = {
        'schema_version': 1, 'commit': commit,
        'working_tree_dirty': bool(capture('git', 'status', '--porcelain')),
        'platform': args.platform,
        'architecture': args.arch, 'variant': args.variant if args.platform == 'ios' else None,
        'build_mode': mode, 'rust_build_mode': 'release',
        'runner_os': os.environ.get('RUNNER_OS', platform.system()),
        'runner_arch': os.environ.get('RUNNER_ARCH', platform.machine()),
        'os_version': platform.platform(), 'rust_version': capture('rustc', '--version'),
        'flutter': json.loads(capture('flutter', '--version', '--machine')),
        'runtime_validation': 'NOT TESTED by build job',
        'player': 'IOS PLAYER RUNTIME NOT IMPLEMENTED' if args.platform == 'ios' else
                  'Media3 Android provisional' if args.platform == 'android' else
                  'libmpv adapter compiled; desktop Flutter surface not integrated',
        'multimedia_sdk_bundled': args.platform == 'android',
        'experimental': True, 'project_license': 'PENDING; not a distribution approval',
    }
    # Flutter --machine exposes the installation path; do not archive runner paths.
    metadata['flutter'].pop('flutterRoot', None)
    if args.platform in ['ios', 'macos']:
        metadata['xcode_version'] = capture('xcodebuild', '-version')
    if args.platform == 'ios':
        metadata.update(signed=False, rust_target=APPLE_TARGETS[args.variant], format='compiled .app; no IPA')
    if args.platform == 'android':
        metadata.update(java_version=subprocess.check_output(['java', '-version'], text=True, stderr=subprocess.STDOUT).strip(),
                        android_api=CONFIG['android_api'], android_ndk=CONFIG['android_ndk'],
                        android_build_tools=CONFIG['android_build_tools'], jdk_version=CONFIG['jdk'],
                        signing='standard ephemeral SDK debug key; not release signing')
    (stage / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    shutil.copy2(stage / 'metadata.json', out)
    (stage / 'BUILD-NOTICE.txt').write_text(
        'Experimental pre-v0.1 CI build. No release/publication. Project license pending.\n'
        'Build PASS does not validate playback/runtime. No libmpv/FFmpeg runtime bundled.\n'
        'iOS Player unimplemented; Android APK debug only. See docs/CI.md.\n', encoding='utf-8')
    archive = out / (name + ('.zip' if args.platform in ['windows', 'android'] else '.tar.gz'))
    make_archive(stage, archive)
    shutil.rmtree(stage)
    print(f'Artifact assembled: {name}')
    if 'GITHUB_OUTPUT' in os.environ:
        with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as f:
            f.write(f'artifact_name={name}\nartifact_path={out.relative_to(ROOT).as_posix()}\n')


def summary(args):
    def status(outcome):
        return {'success': 'PASS', 'failure': 'FAIL'}.get(outcome, 'BLOCKED')
    rust = status(args.rust_outcome)
    player = ('NOT IMPLEMENTED' if args.platform == 'ios' else
              'COMPILE ' + status(args.flutter_outcome) if args.platform == 'android' else
              f'COMPILE {rust}')
    table = ('| Platform | Core/bridge | Flutter | Player | Package | Runtime |\n'
             '| --- | --- | --- | --- | --- | --- |\n'
             f'| {args.platform} {args.arch} {args.variant} | {rust} | {status(args.flutter_outcome)} | '
             f'{player} | {status(args.package_outcome)} | NOT TESTED |\n\n'
             'Compilation/linking only. Physical and headless SDK evidence is separate.\n')
    print(table)
    if 'GITHUB_STEP_SUMMARY' in os.environ:
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a', encoding='utf-8') as f:
            f.write(table)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('action', choices=['rust', 'flutter', 'package', 'summary', 'flutter-checks'])
    parser.add_argument('--platform', choices=['linux', 'windows', 'macos', 'android', 'ios'], default='linux')
    parser.add_argument('--arch', default='x64')
    parser.add_argument('--variant', choices=['device', 'simulator'], default='device')
    for name in ['rust', 'flutter', 'package']:
        parser.add_argument('--' + name + '-outcome', default='skipped')
    args = parser.parse_args()
    if args.action == 'flutter-checks':
        flutter_checks()
    else:
        {'rust': rust_build, 'flutter': flutter_build, 'package': package, 'summary': summary}[args.action](args)


if __name__ == '__main__':
    main()
