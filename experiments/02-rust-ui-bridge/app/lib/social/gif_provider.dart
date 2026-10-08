import 'dart:async';
import 'dart:convert';
import 'dart:io';

class GifFailure implements Exception {
  final String code;
  const GifFailure(this.code);
  String get message => switch (code) {
    'blocked' =>
      'Proveedor GIF bloqueado: requiere revisión de términos y cache.',
    'unavailable' => 'Búsqueda GIF no configurada.',
    'offline' => 'Sin conexión con el proveedor GIF.',
    'timeout' => 'El proveedor tardó demasiado. Inténtalo de nuevo.',
    'rate_limited' => 'Límite del proveedor alcanzado. Espera un momento.',
    _ => 'No se pudieron cargar los GIFs. Inténtalo de nuevo.',
  };
  @override
  String toString() => code; // Never include request URLs/credentials.
}

class GifDescriptor {
  final String provider, id, mediaUrl, alt;
  final String? previewUrl;
  final int width, height;
  const GifDescriptor({
    required this.provider,
    required this.id,
    required this.mediaUrl,
    this.previewUrl,
    required this.width,
    required this.height,
    this.alt = '',
  });
  factory GifDescriptor.fromJson(Map<String, dynamic> data) {
    final gif = GifDescriptor(
      provider: data['provider'] as String,
      id: data['provider_content_id'] as String,
      mediaUrl: data['media_url'] as String,
      previewUrl: data['preview_url'] as String?,
      width: data['width'] as int,
      height: data['height'] as int,
      alt: data['alt_text'] as String? ?? '',
    );
    if (!gif.valid) throw const GifFailure('malformed');
    return gif;
  }
  bool get valid =>
      RegExp(r'^[a-zA-Z0-9_-]{1,64}$').hasMatch(id) &&
      width > 0 &&
      width <= 640 &&
      height > 0 &&
      height <= 640 &&
      utf8.encode(alt).length <= 256 &&
      !RegExp(r'[\x00-\x1f\x7f-\x9f]').hasMatch(alt) &&
      _safe(mediaUrl) &&
      (previewUrl == null || _safe(previewUrl!));
  bool _safe(String raw) {
    if (utf8.encode(raw).length > 1024 ||
        RegExp(r'[\x00-\x20\\]').hasMatch(raw)) {
      return false;
    }
    final u = Uri.tryParse(raw);
    if (u == null ||
        u.scheme != 'https' ||
        u.userInfo.isNotEmpty ||
        u.hasPort ||
        u.hasFragment) {
      return false;
    }
    if (provider == 'fixture') {
      return id == 'celebrate' && raw == fixtureGif.mediaUrl;
    }
    return provider == 'giphy' &&
        const [
          'media.giphy.com',
          'media0.giphy.com',
          'media1.giphy.com',
          'media2.giphy.com',
          'media3.giphy.com',
          'media4.giphy.com',
        ].contains(u.host) &&
        u.path.startsWith('/media/$id/') &&
        !u.path.contains('%') &&
        !u.path.contains('..') &&
        (u.path.endsWith('.gif') || u.path.endsWith('.webp')) &&
        u.queryParametersAll.entries.every(
          (e) =>
              const ['cid', 'ep', 'rid', 'ct'].contains(e.key) &&
              e.value.every((v) => v.length <= 128),
        );
  }

  Map<String, dynamic> toJson() => {
    'provider': provider,
    'provider_content_id': id,
    'media_url': mediaUrl,
    'preview_url': previewUrl,
    'width': width,
    'height': height,
    'alt_text': alt,
  };
}

const fixtureGif = GifDescriptor(
  provider: 'fixture',
  id: 'celebrate',
  mediaUrl: 'https://fixtures.cine.invalid/celebrate.gif',
  width: 160,
  height: 100,
  alt: 'Tres círculos de colores celebran (fixture sintético)',
);

class GifPage {
  final List<GifDescriptor> items;
  final String? next;
  const GifPage(this.items, {this.next});
}

abstract class GifProvider {
  String get status;
  String get attribution;
  Future<GifPage> search(String query, {String? cursor});
  Future<GifPage> trending({String? cursor});
  Future<GifDescriptor> resolve(String id);
}

class UnavailableGifProvider implements GifProvider {
  const UnavailableGifProvider();
  @override
  String get status => 'blocked';
  @override
  String get attribution => '';
  @override
  Future<GifPage> search(String query, {String? cursor}) async =>
      throw const GifFailure('blocked');
  @override
  Future<GifPage> trending({String? cursor}) async =>
      throw const GifFailure('blocked');
  @override
  Future<GifDescriptor> resolve(String id) async =>
      throw const GifFailure('blocked');
}

/// Only enabled by an explicit development flag. No uploads or network requests.
class FixtureGifProvider implements GifProvider {
  const FixtureGifProvider();
  @override
  String get status => 'fixture';
  @override
  String get attribution => 'Fixtures sintéticos · sin proveedor remoto';
  @override
  Future<GifPage> search(String query, {String? cursor}) async => GifPage(
    cursor == null &&
            (query.isEmpty ||
                'celebración celebrate fiesta cine'.contains(
                  query.toLowerCase(),
                ))
        ? [fixtureGif]
        : [],
  );
  @override
  Future<GifPage> trending({String? cursor}) => search('', cursor: cursor);
  @override
  Future<GifDescriptor> resolve(String id) async {
    if (id != fixtureGif.id) throw const GifFailure('missing');
    return fixtureGif;
  }
}

