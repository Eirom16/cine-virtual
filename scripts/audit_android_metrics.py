"""Recompute published experiment aggregates without rewriting physical evidence."""
import argparse
import json
from pathlib import Path
from demo_cross_platform import stats, valid


def audit(directory):
    runs = []
    for path in sorted(directory.glob('results-*.json')):
        report = json.loads(path.read_text())
        if not all(k in report for k in ['android_samples', 'linux_samples', 'pair_samples']):
            continue
        comparisons = {}
        for key, samples in [('linux_drift', report['linux_samples']),
                             ('android_drift', [s.get('sync') for s in report['android_samples']])]:
            recomputed = stats([abs(s['drift_ms']) for s in samples if valid(s)])
            published = report[key]
            comparisons[key] = dict(published=published, recomputed_from_raw=recomputed,
                                    count_delta=recomputed['count']-published['count'],
                                    quantiles_and_max_equal=all(recomputed[k] == published[k]
                                        for k in ['p50_ms', 'p95_ms', 'p99_ms', 'max_ms']))
        pairs = stats([s['difference_ms'] for s in report['pair_samples']])
        runs.append(dict(source=path.name, comparisons=comparisons,
                         cross_pair_aggregate_matches=pairs == report['cross_device_difference'],
                         precision_applicable=not report.get('faults_requested', False)))
    return dict(schema_version=1, classification='MEASURED recomputation of saved raw', runs=runs,
                known_race='Reader could append between aggregate calculation and raw copy; original evidence preserved',
                fix='Future runs freeze sample_snapshot before statistics and use it for saved raw',
                all_published_quantiles_and_max_reproduced=all(
                    all(c['quantiles_and_max_equal'] for c in r['comparisons'].values())
                    and r['cross_pair_aggregate_matches'] for r in runs))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = audit(args.directory)
    args.output.write_text(json.dumps(result, indent=2)+'\n')
    print('Published quantiles/max reproduced:', result['all_published_quantiles_and_max_reproduced'])
    if not result['all_published_quantiles_and_max_reproduced']:
        raise SystemExit('Aggregate mismatch: evidence preserved, requires investigation')


if __name__ == '__main__':
    main()
