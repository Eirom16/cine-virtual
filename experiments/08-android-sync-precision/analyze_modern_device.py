"""Offline A/B analysis of existing telemetry; no playback or metric filter changes.

Run from any directory with Python 3. The JSON contains per-run distributions,
not a pooled percentile. First position advancement is interval-censored by the
existing driver trace. This is SDK position, not presented video/audio.
"""
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from analyze_android_timing import analyze  # noqa: E402
from demo_cross_platform import stats, valid  # noqa: E402


def first_play_advance(report):
    events = report['android_events']
    calls = [e for e in events if e['event'] == 'media3_call']
    play = next(e for e in calls if e['action'] == 'play' and e['reason'] == 'scheduled')
    begin, position = play['sdk_ms'], play['position_ms']
    # Stop before an unrelated seek/pause can produce a position jump.
    end = min((e['sdk_ms'] for e in calls if e['sdk_ms'] > begin
               and e['action'] in ['seek', 'pause']), default=float('inf'))
    observations = sorted((e for e in events if e['event'] == 'driver_observation'
                           and begin < e['source_sdk_ms'] < end),
                          key=lambda e: e['source_sdk_ms'])
    advanced = next((e for e in observations if e['playing'] and not e['seeking']
                     and not e['buffering'] and e['position_ms'] > position), None)
    if advanced is None:
        return {'status': 'UNTESTED', 'reason': 'No uncontaminated position advance trace'}
    finish = advanced['source_sdk_ms']
    # A same-position observation bounds the onset from below. Never use
    # isPlaying=true as evidence that the position has already advanced.
    same = [begin] + [e['source_sdk_ms'] for e in observations
                      if e['source_sdk_ms'] < finish and e['position_ms'] == position
                      and not e['seeking'] and not e['buffering']]
    same += [e['sdk_ms'] for e in events
             if e['event'] in ['playing_completed', 'playing_changed']
             and e.get('operation_id') == play['operation_id']
             and begin <= e['sdk_ms'] < finish and e.get('position_ms') == position
             and e.get('playing') is True]
    effect = next(e for e in events if e['event'] == 'effect_received'
                  and e['operation_id'] == play['operation_id'])
    dispatch_lateness = (begin + effect['source_mapped_at_ms']
                         - effect['sample_source_sdk_ms'] - effect['deadline_local_ms'])
    rates = [e for e in calls if begin < e['sdk_ms'] < finish and e['action'] == 'rate']
    return {
        'status': 'MEASURED_INTERVAL',
        'operation_id': play['operation_id'], 'sequence': play['sequence'],
        'generation': play['generation'],
        'dispatch_to_first_advance_lower_bound_ms': max(same) - begin,
        'dispatch_to_first_advance_upper_bound_ms': finish - begin,
        'initial_play_dispatch_lateness_ms': dispatch_lateness,
        'deadline_to_first_advance_lower_bound_ms': dispatch_lateness + max(same) - begin,
        'deadline_to_first_advance_upper_bound_ms': dispatch_lateness + finish - begin,
        'clock_mapping_bracket_ms': effect['clock_bracket_ms'],
        'first_observed_position_delta_ms': advanced['position_ms'] - position,
        'effective_lost_advance_at_first_trace_ms':
            (finish - begin - (advanced['position_ms'] - position)) if not rates else None,
        'effective_lost_advance_status': 'INFERRED: elapsed minus SDK position advance, '
                                        'assuming rate 1 without intervening rate/seek; '
                                        'not an exact first-advance timestamp',
        'clock_domain': 'Kotlin elapsedRealtime for startup interval; deadline '
                        'mapped through the same operation source bracket into Rust',
        'limitation': 'Existing driver trace every 25 polls; onset is not exact, '
                      'SDK position is not physical frame/audio presentation',
    }


