
import 'package:flutter/painting.dart';
import 'package:flutter/services.dart';

import 'gif_provider.dart';

/// Session-only LRU of approved media bytes; no disk, queries or credentials.
class GifMediaCache {
  static const maxBytes = 8 * 1024 * 1024,
      maxEntries = 32,
      maxAssetBytes = 2 * 1024 * 1024;
  final _bytes = <String, Uint8List>{};
  int byteCount = 0;
  bool _disposed = false;
  int _generation = 0;
  int get entries => _bytes.length;
  void configureImages() {
    PaintingBinding.instance.imageCache.maximumSize = 64;
    PaintingBinding.instance.imageCache.maximumSizeBytes = 24 * 1024 * 1024;
  }

  Future<Uint8List> load(GifDescriptor gif) async {
    if (_disposed || !gif.valid) throw const GifFailure('malformed');
    // GIPHY media/cache blocked by policy, including received descriptors.
    if (gif.provider != 'fixture') throw const GifFailure('blocked');
    final key = '${gif.provider}:${gif.id}';
    final cached = _bytes.remove(key);
    if (cached != null) {
      _bytes[key] = cached;
      return cached;
    }
    final generation = _generation;
    final asset = await rootBundle.load('assets/social/celebrate.gif');
    final bytes = asset.buffer.asUint8List(
      asset.offsetInBytes,
      asset.lengthInBytes,
    );
    if (bytes.length > maxAssetBytes) throw const GifFailure('oversized');
    if (!_disposed && generation == _generation) put(key, bytes);
    return bytes;
  }

  void put(String key, Uint8List bytes) {
    if (_disposed || bytes.length > maxAssetBytes) return;
    final old = _bytes.remove(key);
    byteCount -= old?.length ?? 0;
    while (_bytes.isNotEmpty &&
        (_bytes.length >= maxEntries || byteCount + bytes.length > maxBytes)) {
      byteCount -= _bytes.remove(_bytes.keys.first)!.length;
    }
    _bytes[key] = bytes;
    byteCount += bytes.length;
  }

  void clear() {
    _generation++;
    _bytes.clear();
    byteCount = 0;
    PaintingBinding.instance.imageCache.clear();
  }

  void dispose() {
    _disposed = true;
    clear();
  }
}
