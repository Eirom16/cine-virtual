"""Analyze sanitized experiment telemetry. Never compare uncalibrated clock origins."""
import argparse
import collections
import json
from pathlib import Path
from demo_cross_platform import stats


def analyze(report):
    events = report.get('android_events', [])
    receives = [e for e in events if e['event'] == 'effect_received']
    calls = {e['operation_id']: e for e in events if e['event'] == 'media3_call'}
    returns = {e['operation_id']: e for e in events if e['event'] == 'media3_return'}
    completed = [e for e in events if e['event'] == 'seek_ready_candidate']
    stable = {e['operation_id']: e for e in events if e['event'] == 'first_stable_position'}
    decisions = [e for e in events if e['event'] == 'hard_seek_decision']
    observations = [e for e in events if e['event'] == 'driver_observation']
    dispatches = {e['operation_id']: e for e in events if e['event'] == 'native_dispatch' and 'operation_id' in e}
    offsets = {e['sequence']: dispatches[e['operation_id']]['deadline_server_ms']-e['deadline_local_ms']
               for e in receives if e['operation_id'] in dispatches and e['deadline_local_ms'] > 0}
    raw_callbacks = [e for e in events if e['event'] in ['position_discontinuity', 'playback_state_changed',
                 'playing_completed', 'rate_completed'] and e.get('operation_id') in calls
                 and e['sdk_ms'] >= calls[e['operation_id']]['sdk_ms']]
    callbacks, rejected_callbacks, seen_completions = [], [], set()
    for e in raw_callbacks:
        key = (e['event'], e['operation_id'])
        call = calls[e['operation_id']]
        opposing = 'pause' if e.get('action') == 'play' else 'play'
        obsolete = e['event'] == 'playing_completed' and any(
            c.get('action') == opposing and call['sdk_ms'] < c['sdk_ms'] <= e['sdk_ms'] for c in calls.values())
        duplicate = e['event'] in ['playing_completed', 'rate_completed'] and key in seen_completions
        if obsolete or duplicate:
            rejected_callbacks.append(dict(e, rejection='superseded_by_opposing_command' if obsolete else 'duplicate_completion'))
            continue
        callbacks.append(e)
        seen_completions.add(key)
    first_callbacks = {}
    for e in callbacks:
        first_callbacks.setdefault(e['operation_id'], e)
    metrics = {
        'network_runtime': [e['at_ms']+offsets[e['sequence']]-e['sent_at_server_ms'] for e in events
                            if e['event']=='authoritative_received' and e['kind'] in ['PLAY','PAUSE','SEEK'] and e['sequence'] in offsets],
        'scheduler_lateness': [e['wake_at_ms'] - e['deadline_local_ms'] for e in receives if e['reason'] == 'scheduled' and e['deadline_local_ms'] > 0],
        'effect_queue': [e['rust_received_ms'] - e['enqueued_ms'] for e in receives],
        'poll_lateness': [e['poll_lateness_ms'] for e in observations],
        'jni_roundtrip': [e['jni_roundtrip_ms'] for e in observations],
        'jni_parse_and_registry': [e['jni_to_lock_ms'] for e in observations],
        'source_timestamp_relabel_error': [e['assigned_at_ms'] - e['source_mapped_at_ms'] for e in observations],
        'media3_call': [e['command_latency_ms'] for e in returns.values()],
        'media3_first_callback': [e['sdk_ms']-calls[k]['sdk_ms'] for k,e in first_callbacks.items()],
        'playing_completion': [e['sdk_ms']-calls[e['operation_id']]['sdk_ms'] for e in callbacks
                               if e['event']=='playing_completed' and e.get('action')=='play'],
        'paused_completion': [e['sdk_ms']-calls[e['operation_id']]['sdk_ms'] for e in callbacks
                              if e['event']=='playing_completed' and e.get('action')=='pause'],
        'rate_completion': [e['sdk_ms']-calls[e['operation_id']]['sdk_ms'] for e in callbacks if e['event']=='rate_completed'],
        'seek_ready': [e['latency_ms'] for e in completed],
        'seek_loss': [e['loss_ms'] for e in events if e['event']=='seek_loss_measured'],
        'seek_stable': [e['sdk_ms'] - calls[k]['sdk_ms'] for k, e in stable.items() if k in calls],
        'observation_return': [e['assigned_at_ms']-e['source_mapped_at_ms'] for e in observations],
    }
    hard_seeks = []
    for decision in decisions:
        matching = [e for e in receives if e['action'] == 'seek' and e['reason'] == 'correction' and e['sequence'] == decision['sequence'] and 0 <= e['enqueued_ms'] - decision['at_ms'] <= 10]
        fx = matching[0] if matching else None
        call = calls.get(fx['operation_id']) if fx else None
        previous = [e for e in completed if call and e['sdk_ms'] < call['sdk_ms'] and e['generation'] == fx['generation']]
        last = previous[-1] if previous else None
        first_stable = stable.get(last['operation_id']) if last else None
        classification = 'OTHER'
        evidence = 'Insufficient causal evidence; no automatic legitimate label'
        if decision['sample_age_ms'] > 100:
            classification, evidence = 'STALE_SAMPLE', 'Decision used observation older than 100 ms'
        elif report.get('faults_requested') and call and any(e['event']=='fault_begin' and abs(e['bias_ms'])>=250
                and e['sdk_ms']<=call['sdk_ms']<=e['sdk_ms']+e['duration_ms'] for e in events):
            classification, evidence = 'LEGITIMATE', 'Deliberate >=250 ms observation bias in isolated fault test'
        elif last and last.get('reason')=='snapshot' and call['sdk_ms']-last['sdk_ms']<5000:
            classification, evidence = 'SNAPSHOT_RECOVERY', 'Snapshot reconciliation operation'
        elif last and (not first_stable or first_stable['sdk_ms'] > call['sdk_ms']):
            classification, evidence = 'TRANSIENT_AFTER_SEEK', 'No stable observation after previous seek before decision'
        elif last and decision['drift_ms'] < -250 and abs(abs(decision['drift_ms']) - last['latency_ms']) <= 150:
            classification, evidence = 'PLAYER_COMPLETION_DELAY', 'Lag matches preceding seek stopped playback interval within 150 ms; inferred mechanism'
        hard_seeks.append(dict(decision, reported_position_age_ms=decision['sample_age_ms'], pending_seek=decision['seeking'],
            player_state='READY' if decision['ready'] and not decision['buffering'] else 'BUFFERING_OR_NOT_READY', operation_id=fx['operation_id'] if fx else None,
            classification=classification, classification_evidence=evidence,
            last_seek_operation=last['operation_id'] if last else None,
            last_seek_completion_sdk_ms=last['sdk_ms'] if last else None,
            since_last_seek_completion_ms=call['sdk_ms']-last['sdk_ms'] if last and call else None,
            last_seek_completion_latency_ms=last['latency_ms'] if last else None,
            command_latency_ms=returns.get(fx['operation_id'], {}).get('command_latency_ms') if fx else None))
    resources = report.get('android_resources', [])
    costs = {}
    thread_cpu = {}
    if len(resources) > 1:
        first,last=resources[0],resources[-1]
        seconds=(last['sample_monotonic_ms']-first['sample_monotonic_ms'])/1000
        if seconds > 0:
            costs={k:(last.get(k,0)-first.get(k,0))/1e9/seconds*100 for k in ['driver_cost_ns','jni_cost_ns','source_cost_ns']}
            hz=last['clock_ticks_per_second']
            thread_cpu={k:max(0,ticks-first.get('thread_cpu_groups',{}).get(k,0))/hz/seconds*100
                        for k,ticks in last.get('thread_cpu_groups',{}).items()}
    return {'driver_wall_time_percent':costs,'wall_time_is_not_thread_cpu':True,
            'thread_cpu_percent_one_core_lower_bound':thread_cpu,
            'thread_cpu_limitation':'Exited threads disappear from group counters; lower bound, not exhaustive attribution',
            'rejected_completion_callbacks':rejected_callbacks,
            'schema_version': 1, 'timing_ms': {k:stats(v) for k,v in metrics.items()},
            'hard_seek_classifications': {k:sum(e['classification']==k for e in hard_seeks) for k in ['LEGITIMATE','TRANSIENT_AFTER_SEEK','STALE_SAMPLE','SCHEDULER_DELAY','PLAYER_COMPLETION_DELAY','SNAPSHOT_RECOVERY','OTHER']},
            'hard_seek_decisions': hard_seeks,
            'classification_status': 'INFERRED from MEASURED timestamps; OTHER preserved',
            'clock_relation': 'Rust process Instant and Kotlin elapsedRealtime bracket; no raw cross-origin subtraction',
            'raw_event_counts':dict(collections.Counter(e['event'] for e in events))}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('input', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = analyze(json.loads(args.input.read_text()))
    if args.output:
        args.output.write_text(json.dumps(result, indent=2)+'\n')
    else:
        print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
