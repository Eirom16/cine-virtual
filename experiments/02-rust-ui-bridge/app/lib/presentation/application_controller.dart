import 'package:flutter/widgets.dart';

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';

import '../bridge.dart';
import 'view_state.dart';

/// Replaceable only at the presentation boundary for widget tests.
abstract class SessionGateway {
  bool get android;
  bool get supportsPlayer;
  Future<void> initialize();
  Map<String, dynamic> call(String type, [Map<String, dynamic>? fields]);
  Future<Map<String, dynamic>?> pick();
  Future<void> prepareAndroid();
  Future<void> dispose();
}

class NativeSessionGateway implements SessionGateway {
  static const native = MethodChannel('cine.mobile/player');
  static const desktop = MethodChannel('cine.desktop/files');
  CineBridge? bridge;
  @override
  bool get android => Platform.isAndroid;
  @override
  bool get supportsPlayer => Platform.isAndroid || Platform.isLinux;
  @override
  Future<void> initialize() async {
    bridge = CineBridge();
    bridge!.call('configure', {
      'playback_rate': true,
      'content_uri_input': android,
    });
    if (android) {
      await native.invokeMethod('bindOwner', bridge!.handle);
      await native.invokeMethod('startNetworkDriver');
    }
  }

  @override
  Map<String, dynamic> call(String type, [Map<String, dynamic>? fields]) =>
      bridge!.call(type, fields);
  @override
  Future<Map<String, dynamic>?> pick() =>
      (android ? native : desktop).invokeMapMethod<String, dynamic>('select');
  @override
  Future<void> prepareAndroid() async {
    final fd = await native.invokeMethod<int>('openHashFd');
    try {
      bridge!.hashFd(fd!);
    } finally {
      await native.invokeMethod('closeHashFd');
    }
    await native.invokeMethod('load', {'disable_audio': false});
  }

  @override
  Future<void> dispose() async {
    await bridge?.disposeAsync();
  }
}

class Invitation {
  final Map<String, dynamic> credentials;
  final String? server;
  const Invitation(this.credentials, this.server);
  factory Invitation.parse(String value) {
    try {
      final root = object(jsonDecode(value.trim()));
      final data = root['invitation'] is Map
          ? object(root['invitation'])
          : root;
      final uuid = RegExp(
        r'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-4[0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}$',
      );
      if (!uuid.hasMatch('${data['room_id']}') ||
          !uuid.hasMatch('${data['room_epoch']}') ||
          data['invite_token'] is! String ||
          (data['invite_token'] as String).isEmpty) {
        throw const FormatException();
      }
      return Invitation({
        for (final key in ['room_id', 'room_epoch', 'invite_token'])
          key: data[key],
      }, root['server'] is String ? root['server'] as String : null);
    } catch (_) {
      throw BridgeFailure('INVALID_INVITE');
    }
  }
}

/// Intents and observation only. Rust remains the only room/network state machine.
class ApplicationController extends ChangeNotifier with WidgetsBindingObserver {
  final SessionGateway gateway;
  final playback = ValueNotifier(const PlaybackView());
  final hashProgress = ValueNotifier<double?>(null);
  final diagnostics = ValueNotifier<Map<String, dynamic>>({});
  RoomView view = const RoomView();
  String endpoint = const String.fromEnvironment('CINE_SERVER');
  String displayName = '', filename = '', error = '', action = '';
  String? invitation;
  bool initialized = false,
      session = false,
      busy = false,
      picking = false,
      recovering = false,
      suspended = false,
      active = true;
  Timer? _timer;
  Map<String, dynamic> _raw = {};
  int _mediaOperation = 0;
  ApplicationController({SessionGateway? gateway})
    : gateway = gateway ?? NativeSessionGateway() {
    if (endpoint.isEmpty &&
        this.gateway.supportsPlayer &&
        !this.gateway.android) {
      endpoint = 'ws://127.0.0.1:8765';
    }
  }
  Future<void> initialize({bool startPolling = true}) async {
    WidgetsBinding.instance.addObserver(this);
    try {
      await gateway.initialize();
      if (!active) return;
      initialized = true;
      poll();
      if (startPolling) {
        _timer = Timer.periodic(
          const Duration(milliseconds: 500),
          (_) => poll(),
        );
      }
    } catch (_) {
      error = 'BRIDGE_UNAVAILABLE';
      _publish();
    }
  }

