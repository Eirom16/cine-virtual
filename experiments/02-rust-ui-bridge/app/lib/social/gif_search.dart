import 'dart:async';

import 'package:flutter/foundation.dart';

import 'gif_provider.dart';

/// Generation invalidates responses immediately, including the debounce window.
class GifSearch extends ChangeNotifier {
  final GifProvider provider;
  GifSearch(this.provider);
  Timer? _debounce;
  int _generation = 0;
  bool _disposed = false, loading = false;
  String query = '', error = '';
  String? next;
  int? latencyMs;
  List<GifDescriptor> items = [];
  void change(String value) {
    _generation++;
    _debounce?.cancel();
    query = value;
    items = [];
    next = null;
    error = '';
    loading = true;
    notifyListeners();
    _debounce = Timer(
      const Duration(milliseconds: 350),
      () => unawaited(load()),
    );
  }

  Future<void> load({bool more = false}) async {
    if (_disposed || (more && (loading || next == null))) return;
    _debounce?.cancel();
    final generation = ++_generation;
    final watch = Stopwatch()..start();
    loading = true;
    error = '';
    if (!more) {
      items = [];
      next = null;
    }
    notifyListeners();
    try {
      final page =
          await (query.trim().isEmpty
                  ? provider.trending(cursor: more ? next : null)
                  : provider.search(query.trim(), cursor: more ? next : null))
              .timeout(
                const Duration(seconds: 8),
                onTimeout: () => throw const GifFailure('timeout'),
              );
      if (_disposed || generation != _generation) return;
      // UI retains at most 100 results; pagination never grows unbounded.
      items = [if (more) ...items, ...page.items].take(100).toList();
      next = items.length < 100 ? page.next : null;
    } on GifFailure catch (e) {
      if (_disposed || generation != _generation) return;
      error = e.code;
    } catch (_) {
      if (_disposed || generation != _generation) return;
      error = 'provider_error';
    } finally {
      if (!_disposed && generation == _generation) {
        loading = false;
        latencyMs = watch.elapsedMilliseconds;
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _generation++;
    _debounce?.cancel();
    super.dispose();
  }
}
