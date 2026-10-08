import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'bridge.dart';

/// Engineering presentation only. Protocol, authority, clock, readiness and
/// timeline decisions remain in the shared Rust client.
class RoomScreen extends StatefulWidget {
  const RoomScreen({super.key});
  @override
  State<RoomScreen> createState() => _RoomScreenState();
}

class _RoomScreenState extends State<RoomScreen> with WidgetsBindingObserver {
  static const native = MethodChannel('cine.mobile/player');
  final server = TextEditingController(text: 'ws://');
  final name = TextEditingController(text: 'Android');
  final invite = TextEditingController();
  late CineBridge bridge;
  Map<String, dynamic> view = {};
  String error = '', title = 'None';
  bool suspended = false, recovering = false, active = true, picking = false;
  Timer? timer;
  int polls = 0;
  Map<String, dynamic> get network =>
      (view['network'] as Map?)?.cast<String, dynamic>() ?? {};
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    bridge = CineBridge();
    view = bridge.call('configure', {
      'playback_rate': true,
      'content_uri_input': true,
    });
    unawaited(
      native
          .invokeMethod('bindOwner', bridge.handle)
          .then((_) => native.invokeMethod('startNetworkDriver')),
    );
    timer = Timer.periodic(const Duration(milliseconds: 500), (_) => poll());
    if (const bool.fromEnvironment('ROOM_AUTORUN')) {
      WidgetsBinding.instance.addPostFrameCallback((_) => automatic());
    }
  }

  void poll() {
    if (!active || bridge.destroyed) return;
    try {
      view = bridge.call('state');
      final n = network;
      if (n['error'] != null) error = n['error'] as String;
      if (const bool.fromEnvironment('ROOM_AUTORUN') && !suspended) {
        // Whitelisted local telemetry; no invitation, digest, URI or payload.
        debugPrint(
          'CINE_ROOM_SAMPLE ${jsonEncode({
            'sync': n['sync'],
            'clock': (n['state'] as Map?)?['clock'],
            'sequence': (n['state'] as Map?)?['sequence'],
            'media': n['media'],
            'busy': n['busy'],
            'last_action': n['last_action'],
            'error': n['error'],
            'hash': {
              for (final k in ['state', 'read_bytes', 'size_bytes', 'elapsed_ms']) k: (view['hash'] as Map?)?[k],
            },
          })}',
        );
      }
      if (const bool.fromEnvironment('ROOM_AUTORUN') && ++polls % 10 == 0) {
        unawaited(
          native.invokeMapMethod<String, dynamic>('resources').then((r) {
            if (active) debugPrint('CINE_ROOM_RESOURCE ${jsonEncode(r)}');
          }),
        );
      }
      if (mounted) setState(() {});
    } on BridgeFailure catch (e) {
      error = e.code;
    }
  }

  Future<Map<String, dynamic>> intent(
    String action, [
    Map<String, dynamic> fields = const {},
  ]) async {
    bridge.call('network', {'action': action, ...fields});
    final generation = bridge.generation;
    final deadline = Stopwatch()..start();
    while (active &&
        bridge.acceptsObservation(generation) &&
        deadline.elapsedMilliseconds < 15000) {
      view = bridge.call('state');
      final n = network;
      if (n['last_action'] == action && n['busy'] == false) {
        if (n['error'] != null) throw BridgeFailure(n['error'] as String);
        if (mounted) setState(() {});
        return (n['result'] as Map?)?.cast<String, dynamic>() ?? {};
      }
      await Future<void>.delayed(const Duration(milliseconds: 25));
    }
    throw BridgeFailure('OPERATION_CANCELLED_OR_TIMEOUT');
  }

  Future<void> safe(Future<void> Function() operation) async {
    try {
      await operation();
      error = '';
    } on BridgeFailure catch (e) {
      error = e.code;
    } catch (_) {
      error = 'NATIVE_OR_NETWORK_ERROR';
    }
    if (mounted) setState(() {});
  }

  Future<void> connect() async {
    await intent('connect', {
      'url': server.text,
      'name': name.text,
      'allow_lan': true,
    });
  }

  Future<void> join() async {
    await intent(
      'join',
      (jsonDecode(invite.text) as Map).cast<String, dynamic>(),
    );
  }

  Future<void> create() async {
    invite.text = jsonEncode(await intent('create'));
  }

  Future<void> hashAndAttach({bool revalidate = false}) async {
    final fd = await native.invokeMethod<int>('openHashFd');
    try {
      bridge.hashFd(fd!);
    } finally {
      await native.invokeMethod('closeHashFd');
    }
    await native.invokeMethod('load', {'disable_audio': false});
    final sw = Stopwatch()..start();
    final generation = bridge.generation;
    while (active &&
        bridge.acceptsObservation(generation) &&
        sw.elapsedMilliseconds < 20000) {
      view = bridge.call('state');
      final hash = view['hash'] as Map;
      final sample = view['sample'] as Map;
      if (hash['state'] == 'complete' &&
          sample['loaded'] == true &&
          sample['seeking'] == false) {
        await intent(revalidate ? 'revalidate' : 'attach');
        return;
      }
      if (['modified', 'read_failed', 'cancelled'].contains(hash['state'])) {
        throw BridgeFailure('HASH_FAILED');
      }
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
    throw BridgeFailure('MEDIA_NOT_READY');
  }

  Future<void> select() async {
    picking = true;
    try {
      final chosen = await native.invokeMapMethod<String, dynamic>('select');
      if (chosen == null) return;
      final foregroundWait = Stopwatch()..start();
      while (suspended ||
          recovering ||
          bridge.call('state')['suspended'] == true) {
        if (foregroundWait.elapsedMilliseconds > 15000) {
          throw BridgeFailure('FOREGROUND_TIMEOUT');
        }
        await Future<void>.delayed(const Duration(milliseconds: 25));
      }
      title = chosen['title'] as String;
      await hashAndAttach();
    } finally {
      picking = false;
    }
  }

  Future<void> automatic() async {
    await safe(() async {
      final config =
          await native.invokeMapMethod<String, dynamic>('roomConfig') ?? {};
      debugPrint('CINE_ROOM_BEGIN ${config['run_id']}');
      server.text = config['server'] as String;
      invite.text = config['invite'] as String;
      await connect();
      await join();
      await select();
      await intent('ready');
      debugPrint('CINE_ROOM_READY');
    });
    if (error.isNotEmpty) debugPrint('CINE_ROOM_FAILURE $error');
  }

  Future<void> recover() async {
    recovering = true;
    try {
      view = bridge.call('resume');
      final sw = Stopwatch()..start();
      var recovered = false;
      while (active && sw.elapsedMilliseconds < 15000) {
        view = bridge.call('state');
        if (network['last_action'] == 'foreground' &&
            network['busy'] == false) {
          if (network['error'] != null) {
            throw BridgeFailure(network['error'] as String);
          }
          recovered = true;
          break;
        }
        await Future<void>.delayed(const Duration(milliseconds: 25));
      }
      if (!recovered) throw BridgeFailure('FOREGROUND_TIMEOUT');
      if (!picking && title != 'None') {
        await hashAndAttach(revalidate: true);
        await intent('ready');
      }
      debugPrint(
        'CINE_ROOM_RECOVERED ${jsonEncode({'clock_trusted': view['clock_trusted'], 'snapshot_required': view['snapshot_required']})}',
      );
    } finally {
      recovering = false;
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState s) {
    if (!active) return;
    if ((s == AppLifecycleState.hidden || s == AppLifecycleState.paused) &&
        !suspended) {
      suspended = true;
      view = bridge.call('suspend');
      debugPrint('CINE_ROOM_SUSPENDED');
    } else if (s == AppLifecycleState.resumed && suspended) {
      suspended = false;
      unawaited(safe(recover));
    }
  }

  @override
  void dispose() {
    active = false;
    timer?.cancel();
    WidgetsBinding.instance.removeObserver(this);
    unawaited(bridge.disposeAsync());
    server.dispose();
    name.dispose();
    invite.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final sync = (network['sync'] as Map?) ?? {};
    final hash = (view['hash'] as Map?) ?? {};
    return Scaffold(
      appBar: AppBar(title: const Text('Cine Cross-platform Engineering')),
      body: SingleChildScrollView(
        child: Column(
          children: [
            const SizedBox(
              height: 160,
              child: AndroidView(viewType: 'cine.mobile/surface'),
            ),
            TextField(
              controller: server,
              decoration: const InputDecoration(
                labelText: 'Server ws:// (controlled LAN only)',
              ),
            ),
            TextField(
              controller: name,
              decoration: const InputDecoration(labelText: 'Name'),
            ),
            TextField(
              controller: invite,
              decoration: const InputDecoration(
                labelText: 'Private invitation JSON',
              ),
              obscureText: true,
            ),
            Text('Error: $error'),
            Text('File: $title'),
            Text(
              'Hash: ${hash['state']} ${hash['read_bytes'] ?? 0}/${hash['size_bytes'] ?? 0}',
            ),
            Text(
              'Connected: ${network['connected']} Clock: ${view['clock_trusted']}',
            ),
            Text(
              'Position: ${sync['position_ms']} Drift: ${sync['drift_ms']} ms',
            ),
            Text('Room: ${jsonEncode(network['state'])}'),
            Wrap(
              children: [
                ElevatedButton(
                  onPressed: () => safe(connect),
                  child: const Text('Connect'),
                ),
                ElevatedButton(
                  onPressed: () => safe(create),
                  child: const Text('Create'),
                ),
                ElevatedButton(
                  onPressed: () => safe(join),
                  child: const Text('Join'),
                ),
                ElevatedButton(
                  onPressed: () => safe(select),
                  child: const Text('Select file'),
                ),
                for (final action in [
                  'ready',
                  'play',
                  'pause',
                  'disconnect',
                  'reconnect',
                  'leave',
                ])
                  ElevatedButton(
                    onPressed: () => safe(() async {
                      await intent(action);
                    }),
                    child: Text(action),
                  ),
                ElevatedButton(
                  onPressed: () => safe(() async {
                    await intent('seek', {
                      'position_ms':
                          ((sync['position_ms'] ?? 0) as num).toInt() + 10000,
                    });
                  }),
                  child: const Text('Seek +10s'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