  void poll() {
    if (!initialized || !active) return;
    try {
      _raw = gateway.call('state');
      // A completed operation owns its error; avoid resurrecting an old error after dismissal.
      _publish();
    } on BridgeFailure catch (e) {
      error = e.code;
      _publish();
    }
  }

  void _publish() {
    if (!active) return;
    final next = RoomView(
      raw: _raw,
      session: session,
      busy: busy,
      picking: picking,
      recovering: recovering,
      filename: filename,
      error: error,
      action: action,
    );
    final changed = next.layoutKey != view.layoutKey;
    view = next;
    playback.value = PlaybackView.from(next);
    hashProgress.value = next.hashProgress;
    // Explicit allowlist: never invitation/resume token, local path, URI or full room descriptor.
    diagnostics.value = {
      'endpoint': Uri.tryParse(endpoint)
          ?.replace(userInfo: '', query: '', fragment: '')
          .toString(),
      'member_id': next.presentation['member_id'],
      'room_id': next.room['room_id'],
      'room_epoch': next.room['room_epoch'],
      'sequence': next.room['sequence'],
      'authority_revision': next.room['authority_revision'],
      'media_revision': next.media['media_revision'],
      'generation': _raw['generation'],
      'clock': object(next.network['state'])['clock'],
      'sync': next.sync,
      'media': next.media,
      'hash': {
        for (final key in [
          'state',
          'status',
          'read_bytes',
          'size_bytes',
          'total_bytes',
          'elapsed_ms',
        ])
          key: next.hash[key],
      },
      'raw_error': error,
      'platform_player': gateway.android
          ? 'Media3 provisional'
          : 'libmpv native window / unsupported platform',
    };
    if (changed) notifyListeners();
  }

