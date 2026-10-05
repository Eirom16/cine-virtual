"""Parse YAML and check project-specific CI invariants; not an Actions emulator."""
import json
from pathlib import Path
import re
import sys
import tomllib

try:
    import yaml
except ImportError:
    raise SystemExit('Install validator dependency: python3 -m pip install -r .github/ci/requirements.txt')

ROOT = Path(__file__).resolve().parents[1]


class UniqueLoader(yaml.BaseLoader):
    """BaseLoader preserves GitHub's `on` key instead of YAML 1.1 boolean coercion."""
    def construct_mapping(self, node, deep=False):
        result = {}
        for key, value in node.value:
            k = self.construct_object(key, deep=deep)
            if k in result:
                raise ValueError(f'Duplicate YAML key: {k}')
            result[k] = self.construct_object(value, deep=deep)
        return result


def check():
    config = json.loads((ROOT / '.github/ci/toolchains.json').read_text())
    toolchain = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())
    assert toolchain['toolchain']['channel'] == config['rust'], 'Rust pins disagree'
    assert re.fullmatch(r'[a-f0-9]{40}', config['flutter_revision']), 'Flutter revision unpinned'
    required = ['docs/CI.md', '.github/ci/requirements.txt', 'scripts/ci_setup.py',
                'scripts/ci_build.py', 'experiments/02-rust-ui-bridge/app/pubspec.lock',
                'experiments/02-rust-ui-bridge/app/ios/Runner.xcodeproj/project.pbxproj',
                'experiments/02-rust-ui-bridge/app/windows/CMakeLists.txt',
                'experiments/02-rust-ui-bridge/app/macos/Runner.xcodeproj/project.pbxproj']
    for path in required:
        assert (ROOT / path).is_file(), f'Missing {path}'
    files = [ROOT / '.github/workflows/ci.yml'] + [ROOT / f'.github/workflows/build-{p}.yml'
              for p in ['linux', 'windows', 'macos', 'android', 'ios']]
    files += [ROOT / '.github/actions/setup/action.yml']
    for path in files:
        data = yaml.load(path.read_text(), Loader=UniqueLoader)
        assert isinstance(data, dict) and 'name' in data, f'Invalid YAML: {path.name}'
        if path.parent.name == 'workflows':
            assert data.get('permissions') == {'contents':'read'}, 'Unexpected permissions'
            assert 'concurrency' in data, 'Missing obsolete-run cancellation'
            events = data['on']
            if path.name == 'ci.yml':
                assert set(events) == {'push', 'pull_request', 'workflow_dispatch'}
                assert events['workflow_dispatch']['inputs']['platform']['options'] == ['all', 'linux', 'windows', 'macos', 'android', 'ios']
                for platform in ['linux', 'windows', 'macos', 'android', 'ios']:
                    job = data['jobs'][platform]
                    assert job['needs'] == 'base', 'Expensive build bypasses base gate'
                    assert job['if'] == f"github.event_name != 'workflow_dispatch' || inputs.platform == 'all' || inputs.platform == '{platform}'", 'Manual selection must not omit PR/push builds'
            else:
                assert set(events) == {'workflow_call', 'workflow_dispatch'}, 'Duplicate automatic triggers'
            for job in data['jobs'].values():
                if 'runs-on' in job:
                    assert 'timeout-minutes' in job, 'Unbounded CI job'
                for step in job.get('steps', []):
                    assert step.get('continue-on-error', 'false') != 'true', 'Hidden failure'
        text = path.read_text()
        assert 'pull_request_target' not in text and '${{ secrets.' not in text, 'Unsafe secret/event context'
        assert 'continue-on-error: true' not in text, 'Hidden job failure'
        for uses in re.findall(r'^\s*(?:- )?uses: (\S+)', text, re.M):
            if uses.startswith('./'):
                local = ROOT / uses[2:]
                assert local.is_file() or (local / 'action.yml').is_file(), f'Missing uses path {uses}'
            else:
                assert re.fullmatch(r'[\w-]+/[\w-]+@[a-f0-9]{40}', uses), f'Unpinned action: {uses}'
        for script in re.findall(r'python(?:3)? (scripts/[\w/.]+\.py)', text):
            assert (ROOT / script).is_file(), f'Missing script {script}'
    print(f'CI structure OK: {len(files)-1} workflows, pinned setup action, local paths and gates checked.')


if __name__ == '__main__':
    try:
        check()
    except (AssertionError, ValueError, KeyError, yaml.YAMLError) as error:
        print(f'CI validation failed: {error}', file=sys.stderr)
        raise SystemExit(1)