typedef GifTransport = Future<Map<String, dynamic>> Function(Uri uri);
Future<Map<String, dynamic>> gifHttp(Uri uri) async {
  final http = HttpClient()..connectionTimeout = const Duration(seconds: 8);
  try {
    return await (() async {
      final request = await http.getUrl(uri);
      request.followRedirects = false;
      final response = await request.close();
      if (response.statusCode == 429) throw const GifFailure('rate_limited');
      if (response.statusCode != 200) throw const GifFailure('provider_error');
      final bytes = <int>[];
      await for (final chunk in response) {
        if (bytes.length + chunk.length > 2 * 1024 * 1024) {
          throw const GifFailure('malformed');
        }
        bytes.addAll(chunk);
      }
      return (jsonDecode(utf8.decode(bytes)) as Map).cast<String, dynamic>();
    })().timeout(const Duration(seconds: 8));
  } on TimeoutException {
    throw const GifFailure('timeout');
  } on SocketException {
    throw const GifFailure('offline');
  } on GifFailure {
    rethrow;
  } catch (_) {
    throw const GifFailure('malformed');
  } finally {
    http.close(force: true);
  }
}

/// Real API adapter, policy-gated and never selected by the application today.
/// Keys are runtime-only; no key exists in source, binaries, diagnostics or room data.
class GiphyGifProvider implements GifProvider {
  final String _key;
  final bool approved;
  final GifTransport transport;
  GiphyGifProvider({
    String apiKey = '',
    this.approved = false,
    this.transport = gifHttp,
  }) : _key = apiKey;
  @override
  String get status => !approved
      ? 'blocked'
      : _key.isEmpty
      ? 'unavailable'
      : 'available';
  @override
  String get attribution => 'Powered By GIPHY';
  void _check() {
    if (!approved) throw const GifFailure('blocked');
    if (_key.isEmpty) throw const GifFailure('unavailable');
  }

  Future<Map<String, dynamic>> _get(
    String path,
    Map<String, String> params,
  ) async {
    _check();
    return transport(
      Uri.https('api.giphy.com', '/v1/gifs/$path', {
        'api_key': _key,
        ...params,
      }),
    ).timeout(
      const Duration(seconds: 8),
      onTimeout: () => throw const GifFailure('timeout'),
    );
  }

  GifDescriptor _parse(dynamic raw) {
    try {
      final row = (raw as Map).cast<String, dynamic>();
      final images = row['images'] as Map;
      final media = images['fixed_width_downsampled'] as Map;
      final size = int.parse('${media['size']}');
      if (size <= 0 || size > 2 * 1024 * 1024 || row['rating'] != 'g') {
        throw const GifFailure('malformed');
      }
      return GifDescriptor.fromJson({
        'provider': 'giphy',
        'provider_content_id': row['id'],
        'media_url': media['url'],
        'preview_url': (images['fixed_width_still'] as Map?)?['url'],
        'width': int.parse('${media['width']}'),
        'height': int.parse('${media['height']}'),
        'alt_text': 'GIF',
      });
    } catch (_) {
      throw const GifFailure('malformed');
    }
  }

  Future<GifPage> _page(String path, String query, String? cursor) async {
    if (query.runes.length > 50) throw const GifFailure('query');
    final maxOffset = path == 'trending' ? 499 : 4999;
    final offset = cursor == null ? 0 : int.tryParse(cursor);
    if (offset == null || offset < 0 || offset > maxOffset) {
      throw const GifFailure('malformed');
    }
    final result = await _get(path, {
      'rating': 'g',
      'limit': '20',
      'offset': '$offset',
      if (query.isNotEmpty) 'q': query,
    });
    try {
      final data = result['data'] as List;
      final pagination = result['pagination'] as Map;
      if (data.length > 20 ||
          pagination['offset'] != offset ||
          pagination['count'] != data.length) {
        throw const GifFailure('malformed');
      }
      final next = offset + data.length;
      return GifPage(
        data.map(_parse).toList(),
        next:
            data.isNotEmpty &&
                next < (pagination['total_count'] as int) &&
                next <= maxOffset
            ? '$next'
            : null,
      );
    } on GifFailure {
      rethrow;
    } catch (_) {
      throw const GifFailure('malformed');
    }
  }

  @override
  Future<GifPage> search(String query, {String? cursor}) => query.trim().isEmpty
      ? trending(cursor: cursor)
      : _page('search', query.trim(), cursor);
  @override
  Future<GifPage> trending({String? cursor}) => _page('trending', '', cursor);
  @override
  Future<GifDescriptor> resolve(String id) async {
    if (!RegExp(r'^[a-zA-Z0-9_-]{1,64}$').hasMatch(id)) {
      throw const GifFailure('malformed');
    }
    final result = await _get(id, {'rating': 'g'});
    final gif = _parse(result['data']);
    if (gif.id != id) throw const GifFailure('malformed');
    return gif;
  }
}