  Future<Map<String, dynamic>> _intent(
    String intent, [
    Map<String, dynamic> fields = const {},
  ]) async {
    gateway.call(gateway.android ? 'network' : 'desktop_network', {
      'action': intent,
      ...fields,
    });
    final generation = _raw['generation'];
    final elapsed = Stopwatch()..start();
    while (active) {
      poll();
      final n = view.network;
      if (n['last_action'] == intent && n['busy'] == false) {
        if (n['error'] != null) throw BridgeFailure('${n['error']}');
        return object(n['result']);
      }
      if (generation != null && _raw['generation'] != generation) {
        throw BridgeFailure('OPERATION_CANCELLED');
      }
      // Media hashing has no arbitrary timeout proportional to file size.
      if (intent != 'select' && elapsed.elapsedMilliseconds > 20000) {
        throw BridgeFailure('OPERATION_CANCELLED_OR_TIMEOUT');
      }
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
    throw BridgeFailure('DESTROYED');
  }

  Future<bool> _run(String label, Future<void> Function() work) async {
    if (busy || !active) return false;
    busy = true;
    action = label;
    error = '';
    _publish();
    try {
      await work();
      return true;
    } on BridgeFailure catch (e) {
      error = e.code;
      return false;
    } on PlatformException catch (e) {
      error = e.code;
      return false;
    } catch (_) {
      error = 'NETWORK_OR_PLAYER_ERROR';
      return false;
    } finally {
      busy = false;
      action = '';
      poll();
      _publish();
    }
  }

  Future<bool> enter({
    required bool create,
    required String name,
    required String server,
    String invite = '',
  }) => _run('enter', () async {
    if (!initialized) throw BridgeFailure('BRIDGE_UNAVAILABLE');
    if (!gateway.supportsPlayer) throw BridgeFailure('PLAYER_UNSUPPORTED');
    displayName = name.trim();
    final parsed = create ? null : Invitation.parse(invite);
    endpoint = (parsed?.server ?? server).trim();
    final uri = Uri.tryParse(endpoint);
    if (uri == null ||
        !['ws', 'wss'].contains(uri.scheme) ||
        uri.host.isEmpty) {
      throw BridgeFailure('INVALID_ENDPOINT');
    }
    await _intent('connect', {
      'url': endpoint,
      'name': displayName,
      'allow_lan': true,
    });
    if (create) {
      final data = await _intent('create');
      invitation = jsonEncode({'server': endpoint, 'invitation': data});
    } else {
      await _intent('join', parsed!.credentials);
      invitation = invite.trim();
    }
    session = true;
    filename = '';
    poll();
  });
  Future<void> _hashAndAttach(int operation) async {
    await gateway.prepareAndroid();
    final generation = _raw['generation'];
    Stopwatch? loadWait;
    while (active && operation == _mediaOperation) {
      poll();
      if (generation != _raw['generation']) {
        throw BridgeFailure('OPERATION_CANCELLED');
      }
      final hash = view.hashState;
      final sample = object(_raw['sample']);
      if (hash == 'complete' &&
          sample['loaded'] == true &&
          sample['seeking'] == false) {
        await _intent('attach');
        return;
      }
      if (['modified', 'read_failed', 'cancelled'].contains(hash)) {
        throw BridgeFailure('HASH_FAILED');
      }
      if (hash == 'complete') {
        loadWait ??= Stopwatch()..start();
      }
      if ((loadWait?.elapsedMilliseconds ?? 0) > 20000) {
        throw BridgeFailure('MEDIA_NOT_READY_TIMEOUT');
      }
      await Future<void>.delayed(const Duration(milliseconds: 100));
    }
  }

  Future<bool> selectMedia() => _run('select', () async {
    if (!gateway.supportsPlayer) throw BridgeFailure('PLAYER_UNSUPPORTED');
    picking = true;
    _publish();
    try {
      final chosen = await gateway.pick();
      if (chosen == null) return;
      while (active && (suspended || recovering)) {
        await Future<void>.delayed(const Duration(milliseconds: 50));
      }
      if (!active) return;
      filename = '${chosen['title']}';
      _publish();
      final operation = ++_mediaOperation;
      if (gateway.android) {
        await _hashAndAttach(operation);
      } else {
        await _intent('select', {'path': chosen['path']});
      }
    } finally {
      picking = false;
    }
  });
  Future<bool> ready() => _run('ready', () async {
    await _intent('ready');
  });
  Future<bool> control(String intent, [int? position]) => _run(
    intent,
    () async {
      if (!(intent == 'play' ? view.canPlay : view.canControl)) {
        // _run sets busy; use the projection before that local busy flag for eligibility.
        final eligible = RoomView(
          raw: _raw,
          session: session,
          filename: filename,
        );
        if (!(intent == 'play' ? eligible.canPlay : eligible.canControl)) {
          throw BridgeFailure('NOT_AUTHORIZED');
        }
      }
      await _intent(intent, position == null ? {} : {'position_ms': position});
    },
  );
  Future<bool> reconnect() => _run('reconnect', () async {
    await _intent('reconnect');
  });
  Future<bool> leave() => _run('leave', () async {
    // Leave only resets navigation after the backend acknowledges the operation.
    await _intent(view.connected ? 'leave' : 'disconnect');
    session = false;
    filename = '';
    invitation = null;
    _mediaOperation++;
  });
  Future<bool> disconnectForDiagnostics() => _run('disconnect', () async {
    await _intent('disconnect');
  });
  void dismissError() {
    error = '';
    _publish();
  }

  void cancelHash() {
    if (!gateway.android || view.hashState != 'running') return;
    gateway.call('cancel_hash');
    _mediaOperation++;
    error = '';
    poll();
  }

  Future<void> _recover() async {
    recovering = true;
    _publish();
    try {
      gateway.call('resume');
      final elapsed = Stopwatch()..start();
      while (active) {
        poll();
        if (view.network['last_action'] == 'foreground' &&
            view.network['busy'] == false) {
          if (view.network['error'] != null) {
            throw BridgeFailure('${view.network['error']}');
          }
          break;
        }
        if (elapsed.elapsedMilliseconds > 20000) {
          throw BridgeFailure('NETWORK_TIMEOUT');
        }
        await Future<void>.delayed(const Duration(milliseconds: 50));
      }
      if (!picking && filename.isNotEmpty && gateway.android) {
        await _hashAndAttach(++_mediaOperation);
        await _intent('ready');
      }
    } on BridgeFailure catch (e) {
      error = e.code;
    } catch (_) {
      error = 'NETWORK_OR_PLAYER_ERROR';
    } finally {
      recovering = false;
      poll();
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (!initialized || !session || !gateway.android || !active) return;
    if ((state == AppLifecycleState.paused ||
            state == AppLifecycleState.hidden) &&
        !suspended) {
      suspended = true;
      gateway.call('suspend');
      poll();
    } else if (state == AppLifecycleState.resumed && suspended) {
      suspended = false;
      unawaited(_recover());
    }
  }

  @override
  void dispose() {
    active = false;
    _mediaOperation++;
    _timer?.cancel();
    WidgetsBinding.instance.removeObserver(this);
    unawaited(gateway.dispose());
    playback.dispose();
    hashProgress.dispose();
    diagnostics.dispose();
    super.dispose();
  }
}
