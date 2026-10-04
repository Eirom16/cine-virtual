"""Linux localhost vertical slice: three executables, two in-process decoders.

Default run is 600 real seconds. Short runs are diagnostics, never long-run evidence.
Credentials exist only in subprocess pipes/private memory. Export selected metrics.
"""
import argparse
import ctypes
import ctypes.util
import datetime
import json
import math
import shutil
import subprocess
import time
from pathlib import Path
from demo_control import Process, ROOT, wait_state, execution

OUT = ROOT / 'experiments/06-real-vertical-slice'


def fields(process, event):
    return [r.get('fields', {}) for r in process.logs if r.get('fields', {}).get('event') == event]


def checked(client, command, expected):
    reply = client.command(command)
    assert reply.get('event') == expected, (command.split()[0], reply)
    return reply


def percentile(values, fraction):
    if not values:
        return None
    s = sorted(values)
    return round(s[max(0, math.ceil(fraction * len(s)) - 1)], 3)


def resources(process):
    base = Path('/proc') / str(process.process.pid)
    try:
        status = dict(line.split(':', 1) for line in (base / 'status').read_text().splitlines() if ':' in line)
        stat = (base / 'stat').read_text().split()
        return {'rss_kib': int(status['VmRSS'].split()[0]), 'threads': int(status['Threads']),
                'fds': len(list((base / 'fd').iterdir())), 'cpu_ticks': int(stat[13]) + int(stat[14])}
    except (OSError, KeyError):
        return None


def wait_player(client, predicate, timeout=10):
    start = time.monotonic()
    while time.monotonic() - start < timeout:
        s = client.command('sync-state')
        if predicate(s):
            return s, (time.monotonic() - start) * 1000
        time.sleep(.03)
    raise AssertionError('Real player did not converge')


