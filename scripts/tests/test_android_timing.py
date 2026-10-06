"""Timing evidence must preserve independent clock domains and unknown causes."""
import sys
from pathlib import Path
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from analyze_android_timing import analyze


class AndroidTimingTests(unittest.TestCase):
    def test_calibrated_offset_not_kotlin_absolute_ticks_measures_network_delay(self):
        events = [
            dict(event='authoritative_received', kind='PLAY', sequence=3, at_ms=4800, sent_at_server_ms=99799),
            dict(event='effect_received', operation_id=1, action='play', reason='scheduled', sequence=3,
                 wake_at_ms=6002, deadline_local_ms=6000, enqueued_ms=6002, rust_received_ms=6014, sdk_ms=5000000),
            dict(event='native_dispatch', operation_id=1, deadline_server_ms=101000),
        ]
        timing = analyze({'android_events':events})['timing_ms']
        self.assertEqual(timing['network_runtime']['p50_ms'], 1)
        self.assertEqual(timing['scheduler_lateness']['p50_ms'], 2)
        self.assertEqual(timing['effect_queue']['p50_ms'], 12)

    def test_missing_correlation_remains_other_and_stale_decision_is_visible(self):
        decision = dict(event='hard_seek_decision', sequence=3, at_ms=2000, sample_at_ms=1990,
                        sample_age_ms=10, drift_ms=-700, ready=True, buffering=False, seeking=False)
        report = analyze({'android_events':[decision]})
        self.assertEqual(report['hard_seek_classifications']['OTHER'], 1)
        self.assertIsNone(report['hard_seek_decisions'][0]['operation_id'])
        report = analyze({'android_events':[dict(decision, sample_age_ms=150)]})
        self.assertEqual(report['hard_seek_classifications']['STALE_SAMPLE'], 1)

    def test_opposing_command_invalidates_old_completion_without_erasing_evidence(self):
        events = [
            dict(event='media3_call', operation_id=1, action='pause', sdk_ms=100),
            dict(event='media3_call', operation_id=2, action='play', sdk_ms=200),
            dict(event='playing_completed', operation_id=1, action='pause', sdk_ms=900),
            dict(event='playing_completed', operation_id=2, action='play', sdk_ms=210),
        ]
        report = analyze({'android_events':events})
        self.assertEqual(report['timing_ms']['paused_completion']['count'], 0)
        self.assertEqual(report['timing_ms']['playing_completion']['p50_ms'], 10)
        self.assertEqual(report['rejected_completion_callbacks'][0]['operation_id'], 1)


if __name__ == '__main__':
    unittest.main()
