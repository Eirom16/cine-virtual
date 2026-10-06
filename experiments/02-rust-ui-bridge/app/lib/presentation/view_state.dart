import 'dart:convert';

Map<String, dynamic> object(dynamic value) =>
    value is Map ? value.cast<String, dynamic>() : {};
int number(dynamic value) => value is num ? value.toInt() : 0;

enum ConnectionStatus {
  disconnected,
  connecting,
  connected,
  reconnecting,
  recovering,
}

enum Readiness {
  noMedia,
  hashing,
  loading,
  mismatch,
  clock,
  verifying,
  ready,
  waiting,
  error,
}

class MemberView {
  final String id, name, status;
  final bool host, ready, connected, you;
  const MemberView({
    required this.id,
    required this.name,
    required this.status,
    required this.host,
    required this.ready,
    required this.connected,
    required this.you,
  });
}

/// Only derives presentation from the Rust Application snapshot.
class RoomView {
  final Map<String, dynamic> raw;
  final bool session, busy, picking, recovering;
  final String filename, error, action;
  const RoomView({
    this.raw = const {},
    this.session = false,
    this.busy = false,
    this.picking = false,
    this.recovering = false,
    this.filename = '',
    this.error = '',
    this.action = '',
  });
  Map<String, dynamic> get network => object(raw['network']);
  Map<String, dynamic> get sync => object(network['sync']);
  Map<String, dynamic> get presentation => object(network['presentation']);
  Map<String, dynamic> get room => session ? object(presentation['room']) : {};
  Map<String, dynamic> get media => object(network['media']);
  Map<String, dynamic> get descriptor =>
      object(object(room['media'])['descriptor']);
  Map<String, dynamic> get hash =>
      network['hash'] is Map ? object(network['hash']) : object(raw['hash']);
  String get hashState => '${hash['state'] ?? hash['status'] ?? ''}';
  bool get connected => network['connected'] == true;
  bool get isHost =>
      room['host_id'] != null && room['host_id'] == presentation['member_id'];
  bool get hasMedia => room['media'] != null;
  bool get pending => object(room['playback'])['pending'] != null;
  bool get trusted => sync['clock_trusted'] == true;
  bool get roomReady => sync['room_ready'] == true;
  bool get canSelect =>
      connected && !busy && (!isHost || (!pending && sync['playing'] != true));
  bool get canReady =>
      connected &&
      !busy &&
      hasMedia &&
      media['identity_match'] == true &&
      sync['ready'] == true &&
      sync['buffering'] != true &&
      sync['seeking'] != true &&
      trusted &&
      !roomReady;
  bool get canControl =>
      isHost &&
      connected &&
      hasMedia &&
      trusted &&
      sync['snapshot_required'] == false &&
      !pending &&
      !busy;
  bool get allReady =>
      members.where((m) => m.connected).isNotEmpty &&
      members.where((m) => m.connected).every((m) => m.ready);
  bool get canPlay => canControl && roomReady && allReady;
  ConnectionStatus get connection => recovering
      ? ConnectionStatus.recovering
      : busy && action == 'reconnect'
      ? ConnectionStatus.reconnecting
      : busy && action == 'enter'
      ? ConnectionStatus.connecting
      : connected
      ? ConnectionStatus.connected
      : ConnectionStatus.disconnected;
  List<MemberView> get members => (room['members'] as List? ?? []).map((value) {
    final m = object(value);
    return MemberView(
      id: '${m['member_id']}',
      name: '${m['display_name'] ?? 'Invitado'}',
      status: '${m['status'] ?? 'idle'}',
      host: m['member_id'] == room['host_id'],
      ready: m['ready'] == true,
      connected: m['connected'] == true,
      you: m['member_id'] == presentation['member_id'],
    );
  }).toList();
  Readiness get readiness {
    if (media['identity_match'] == false || error == 'MEDIA_MISMATCH') {
      return Readiness.mismatch;
    }
    if (hashState == 'running' || hashState == 'hashing') {
      return Readiness.hashing;
    }
    if (error.isNotEmpty &&
        (error.contains('HASH') ||
            error.contains('PLAYER') ||
            error.contains('PERMISSION'))) {
      return Readiness.error;
    }
    if (!hasMedia && filename.isEmpty) return Readiness.noMedia;
    if (picking || (filename.isNotEmpty && sync['ready'] != true)) {
      return Readiness.loading;
    }
    if (filename.isEmpty || media['identity_match'] != true) {
      return Readiness.noMedia;
    }
    if (!trusted) return Readiness.clock;
    if (busy && action == 'ready') return Readiness.verifying;
    if (roomReady) return Readiness.ready;
    return Readiness.waiting;
  }

  double? get hashProgress {
    final total = number(hash['size_bytes'] ?? hash['total_bytes']);
    return total > 0
        ? (number(hash['read_bytes']) / total).clamp(0.0, 1.0)
        : null;
  }

  /// High-frequency measurements deliberately excluded from the lobby key.
  String get layoutKey => jsonEncode({
    'session': session,
    'room': room,
    'busy': busy,
    'action': action,
    'error': error,
    'filename': filename,
    'picking': picking,
    'connection': connection.name,
    'readiness': readiness.name,
    'canControl': canControl,
    'playing': sync['playing'],
    'canReady': canReady,
    'canPlay': canPlay,
    'canSelect': canSelect,
    'media': media,
  });
}

class PlaybackView {
  final int position, duration;
  final bool playing, buffering;
  const PlaybackView({
    this.position = 0,
    this.duration = 0,
    this.playing = false,
    this.buffering = false,
  });
  factory PlaybackView.from(RoomView view) => PlaybackView(
    position: number(view.sync['position_ms']),
    duration: number(
      view.media['duration_ms'] ?? view.descriptor['duration_ms'],
    ),
    playing: view.sync['playing'] == true,
    buffering: view.sync['buffering'] == true,
  );
  @override
  bool operator ==(Object other) =>
      other is PlaybackView &&
      position == other.position &&
      duration == other.duration &&
      playing == other.playing &&
      buffering == other.buffering;
  @override
  int get hashCode => Object.hash(position, duration, playing, buffering);
}

String timeLabel(int milliseconds) {
  final seconds = milliseconds ~/ 1000;
  final hours = seconds ~/ 3600;
  final minutes = (seconds ~/ 60) % 60;
  final remainder = (seconds % 60).toString().padLeft(2, '0');
  return hours > 0
      ? '$hours:${minutes.toString().padLeft(2, '0')}:$remainder'
      : '$minutes:$remainder';
}