def run(seconds, faults=False):
    for dep in ('ffmpeg', 'ffprobe'):
        assert shutil.which(dep), f'{dep} required'
    assert ctypes.util.find_library('mpv'), 'libmpv required'
    for binary in ('cine-server', 'cine-client'):
        assert (ROOT / 'target/debug' / binary).exists(), 'cargo build --workspace first'
    media = ROOT / 'test-media/long-duration.mp4'
    if not media.exists():
        subprocess.run(['python3', 'scripts/generate_test_media.py'], cwd=ROOT, check=True)
    assert media.exists()
    wrong = ROOT / 'test-media/identity-mismatch.mp4'
    shutil.copyfile(media, wrong)
    with wrong.open('ab') as file:
        file.write(b'cine-virtual-mismatch')
    probe = json.loads(subprocess.check_output(['ffprobe', '-v', 'quiet', '-show_entries',
                      'format=duration:stream=codec_name,codec_type,width,height,avg_frame_rate', '-of', 'json', str(media)]))
    duration = float(probe['format']['duration'])
    library = ctypes.CDLL(ctypes.util.find_library('mpv'))
    library.mpv_client_api_version.restype = ctypes.c_ulong
    api = library.mpv_client_api_version()
    version_info = {'libmpv_api': f'{api >> 16}.{api & 65535}',
                    'source_base_commit': subprocess.check_output(['git', 'rev-parse', '--short', 'HEAD'], cwd=ROOT, text=True).strip(),
                    'implementation': 'vertical-slice-1',
                    'mpv_package': subprocess.check_output(['pacman', '-Q', 'mpv'], text=True).strip() if shutil.which('pacman') else 'not queried'}
    assert duration > seconds + 15, 'Corpus must outlast real wall-clock run'
    processes = []
    report = {'schema_version': 1, 'scenario': 'real_synchronized_playback', 'platform': 'Linux',
              'candidate': 'libmpv (provisional Linux)', 'timestamp_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'wall_seconds_requested': seconds, 'faults_requested': faults, 'versions': version_info,
              'media': {'test_id': 'synthetic-long-duration', 'duration_ms': round(duration * 1000),
                        'size_bytes': media.stat().st_size, 'streams': probe['streams'], 'codecs': [s['codec_name'] for s in probe['streams']]}}
    try:
        server = Process([str(ROOT / 'target/debug/cine-server'), '--bind', '127.0.0.1:0'])
        processes.append(server)
        address = None
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline and not address:
            starts = fields(server, 'server_started')
            if starts:
                address = starts[0]['bind']
            time.sleep(.01)
        assert address, 'server startup failed'
        clients = []
        for name in ('Host', 'Participant'):
            c = Process([str(ROOT / 'target/debug/cine-client'), '--server', 'ws://' + address,
                         '--name', name, '--player', 'mpv'])
            processes.append(c)
            assert c.receive()['event'] == 'cli_connected'
            clients.append(c)
        a, b = clients
        invitation = checked(a, 'create', 'created')
        checked(b, 'join {room_id} {room_epoch} {invite_token}'.format(**invitation), 'joined')
        wait_state(a, lambda s: len(s['members']) == 2)
        checked(a, 'select ' + str(media), 'media_selected')
        wait_state(b, lambda s: s['sequence'] >= 3)
        report['not_loaded_ready'] = b.command('ready')
        assert report['not_loaded_ready']['message'] == 'MEDIA_NOT_READY'
        checked(b, 'select ' + str(wrong), 'media_selected')
        mismatch = b.command('ready')
        assert mismatch['message'] == 'MEDIA_MISMATCH', mismatch
        assert not b.command('state')['members'][1]['ready']
        report['mismatch'] = 'MEDIA_MISMATCH; participant remains not ready'
        checked(b, 'select ' + str(media), 'media_selected')
        report['hash'] = {'a': a.command('hash-status'), 'b': b.command('hash-status')}
        for c in clients:
            assert c.command('media')['identity_match'] is True
            checked(c, 'ready', 'ready')
        ready = wait_state(a, lambda s: all(m['ready'] for m in s['members']))
        rejected = b.command('play')
        assert rejected['message'] == 'NOT_AUTHORIZED'
        assert a.command('state')['sequence'] == ready['sequence']
        report['authority'] = 'NOT_AUTHORIZED; sequence unchanged'
        controls = []

        def control(command):
            r = checked(a, command, 'accepted')
            ea, eb = execution(a, r['sequence']), execution(b, r['sequence'])
            assert ea['expected_server_ms'] == eb['expected_server_ms']
            controls.append({'command': command, 'sequence': r['sequence'], 'deadline_server_ms': ea['expected_server_ms'],
                             'scheduler_lateness_a_ms': ea['lateness_ms'], 'scheduler_lateness_b_ms': eb['lateness_ms']})
            return r['sequence']

        control('play 5000')
        for c in clients:
            wait_player(c, lambda s: s['playing'] and not s['seeking'] and s['position_ms'] > 5000)
        began = time.monotonic()
        actions = set()
        pair_samples = []
        resource_samples = {name: [] for name in ('a', 'b')}
        fault_results = []
        resume_at = min(120, seconds * .5)
        while (elapsed := time.monotonic() - began) < seconds:
            if elapsed >= min(15, seconds * .15) and 'pause-seek-play' not in actions:
                actions.add('pause-seek-play')
                control('pause')
                for c in clients:
                    wait_player(c, lambda s: not s['playing'] and not s['seeking'])
                control('seek 10000')
                for c in clients:
                    wait_player(c, lambda s: not s['seeking'] and abs(s['position_ms'] - 10000) < 100)
                control('play')
            if elapsed >= min(30, seconds * .35) and 'playing-seek' not in actions:
                actions.add('playing-seek')
                target = a.command('sync-state')['position_ms'] + 1000
                control('seek ' + str(target))
                for c in clients:
                    wait_player(c, lambda s: s['playing'] and not s['seeking'] and abs(s['drift_ms']) < 150)
            if elapsed >= resume_at and 'resume' not in actions:
                actions.add('resume')
                disconnect_server = b.command('sync-state')['server_ms']
                checked(b, 'disconnect', 'disconnected')
                wait_state(a, lambda s: not s['members'][1]['connected'])
                time.sleep(min(3, seconds * .1))
                start = time.monotonic()
                checked(b, 'resume', 'resumed')
                resume_ms = (time.monotonic() - start) * 1000
                initial = b.command('sync-state')
                converged, convergence_ms = wait_player(b, lambda s: s['playing'] and not s['seeking'] and abs(s['drift_ms']) <= 150)
                sa, sb = a.command('state'), b.command('sync')
                assert sa['room_id'] == sb['room_id'] and sa['sequence'] == sb['sequence']
                report['resume'] = {'disconnect_server_ms': disconnect_server, 'duration_ms': round(resume_ms, 3),
                                    'initial_drift_ms': initial['drift_ms'], 'convergence_after_response_ms': round(convergence_ms, 3),
                                    'final_drift_ms': converged['drift_ms'], 'snapshot_same_sequence': True}
            if faults and elapsed >= seconds * .72 and 'faults' not in actions:
                actions.add('faults')
                soft_target = b.command('sync-state')['position_ms'] + 120
                for command in (f'fault seek {soft_target}', 'fault pause', 'fault clock 1600', 'fault seek 120000'):
                    checked(b, command, 'fault_applied')
                    time.sleep(2)
                    if command.startswith('fault clock'):
                        assert not b.command('state')['members'][1]['ready']
                        checked(b, 'ready', 'ready')
                    _, latency = wait_player(b, lambda s: s['playing'] and not s['seeking'] and abs(s['drift_ms']) < 150)
                    fault_results.append({'command': command, 'convergence_ms': round(latency + 2000, 3)})
            pa, pb = a.command('sync-state'), b.command('sync-state')
            # Raw cross-process comparison and projection to B's sample timestamp.
            delta_time = pb['server_ms'] - pa['server_ms']
            projected_a = pa['position_ms'] + (delta_time * pa['rate'] if pa['playing'] else 0)
            pair_samples.append({'elapsed_s': round(time.monotonic() - began, 3),
                                 'raw_difference_ms': abs(pa['position_ms'] - pb['position_ms']),
                                 'sample_time_difference_ms': delta_time,
                                 'aligned_difference_ms': round(abs(projected_a - pb['position_ms']), 3)})
            for name, c in zip(('a', 'b'), clients):
                r = resources(c)
                if r:
                    resource_samples[name].append({'elapsed_s': round(time.monotonic() - began, 3), **r})
            if int(elapsed) % 30 == 0:
                print(json.dumps({'progress_s': round(elapsed), 'drift_a_ms': pa['drift_ms'], 'drift_b_ms': pb['drift_ms']}), flush=True)
            time.sleep(.9)
        report['wall_seconds_observed'] = round(time.monotonic() - began, 3)
        control('pause')
        for c in clients:
            wait_player(c, lambda s: not s['playing'] and not s['seeking'])
        report['final_position_difference_ms'] = abs(a.command('state')['position_ms'] - b.command('state')['position_ms'])
        report['controls'] = controls
        report['faults'] = fault_results
        report['clients'] = {}
        for name, c in zip(('a', 'b'), clients):
            clock = c.command('state')['clock']
            checked(c, 'quit', 'quit')
            c.process.wait(timeout=5)
            c.logger.join(timeout=2)
            samples = fields(c, 'sync_sample')
            valid = [s for s in samples if s['ready'] and s['clock_trusted'] and not s['seeking'] and not s['buffering'] and s['sample_age_ms'] <= 100 and s['drift_ms'] is not None]
            drift = [abs(s['drift_ms']) for s in valid]
            dispatch = [s for s in fields(c, 'player_dispatch') if s['reason'] == 'scheduled' and s['kind'] in ('play', 'pause', 'seek')]
            primary = []
            control_metrics = []
            events = fields(c, 'player_event')
            for ctl in controls:
                kind = ctl['command'].split()[0]
                action = next((d for d in dispatch if d['sequence'] == ctl['sequence'] and d['kind'] == kind), None)
                if action:
                    primary.append(action)
                completed = [e for e in events if e['sequence'] == ctl['sequence'] and e.get('reason') not in ('correction', 'snapshot', 'disconnect') and e['kind'] in ('playing', 'paused', 'seek_completed', 'first_position_advance', 'pause_stable')]
                control_metrics.append({'command': ctl['command'], 'sequence': ctl['sequence'], 'primary_dispatch': action, 'completion_events': completed})
            late = [s['actual_server_ms'] - s['deadline_server_ms'] for s in primary]
            corrections = [s for s in samples if s['correction'] in ('rate', 'restore', 'seek')]
            lifecycle = fields(c, 'player_resources')
            for r in lifecycle:
                r['resources'] = json.loads(r['resources'])
            report['clients'][name] = {'clock': clock, 'drift_p50_ms': percentile(drift, .5), 'drift_p95_ms': percentile(drift, .95),
                        'drift_p99_ms': percentile(drift, .99), 'drift_max_ms': max(drift, default=None),
                        'soft_corrections': sum(s['correction'] == 'rate' for s in corrections),
                        'restore_corrections': sum(s['correction'] == 'restore' for s in corrections),
                        'hard_seeks': sum(s['correction'] == 'seek' for s in corrections),
                        'soft_corrections_per_minute': round(sum(s['correction'] == 'rate' for s in corrections) * 60 / report['wall_seconds_observed'], 3),
                        'hard_seeks_per_minute': round(sum(s['correction'] == 'seek' for s in corrections) * 60 / report['wall_seconds_observed'], 3),
                        'missed_deadlines_over_50_ms': sum(v > 50 for v in late), 'dispatch_lateness_max_ms': max(late, default=None),
                        'samples_included': len(valid), 'samples_excluded': len(samples) - len(valid),
                        'sampling_policy': 'All locally loaded, trusted, nonseeking/nonbuffering samples <=100ms old; paused/preparation samples included; raw samples retained',
                        'samples': samples, 'dispatches': dispatch, 'control_metrics': control_metrics, 'events': events,
                        'lifecycle': lifecycle, 'resource_samples': resource_samples[name]}
        difference = [s['aligned_difference_ms'] for s in pair_samples]
        report['pair'] = {'aligned_p50_ms': percentile(difference, .5), 'aligned_p95_ms': percentile(difference, .95),
                          'aligned_p99_ms': percentile(difference, .99), 'aligned_max_ms': max(difference), 'samples': pair_samples}
        report['p95_experimental_goal_passed'] = all(c['drift_p95_ms'] <= 150 for c in report['clients'].values())
        # Only metric fields above are exported, never protocol envelopes/credentials.
        forbidden = ('invite_token', 'resume_token', 'digest', 'payload', 'path')
        assert all(not any(k in record.get('fields', {}) for k in forbidden) for p in processes for record in p.logs)
        report['privacy_check'] = 'no credentials, hashes, local paths or bytes exported'
        server.stop()
        assert all(p.process.poll() is not None for p in processes)
        report['process_cleanup'] = 'all three child processes reaped; no external player process spawned'
        report['result'] = 'PASS'
        OUT.mkdir(parents=True, exist_ok=True)
        target = OUT / ('results-faults-linux.json' if faults else 'results-linux.json' if seconds >= 590 else 'results-short-linux.json')
        target.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
        print(json.dumps({k: v for k, v in report.items() if k not in ('clients', 'pair')}, ensure_ascii=False, indent=2))
        print(json.dumps({'clients': {n: {k: v for k, v in c.items() if k not in ('samples', 'dispatches', 'control_metrics', 'events', 'lifecycle', 'resource_samples')} for n, c in report['clients'].items()},
                          'pair': {k: v for k, v in report['pair'].items() if k != 'samples'}}, indent=2))
    finally:
        for p in reversed(processes):
            p.stop()
        wrong.unlink(missing_ok=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--seconds', type=int, default=600)
    parser.add_argument('--faults', action='store_true')
    args = parser.parse_args()
    assert args.seconds >= 15
    run(args.seconds, args.faults)
