import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'bridge.dart';
import 'room.dart';
import 'product_app.dart';

// Explicit experiment flags retain fixture/autorun tools; normal builds use product UI.
void main() => runApp(
  const bool.fromEnvironment('SPIKE_AUTORUN')
      ? const MaterialApp(home: SpikeScreen())
      : const bool.fromEnvironment('ROOM_AUTORUN')
      ? const MaterialApp(home: RoomScreen())
      : const CineVirtualApp(),
);

class SpikeScreen extends StatefulWidget {
  const SpikeScreen({super.key});
  @override
  State<SpikeScreen> createState() => _SpikeScreenState();
}

class _SpikeScreenState extends State<SpikeScreen> with WidgetsBindingObserver {
  static const native = MethodChannel('cine.mobile/player');
  CineBridge? bridge;
  Timer? timer;
  Map<String, dynamic> view = {};
  String status = 'Starting', filename = 'None';
  bool picking = false, suspended = false, polling = false;
  final bool automatic = const bool.fromEnvironment('SPIKE_AUTORUN');
  int lifecycleResets = 0;
  final Map<String, dynamic> evidence = {};
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    try {
      bridge = CineBridge();
      view = bridge!.call('configure', {
        'playback_rate': Platform.isAndroid,
        'content_uri_input': Platform.isAndroid,
      });
      if (Platform.isAndroid) {
        unawaited(native.invokeMethod('bindOwner', bridge!.handle));
      }
      status = 'Rust connected';
      timer = Timer.periodic(const Duration(milliseconds: 500), (_) => poll());
      if (automatic) {
        WidgetsBinding.instance.addPostFrameCallback((_) async {
          if (Platform.isAndroid) {
            await select();
          } else {
            await benchmark();
          }
        });
      }
    } catch (_) {
      status = 'BRIDGE_UNAVAILABLE';
    }
  }

  Future<void> effects(Map<String, dynamic> reply) async {
    view = reply;
    if (!Platform.isAndroid) {
      return;
    }
    for (final e in reply['effects'] as List) {
      if (e['generation'] != bridge!.generation ||
          suspended && e['action'] == 'play') {
        continue;
      }
      try {
        await native.invokeMethod('effect', e);
      } catch (_) {
        view = bridge!.call('player_error');
        rethrow;
      }
    }
  }

  Future<Map<String, dynamic>> sample() async {
    final sampledGeneration = bridge!.generation;
    final elapsed = Stopwatch()..start();
    final s = await native.invokeMapMethod<String, dynamic>('state') ?? {};
    if (suspended || !bridge!.acceptsObservation(sampledGeneration)) {
      return s;
    }
    if ((view['capabilities'] as Map?)?['playback_rate'] !=
        s['supports_rate']) {
      view = bridge!.call('configure', {
        'playback_rate': s['supports_rate'],
        'content_uri_input': true,
      });
    }
    if (elapsed.elapsedMilliseconds > 100) {
      return s;
    }
    await effects(
      bridge!.call('sample', {
        'position_ms': s['position_ms'],
        'duration_ms': s['duration_ms'],
        'playing': s['playing'],
        'loaded': s['loaded'],
        'buffering': s['buffering'],
        'seeking': s['seeking'],
        'age_ms': elapsed.elapsedMilliseconds,
      }),
    );
    return s;
  }

  Future<void> poll() async {
    if (bridge == null || suspended || polling) {
      return;
    }
    polling = true;
    try {
      if (Platform.isAndroid) {
        await sample();
      } else {
        view = bridge!.call('state');
      }
      if (mounted) {
        setState(() {});
      }
    } catch (_) {
      status = 'PLAYER_SAMPLE_ERROR';
    } finally {
      polling = false;
    }
  }

  Future<void> intent(String command, [Map<String, dynamic>? payload]) async {
    if (Platform.isAndroid && !suspended && command != 'resume') {
      await sample();
    }
    await effects(bridge!.call(command, payload));
    if (mounted) {
      setState(() {});
    }
  }

  Future<void> safeIntent(
    String command, [
    Map<String, dynamic>? payload,
  ]) async {
    try {
      await intent(command, payload);
      status = 'Rust connected';
    } on BridgeFailure catch (e) {
      status = e.code;
    } catch (_) {
      status = 'NATIVE_ERROR';
    }
    if (mounted) {
      setState(() {});
    }
  }

  Future<Map<String, dynamic>> waitLoaded() async {
    final sw = Stopwatch()..start();
    while (sw.elapsedMilliseconds < 15000) {
      final s = await sample();
      if (s['loaded'] == true && s['seeking'] == false) {
        return s;
      }
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }
    throw BridgeFailure('LOAD_TIMEOUT');
  }

  Future<void> waitPlaying() async {
    final first = await sample();
    final elapsed = Stopwatch()..start();
    while (elapsed.elapsedMilliseconds < 5000) {
      final s = await sample();
      if (s['playing'] == true &&
          (s['position_ms'] as int) > (first['position_ms'] as int)) {
        return;
      }
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }
    throw BridgeFailure('PLAY_TIMEOUT');
  }

  Future<void> select() async {
    picking = true;
    try {
      final selected = await native.invokeMapMethod<String, dynamic>('select');
      if (selected == null) {
        return;
      }
      if (suspended || view['suspended'] == true) {
        suspended = false;
        await intent('resume');
      }
      filename = selected['title'] as String;
      final fd = await native.invokeMethod<int>('openHashFd');
      try {
        bridge!.hashFd(fd!);
      } finally {
        await native.invokeMethod('closeHashFd', fd);
      }
      await native.invokeMethod('load', {
        'disable_audio': const bool.fromEnvironment('SPIKE_SILENT'),
      });
      await waitLoaded();
      status = 'Loaded, hashing in Rust worker';
      if (automatic) {
        unawaited(runAndroid());
      }
    } catch (_) {
      status = 'LOCAL_MEDIA_ERROR';
      if (automatic) {
        evidence['failure'] = status;
        finish();
      }
    } finally {
      picking = false;
      if (mounted) {
        setState(() {});
      }
    }
  }

  // Offline recovery fixtures; no server clock negotiation or RoomState exists here.
  Future<void> calibrateFixture({bool reset = true}) async {
    for (var i = 0; i < 3; i++) {
      final n = bridge!.call('state')['now_ms'];
      bridge!.call('clock', {'t1': n, 't2': n, 't3': n, 't4': n});
    }
    if (reset) {
      await effects(bridge!.call('snapshot'));
    }
  }

  Future<void> runAndroid() async {
    timer?.cancel();
    try {
      var s = await waitLoaded();
      evidence['audio'] = const bool.fromEnvironment('SPIKE_SILENT')
          ? 'disabled for emulator video-clock measurement'
          : 'enabled; perceptual quality not assessed';
      evidence['player'] = {
        for (final k in [
          'duration_ms',
          'decoder',
          'rendered_first_frame',
          'load_ms',
        ])
          k: s[k],
      };
      final hs = Stopwatch()..start();
      Map<String, dynamic> hash;
      do {
        hash = bridge!.call('state')['hash'] as Map<String, dynamic>;
        if (hs.elapsedMilliseconds > 30000) {
          throw BridgeFailure('HASH_TIMEOUT');
        }
        await Future<void>.delayed(const Duration(milliseconds: 20));
      } while (hash['state'] == 'running');
      if (hash['state'] != 'complete') {
        throw BridgeFailure('HASH_FAILED');
      }
      evidence['hash'] = {
        for (final k in ['state', 'read_bytes', 'size_bytes', 'elapsed_ms'])
          k: hash[k],
        'access': 'SAF content URI -> duplicated regular FD -> Rust Read',
      };
      evidence['hash']['identity_match'] = await native.invokeMethod(
        'verifyHash',
        hash['digest'],
      );
      evidence['resources'] = await native.invokeMapMethod<String, dynamic>(
        'resources',
      );
      await calibrateFixture();
      await intent('seek', {'position_ms': 5000});
      await waitLoaded();
      await intent('play');
      await waitPlaying();
      final begin = await sample();
      await Future<void>.delayed(const Duration(seconds: 1));
      final end = await sample();
      if ((end['position_ms'] as int) <= (begin['position_ms'] as int)) {
        throw BridgeFailure('PLAY_DID_NOT_ADVANCE');
      }
      await intent('pause');
      await Future<void>.delayed(const Duration(milliseconds: 150));
      final p1 = await sample();
      await Future<void>.delayed(const Duration(milliseconds: 150));
      final p2 = await sample();
      evidence['play_pause'] = {
        'advance_ms':
            (end['position_ms'] as int) - (begin['position_ms'] as int),
        'pause_advance_ms':
            (p2['position_ms'] as int) - (p1['position_ms'] as int),
      };
      evidence['seeks'] = [];
      for (final target in [5000, 10000, 20000]) {
        final sw = Stopwatch()..start();
        await intent('seek', {'position_ms': target});
        s = await waitLoaded();
        (evidence['seeks'] as List).add({
          'target_ms': target,
          'ready_after_dispatch_ms': sw.elapsedMicroseconds / 1000,
          'position_ms': s['position_ms'],
          'landing_error_ms': (s['position_ms'] as int) - target,
        });
      }
      evidence['rate_measurement'] = {
        'warmup_after_first_advance_ms': 500,
        'nominal_window_ms': 5000,
        'clock': 'native monotonic',
      };
      evidence['rates'] = [];
      for (final rate in [0.98, 1.0, 1.02]) {
        await intent('seek', {'position_ms': 5000});
        await waitLoaded();
        await intent('rate', {'rate': rate});
        await intent('play');
        await waitPlaying();
        await Future<void>.delayed(const Duration(milliseconds: 500));
        final b = await sample();
        await Future<void>.delayed(const Duration(seconds: 5));
        s = await sample();
        (evidence['rates'] as List).add({
          'requested': rate,
          'reported': s['rate'],
          'measured_ratio':
              ((s['position_ms'] as int) - (b['position_ms'] as int)) /
              ((s['sample_monotonic_ms'] as int) -
                  (b['sample_monotonic_ms'] as int)),
          'elapsed_native_ms':
              (s['sample_monotonic_ms'] as int) -
              (b['sample_monotonic_ms'] as int),
          'playing': s['playing'],
          'buffering': s['buffering'],
        });
      }
      await intent('rate', {'rate': 1.0});
      evidence['sync_effects'] = [];
      evidence['native_after_effects'] = [];
      for (final drift in [120, 0, 1000]) {
        await calibrateFixture(reset: false);
        for (var i = 0; i < 3; i++) {
          await sample();
          final r = bridge!.call('simulate_drift', {'drift_ms': drift});
          (evidence['sync_effects'] as List).addAll(r['effects'] as List);
          await effects(r);
          if ((r['effects'] as List).isNotEmpty) {
            final measured = await waitLoaded();
            (evidence['native_after_effects'] as List).add({
              'effects': r['effects'],
              'position_ms': measured['position_ms'],
              'reported_rate': measured['rate'],
            });
          }
          await Future<void>.delayed(const Duration(milliseconds: 500));
        }
      }
      final beforeLifecycle = lifecycleResets;
      await native.invokeMethod('requestLifecycleTest');
      final lifecycleWait = Stopwatch()..start();
      while (lifecycleResets - beforeLifecycle < 2 &&
          lifecycleWait.elapsedMilliseconds < 15000) {
        await Future<void>.delayed(const Duration(milliseconds: 100));
      }
      final resumedPlayer = await sample();
      evidence['lifecycle'] = {
        'native_playing_after_resume': resumedPlayer['playing'],
        'native_rate_after_resume': resumedPlayer['rate'],
        'resets': lifecycleResets - beforeLifecycle,
        'clock_trusted': view['clock_trusted'],
        'snapshot_required': view['snapshot_required'],
        'generation': view['generation'],
      };
      if (resumedPlayer['playing'] != false ||
          ((resumedPlayer['rate'] as num) - 1.0).abs() > 0.001 ||
          lifecycleResets - beforeLifecycle < 2 ||
          view['clock_trusted'] != false ||
          view['snapshot_required'] != true) {
        throw BridgeFailure('LIFECYCLE_NOT_RESET');
      }
      evidence['resources_after_playback'] = await native
          .invokeMapMethod<String, dynamic>('resources');
      evidence['native_errors'] = await native.invokeMapMethod<String, dynamic>(
        'errorTests',
      );
      final nativeErrors = evidence['native_errors'] as Map;
      if (evidence['hash']['identity_match'] != true ||
          (evidence['play_pause']['pause_advance_ms'] as int).abs() > 50 ||
          nativeErrors.values.any((v) => v != true)) {
        throw BridgeFailure('NATIVE_VALIDATION_FAILED');
      }
      final syncEffects = evidence['sync_effects'] as List;
      if (!syncEffects.any((e) => e['action'] == 'rate' && e['value'] != 1.0) ||
          !syncEffects.any((e) => e['action'] == 'rate' && e['value'] == 1.0) ||
          !syncEffects.any((e) => e['action'] == 'seek')) {
        throw BridgeFailure('SYNC_EFFECTS_MISSING');
      }
      for (final r in evidence['rates'] as List) {
        if (((r['reported'] as num) - (r['requested'] as num)).abs() > 0.001 ||
            ((r['measured_ratio'] as num) - (r['requested'] as num)).abs() >
                0.15) {
          throw BridgeFailure('RATE_NOT_APPLIED');
        }
      }
      evidence['player']['rendered_first_frame'] =
          (await sample())['rendered_first_frame'];
      if (evidence['player']['rendered_first_frame'] != true) {
        throw BridgeFailure('FRAME_NOT_RENDERED');
      }
      await benchmark();
    } catch (e) {
      evidence['failure'] = e is BridgeFailure
          ? e.code
          : 'ANDROID_SCENARIO_ERROR';
      finish();
    }
  }

  Future<void> benchmark() async {
    timer?.cancel();
    final latencies = <double>[];
    for (var i = 0; i < 1000; i++) {
      final s = Stopwatch()..start();
      bridge!.call('state');
      latencies.add(s.elapsedMicroseconds / 1000);
    }
    latencies.sort();
    evidence['command_roundtrip_ms'] = {
      'p50': latencies[499],
      'p95': latencies[949],
      'max': latencies.last,
      'count': 1000,
    };
    evidence['updates'] = [];
    for (final hz in [2, 10]) {
      final total = Stopwatch()..start();
      final times = <double>[];
      for (var i = 0; i < hz * 2; i++) {
        final s = Stopwatch()..start();
        if (Platform.isAndroid && !suspended) {
          await sample();
        } else {
          view = bridge!.call('state');
        }
        times.add(s.elapsedMicroseconds / 1000);
        if (mounted) {
          setState(() {});
        }
        final due = (i + 1) * 1000 ~/ hz - total.elapsedMilliseconds;
        if (due > 0) {
          await Future<void>.delayed(Duration(milliseconds: due));
        }
      }
      times.sort();
      (evidence['updates'] as List).add({
        'hz': hz,
        'count': times.length,
        'elapsed_ms': total.elapsedMilliseconds,
        'roundtrip_p95_ms': times[(times.length * 0.95).ceil() - 1],
      });
    }
    try {
      bridge!.call('not_a_command');
    } on BridgeFailure catch (e) {
      evidence['bridge_error_test'] = e.code == 'INVALID_DTO';
    }
    if (evidence['bridge_error_test'] != true) {
      evidence['failure'] = 'BRIDGE_VALIDATION_FAILED';
    } else {
      evidence['passed'] = true;
    }
    finish();
  }

  void finish() {
    evidence['platform'] = Platform.isAndroid ? 'android' : 'linux';
    evidence['api_version'] = 1;
    evidence['timestamp'] = DateTime.now().toUtc().toIso8601String();
    final encoded = base64Encode(utf8.encode(jsonEncode(evidence)));
    final count = (encoded.length / 768).ceil();
    for (var i = 0; i < count; i++) {
      final end = (i + 1) * 768;
      final chunk = encoded.substring(
        i * 768,
        end < encoded.length ? end : encoded.length,
      );
      debugPrint('CINE_SPIKE_RESULT_PART $i/$count $chunk');
    }
    status = evidence['passed'] == true ? 'SPIKE PASS' : 'SPIKE FAILED';
    if (mounted) {
      setState(() {});
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (bridge == null) {
      return;
    }
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.hidden) {
      if (!suspended) {
        suspended = true;
        lifecycleResets++;
        unawaited(safeIntent('suspend'));
      }
    } else if (state == AppLifecycleState.resumed && suspended) {
      suspended = false;
      lifecycleResets++;
      unawaited(safeIntent('resume'));
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    timer?.cancel();
    final b = bridge;
    if (b != null) {
      unawaited(b.disposeAsync());
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final s = view['sample'] as Map<String, dynamic>? ?? {};
    return Scaffold(
      appBar: AppBar(title: const Text('Cine Mobile Spike')),
      body: SingleChildScrollView(
        child: Column(
          children: [
            if (Platform.isAndroid)
              const SizedBox(
                height: 200,
                child: AndroidView(viewType: 'cine.mobile/surface'),
              ),
            Text('Rust: $status'),
            Text('File: $filename'),
            Text('Position: ${s['position_ms'] ?? 0} ms'),
            Text('Duration: ${s['duration_ms'] ?? 0} ms'),
            Text('Playing: ${s['playing'] ?? false}'),
            Text('Hash: ${(view['hash'] as Map?)?['state'] ?? 'none'}'),
            Text('Generation: ${view['generation'] ?? 0}'),
            if (Platform.isAndroid)
              ElevatedButton(
                onPressed: select,
                child: const Text('Select video'),
              ),
            Wrap(
              children: [
                ElevatedButton(
                  onPressed: () => safeIntent('play'),
                  child: const Text('Play'),
                ),
                ElevatedButton(
                  onPressed: () => safeIntent('pause'),
                  child: const Text('Pause'),
                ),
                ElevatedButton(
                  onPressed: () => safeIntent('seek', {
                    'position_ms': ((s['position_ms'] ?? 0) as int) + 5000,
                  }),
                  child: const Text('Seek +5s'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
