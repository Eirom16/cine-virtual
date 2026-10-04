import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';

typedef _CreateC = Uint64 Function();
typedef _Create = int Function();
typedef _DestroyC = Int32 Function(Uint64);
typedef _Destroy = int Function(int);
typedef _CallC = Int32 Function(
  Uint64,
  Pointer<Uint8>,
  UintPtr,
  Pointer<Uint8>,
  UintPtr,
);
typedef _Call = int Function(int, Pointer<Uint8>, int, Pointer<Uint8>, int);
typedef _HashC = Int32 Function(Uint64, Int32);
typedef _Hash = int Function(int, int);

class BridgeFailure implements Exception {
  final String code;
  BridgeFailure(this.code);
  @override
  String toString() => code;
}

/// Numeric handles and caller-owned buffers; no SDK objects cross FFI.
class CineBridge {
  late final DynamicLibrary library;
  late final _Call _call;
  late final _Destroy _destroy;
  late final _Hash _hash;
  late final int handle;
  late final String openedPath;
  int generation = 1;
  bool destroyed = false;
  final Pointer<Uint8> _output = calloc<Uint8>(65536);
  CineBridge({String? libraryPath}) {
    openedPath =
        libraryPath ??
        (Platform.isAndroid
            ? 'libcine_ui_bridge.so'
            : Platform.environment['CINE_BRIDGE_LIBRARY'] ??
                  'libcine_ui_bridge.so');
    try {
      library = DynamicLibrary.open(openedPath);
      _call = library.lookupFunction<_CallC, _Call>('cine_bridge_call');
      _destroy = library.lookupFunction<_DestroyC, _Destroy>(
        'cine_bridge_destroy',
      );
      _hash = library.lookupFunction<_HashC, _Hash>('cine_bridge_hash_fd');
      handle = library.lookupFunction<_CreateC, _Create>(
        'cine_bridge_create',
      )();
      if (handle == 0) {
        throw BridgeFailure('INSTANCE_LIMIT');
      }
    } catch (_) {
      calloc.free(_output);
      rethrow;
    }
  }
  Map<String, dynamic> call(String type, [Map<String, dynamic>? payload]) {
    if (destroyed) {
      throw BridgeFailure('DESTROYED');
    }
    final command = <String, dynamic>{'type': type};
    if (payload != null) {
      command['payload'] = payload;
    }
    final bytes = utf8.encode(
      jsonEncode({
        'api_version': 1,
        'generation': generation,
        'command': command,
      }),
    );
    if (bytes.length > 65536) {
      throw BridgeFailure('PAYLOAD_TOO_LARGE');
    }
    final input = calloc<Uint8>(bytes.length);
    try {
      input.asTypedList(bytes.length).setAll(0, bytes);
      final n = _call(handle, input, bytes.length, _output, 65536);
      if (n < 0) {
        throw BridgeFailure('ABI_$n');
      }
      final reply = jsonDecode(
        utf8.decode(_output.asTypedList(n)),
      ) as Map<String, dynamic>;
      if (reply['api_version'] != 1) {
        throw BridgeFailure('VERSION_UNSUPPORTED');
      }
      generation = reply['generation'] as int;
      if (reply['ok'] != true) {
        throw BridgeFailure(reply['error']['code'] as String);
      }
      return reply;
    } finally {
      calloc.free(input);
    }
  }

  bool acceptsObservation(int observedGeneration) =>
      !destroyed && observedGeneration == generation;

  void hashFd(int fd) {
    if (destroyed) {
      throw BridgeFailure('DESTROYED');
    }
    final r = _hash(handle, fd);
    if (r != 0) {
      throw BridgeFailure('HASH_ABI_$r');
    }
  }

  Future<void> disposeAsync() async {
    if (destroyed) {
      return;
    }
    destroyed = true;
    calloc.free(_output);
    final id = handle;
    final path = openedPath;
    await Isolate.run(() {
      DynamicLibrary.open(path)
          .lookupFunction<_DestroyC, _Destroy>('cine_bridge_destroy')(id);
    });
  }

  void dispose() {
    if (!destroyed) {
      _destroy(handle);
      destroyed = true;
      calloc.free(_output);
    }
  }
}
