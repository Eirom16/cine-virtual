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
    metrics = {
        'scheduler_lateness': [e['wake_at_ms'] - e['deadline_local_ms'] for e in receives if e['reason'] == 'scheduled' and e['deadline_local_ms'] > 0],
        'effect_queue': [e['rust_received_ms'] - e['enqueued_ms'] for e in receives],
        'poll_lateness': [e['poll_lateness_ms'] for e in observations],
        'jni_roundtrip': [e['jni_roundtrip_ms'] for e in observations],
        'jni_parse_and_registry': [e['jni_to_lock_ms'] for e in observations],
        'source_timestamp_relabel_error': [e['assigned_at_ms'] - e['source_mapped_at_ms'] for e in observations],
        'media3_call': [e['command_latency_ms'] for e in returns.values()],
        'seek_ready': [e['latency_ms'] for e in completed],
        'seek_stable': [e['sdk_ms'] - calls[k]['sdk_ms'] for k, e in stable.items() if k in calls],
        'observation_return': [e['jni_roundtrip_ms'] for e in observations],
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
        elif fx and fx['reason'] == 'snapshot':
            classification, evidence = 'SNAPSHOT_RECOVERY', 'Snapshot reconciliation operation'
        elif last and (not first_stable or first_stable['sdk_ms'] > call['sdk_ms']):
            classification, evidence = 'TRANSIENT_AFTER_SEEK', 'No stable observation after previous seek before decision'
        elif last and decision['drift_ms'] < -250 and abs(abs(decision['drift_ms']) - last['latency_ms']) <= 150:
            classification, evidence = 'PLAYER_COMPLETION_DELAY', 'Lag matches preceding seek stopped playback interval within 150 ms; inferred mechanism'
        hard_seeks.append(dict(decision, operation_id=fx['operation_id'] if fx else None,
            classification=classification, classification_evidence=evidence,
            last_seek_operation=last['operation_id'] if last else None,
            last_seek_completion_sdk_ms=last['sdk_ms'] if last else None,
            since_last_seek_completion_ms=call['sdk_ms']-last['sdk_ms'] if last and call else None,
            last_seek_completion_latency_ms=last['latency_ms'] if last else None,
            command_latency_ms=returns.get(fx['operation_id'], {}).get('command_latency_ms') if fx else None))
    return {'schema_version': 1, 'timing_ms': {k:stats(v) for k,v in metrics.items()},
            'hard_seek_classifications': dict(collections.Counter(e['classification'] for e in hard_seeks)),
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