def summarize(path):
    report = json.loads(path.read_text())
    assert report['passed'], f'{path.name}: functional failure must be examined separately'
    assert report['build_mode'] == 'profile' and report['controls_only']
    assert not report['faults_requested'] and report['diagnostic']
    samples = [s['sync'] for s in report['android_samples'] if valid(s.get('sync'))]
    recomputed = stats([abs(s['drift_ms']) for s in samples])
    # Audit the entire frozen population, including count and maximum.
    for key, value in recomputed.items():
        assert abs(value - report['android_drift'][key]) < 1e-9, (path.name, key)
    pair_stats = stats([s['difference_ms'] for s in report['pair_samples']])
    for key, value in pair_stats.items():
        assert abs(value - report['cross_device_difference'][key]) < 1e-9, (path.name, key)
    timing = analyze(report)
    effects = [e for e in report['android_events'] if e['event'] == 'effect_received']
    command_looper = {
        'lateness_after_due_ms': stats([e['drive_execution_ms'] - e['drive_due_ms']
                                        for e in effects]),
        'post_to_execution_ms': stats([e['drive_execution_ms'] - e['drive_post_ms']
                                      for e in effects]),
        'policy': 'Each effect references its polling runnable; post-to-execution '
                  'includes deliberate 20ms polling period. Multiple effects can '
                  'share a runnable; this is effect-weighted, not unique-runnable weighted.',
    }
    controls = [e for e in report['android_events'] if e['event'] == 'media3_call'
                and e['reason'] == 'scheduled']
    start = next(e['sdk_ms'] for e in controls if e['action'] == 'play')
    finish = max(e['sdk_ms'] for e in controls if e['action'] == 'pause')
    resources = [r for r in report['android_resources']
                 if start <= r['sample_monotonic_ms'] <= finish]
    assert len(resources) >= 2
    first, last = resources[0], resources[-1]
    cpu_seconds = (last['sample_monotonic_ms'] - first['sample_monotonic_ms']) / 1000
    cpu_window = {
        'status': 'MEASURED', 'window_seconds': cpu_seconds,
        'sample_count': len(resources),
        'cpu_percent_one_core': (last['cpu_ticks'] - first['cpu_ticks'])
                               / first['clock_ticks_per_second'] / cpu_seconds * 100,
        'policy': 'First resource sample after initial scheduled Play through last '
                  'resource sample before final scheduled Pause; no interpolation',
    }
    sequences = sorted({s['sequence'] for s in samples})
    phase_stats = {str(seq): stats([abs(s['drift_ms']) for s in samples
                                   if s['sequence'] == seq]) for seq in sequences}
    if path.name.startswith('modern-device-run-'):
        path.with_name(path.stem + '-timing.json').write_text(json.dumps(timing, indent=2) + '\n')
    return {
        'file': path.name, 'status': 'MEASURED', 'functional_status': report['status'],
        'android_p95_goal_met': report['android_drift']['p95_ms'] <= 150,
        'cross_device_p95_goal_met': report['cross_device_difference']['p95_ms'] <= 150,
        'source_base_commit': report['source_base_commit'],
        'source_worktree_dirty': report.get('source_worktree_dirty'),
        'build_mode': report['build_mode'], 'device': report['device'],
        'wall_seconds': report['real_wall_seconds'], 'media_size_bytes': report['media_size_bytes'],
        'android_drift': report['android_drift'], 'linux_drift': report['linux_drift'],
        'android_drift_by_sequence_descriptive_only': phase_stats,
        'cross_device_difference': report['cross_device_difference'],
        'corrections': report['corrections']['android']['counts'],
        'cpu_percent_one_core': report['resources_summary']['android']['cpu_percent_one_core'],
        'controls_window_cpu': cpu_window,
        'timing_ms': timing['timing_ms'],
        'command_looper': command_looper,
        'hard_seek_classifications': timing['hard_seek_classifications'],
        'first_play_position_advance': first_play_advance(report),
        'recovery': report['recovery'],
        'audit': 'TESTED: primary Android and pair aggregates reproduce from raw samples',
        'teardown_observed': report['teardown_observed'],
        'processes_reaped': report['processes_reaped'],
    }


def main():
    directory = Path(__file__).resolve().parent
    old = [summarize(directory / f'results-after-profile-{i}.json') for i in range(1, 4)]
    modern_paths = sorted(directory.glob('modern-device-run-[1-5].json'))
    assert len(modern_paths) >= 3, 'At least three completed runs required'
    new = [summarize(path) for path in modern_paths]
    assert len({r['media_size_bytes'] for r in old + new}) == 1
    assert len({r['source_base_commit'] for r in new}) == 1
    assert len({r['device']['abi'] for r in new}) == 1
    assert all(r['device']['abi'] == 'arm64-v8a' for r in new)
    result = {
        'schema_version': 1,
        'analysis_status': 'MEASURED per-run metrics; inference belongs to comparison report',
        'goal_ms': 150, 'old_profile_runs': old, 'modern_profile_runs': new,
        'modern_android_pass_count': sum(r['android_p95_goal_met'] for r in new),
        'modern_cross_device_pass_count': sum(r['cross_device_p95_goal_met'] for r in new),
        'source_limitations': [
            'Historical Profile APKs used V1; current cc7544b uses V2, which keeps '
            'measuring seek loss after a position stabilized while paused. '
            'Historical APK hashes are unavailable; binary identity is unproven.',
            'Only modern phone connected; historical comparison is sequential, '
            'not contemporaneous randomized A/B. Device, OS, ABI and load vary together.',
            'LAN transport and Linux host/fixture are reused; historical Wi-Fi '
            'identity and interference cannot be independently verified.',
            'Recovery is measured separately; historical 600s recovery was Debug '
            'and cannot be pooled with the controls-only Profile comparison.',
        ],
    }
    recovery_path = directory / 'modern-device-recovery.json'
    if recovery_path.exists():
        recovery = json.loads(recovery_path.read_text())
        rs = [s['sync'] for s in recovery['android_samples'] if valid(s.get('sync'))]
        for key, value in stats([abs(s['drift_ms']) for s in rs]).items():
            assert abs(value - recovery['android_drift'][key]) < 1e-9, ('recovery', key)
        for key, value in stats([s['difference_ms'] for s in recovery['pair_samples']]).items():
            assert abs(value - recovery['cross_device_difference'][key]) < 1e-9, ('recovery pairs', key)
        result['modern_recovery'] = {
            'file': recovery_path.name, 'functional_status': recovery['status'],
            'build_mode': recovery['build_mode'], 'recovery': recovery.get('recovery', {}),
            'android_drift_separate_population': recovery.get('android_drift'),
            'cross_device_separate_population': recovery.get('cross_device_difference'),
            'corrections': recovery.get('corrections', {}).get('android'),
            'audit': 'TESTED: Android and pair aggregates reproduce from raw samples',
            'convergence_limitation': 'Inherited wait counts three valid samples <=150ms '
                                      'anywhere since the action index, not consecutive; '
                                      'foreground index begins before HOME. Wait timing '
                                      'is not an exact post-Ready settling interval.',
        }
    (directory / 'modern-device-comparison.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'old_p95_ms': [r['android_drift']['p95_ms'] for r in old],
                      'new_p95_ms': [r['android_drift']['p95_ms'] for r in new],
                      'modern_android_pass_count': result['modern_android_pass_count'],
                      'modern_cross_device_pass_count': result['modern_cross_device_pass_count']}))


if __name__ == '__main__':
    main()
