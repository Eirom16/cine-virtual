import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:cine_mobile_spike/main.dart';
import 'package:cine_mobile_spike/bridge.dart';
import 'package:cine_mobile_spike/unsupported_player.dart';

void main() {
  test('boundary validates errors and explicit destroy', () {
    final b = CineBridge();
    expect(b.call('state')['api_version'], 1);
    expect(() => b.call('invalid'), throwsA(isA<BridgeFailure>()));
    b.dispose();
    b.dispose();
    expect(() => b.call('state'), throwsA(isA<BridgeFailure>()));
  });
  test('suspend and resume invalidate observation generations', () {
    final b = CineBridge();
    final before = b.generation;
    final suspended = b.call('suspend');
    expect(b.acceptsObservation(before), false);
    expect(b.acceptsObservation(b.generation), true);
    expect(suspended['clock_trusted'], false);
    expect(suspended['snapshot_required'], true);
    final resumed = b.call('resume');
    expect(resumed['generation'], 3);
    expect(resumed['sample']['loaded'], false);
    b.dispose();
  });
  test('async destroy keeps worker join off the UI isolate', () async {
    final b = CineBridge();
    await b.disposeAsync();
    expect(() => b.call('state'), throwsA(isA<BridgeFailure>()));
  });
  test(
    'network intents validate UUIDs and forbid offline truth in room mode',
    () async {
      final b = CineBridge();
      expect(
        () => b.call('network', {
          'action': 'join',
          'room_id': 'bad',
          'room_epoch': 'bad',
          'invite_token': 'test',
        }),
        throwsA(isA<BridgeFailure>()),
      );
      expect(
        () => b.call('network', {'action': 'ready'}),
        throwsA(isA<BridgeFailure>()),
      );
      expect(() => b.call('snapshot'), throwsA(isA<BridgeFailure>()));
      final before = b.generation;
      b.call('suspend');
      expect(b.acceptsObservation(before), false);
      await b.disposeAsync();
      expect(
        () => b.call('network', {'action': 'ready'}),
        throwsA(isA<BridgeFailure>()),
      );
    },
  );
  testWidgets('engineering screen presents Rust state without room policy', (
    tester,
  ) async {
    await tester.pumpWidget(const MaterialApp(home: SpikeScreen()));
    expect(find.text('Cine Mobile Spike'), findsOneWidget);
    expect(find.text('Rust: Rust connected'), findsOneWidget);
    expect(find.text('Play'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });
  testWidgets('compile-only iOS screen cannot advertise playback', (
    tester,
  ) async {
    await tester.pumpWidget(const MaterialApp(home: UnsupportedPlayerScreen()));
    expect(find.text('IOS PLAYER RUNTIME NOT IMPLEMENTED'), findsOneWidget);
    expect(find.text('Play'), findsNothing);
    expect(find.text('Pause'), findsNothing);
    expect(find.text('Seek'), findsNothing);
    await tester.pumpWidget(const SizedBox());
  });
}
