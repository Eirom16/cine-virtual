import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:cine_mobile_spike/social/gif_provider.dart';
import 'package:cine_mobile_spike/social/gif_search.dart';
import 'package:cine_mobile_spike/social/gif_cache.dart';

Map<String, dynamic> row(String id) => {
  'id': id,
  'rating': 'g',
  'images': {
    'fixed_width_downsampled': {
      'url': 'https://media.giphy.com/media/$id/200w_d.gif',
      'width': '160',
      'height': '100',
      'size': '1024',
    },
    'fixed_width_still': {
      'url': 'https://media.giphy.com/media/$id/200w_s.gif',
    },
  },
};
Map<String, dynamic> page(List data, {int offset = 0, int total = 2}) => {
  'data': data,
  'pagination': {'offset': offset, 'count': data.length, 'total_count': total},
};

class ControlledProvider implements GifProvider {
  final requests = <Completer<GifPage>>[];
  @override
  String get status => 'available';
  @override
  String get attribution => 'Test fixture';
  @override
  Future<GifPage> search(String query, {String? cursor}) {
    final c = Completer<GifPage>();
    requests.add(c);
    return c.future;
  }

  @override
  Future<GifPage> trending({String? cursor}) => search('', cursor: cursor);
  @override
  Future<GifDescriptor> resolve(String id) async => fixtureGif;
}

class RealHttpOverrides extends HttpOverrides {}

void main() {
  test(
    'adapter is blocked by default and missing key has distinct state',
    () async {
      final p = GiphyGifProvider();
      expect(p.status, 'blocked');
      await expectLater(
        p.trending(),
        throwsA(isA<GifFailure>().having((e) => e.code, 'code', 'blocked')),
      );
      final missing = GiphyGifProvider(approved: true);
      expect(missing.status, 'unavailable');
      await expectLater(missing.trending(), throwsA(isA<GifFailure>()));
    },
  );
  test('real adapter maps search pagination trending resolve with rating and no provider blob', () async {
    final calls = <Uri>[];
    final p = GiphyGifProvider(
      apiKey: 'synthetic-test-credential',
      approved: true,
      transport: (uri) async {
        calls.add(uri);
        if (uri.path.endsWith('/abc')) return {'data': row('abc')};
        final offset = int.parse(uri.queryParameters['offset']!);
        return page([row(offset == 0 ? 'abc' : 'def')], offset: offset);
      },
    );
    final first = await p.search('cat');
    expect(first.items.single.id, 'abc');
    expect(first.next, '1');
    final second = await p.search('cat', cursor: first.next);
    expect(second.items.single.id, 'def');
    expect(second.next, isNull);
    await expectLater(p.trending(cursor: '500'), throwsA(isA<GifFailure>()));
    await p.trending();
    expect(calls.last.path, '/v1/gifs/trending');
    expect((await p.resolve('abc')).id, 'abc');
    expect(calls.first.queryParameters['rating'], 'g');
    expect(calls.first.queryParameters['limit'], '20');
    expect(
      first.items.single.toJson().keys,
      unorderedEquals([
        "provider",
        "provider_content_id",
        "media_url",
        "preview_url",
        "width",
        "height",
        "alt_text",
      ]),
    );
  });
  test(
    'empty malformed missing asset and invalid pagination fail safely',
    () async {
      final empty = GiphyGifProvider(
        apiKey: 'synthetic',
        approved: true,
        transport: (_) async => page([], total: 0),
      );
      expect((await empty.trending()).items, isEmpty);
      for (final result in [
        {},
        page([
          {'id': 'abc'},
        ]),
        page([row('abc')], offset: 3),
        page([
          {...row('abc'), 'rating': 'r'},
        ]),
      ]) {
        final p = GiphyGifProvider(
          apiKey: 'synthetic',
          approved: true,
          transport: (_) async => result.cast<String, dynamic>(),
        );
        await expectLater(
          p.trending(),
          throwsA(isA<GifFailure>().having((e) => e.code, 'code', 'malformed')),
        );
      }
    },
  );
  test(
    'HTTP fixtures cover 429 malformed JSON offline and oversized response',
    () async {
      await HttpOverrides.runWithHttpOverrides(() async {
        final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
        server.listen((request) async {
          if (request.uri.path == '/429') {
            request.response.statusCode = 429;
          } else if (request.uri.path == '/large') {
            request.response.write('x' * (2 * 1024 * 1024 + 1));
          } else {
            request.response.write('{bad');
          }
          await request.response.close();
        });
        final base = 'http://127.0.0.1:${server.port}';
        for (final path in ['429', 'bad', 'large']) {
          await expectLater(
            gifHttp(Uri.parse('$base/$path')),
            throwsA(
              isA<GifFailure>().having(
                (e) => e.code,
                'code',
                path == '429' ? 'rate_limited' : 'malformed',
              ),
            ),
          );
        }
        await server.close(force: true);
        await expectLater(
          gifHttp(Uri.parse(base)),
          throwsA(isA<GifFailure>().having((e) => e.code, 'code', 'offline')),
        );
      }, RealHttpOverrides());
    },
  );
  testWidgets('debounce stale responses cancellation pagination and timeout', (
    t,
  ) async {
    final provider = ControlledProvider();
    final search = GifSearch(provider);
    search.change('cat');
    await t.pump(const Duration(milliseconds: 200));
    search.change('ca');
    await t.pump(const Duration(milliseconds: 200));
    expect(provider.requests, isEmpty);
    await t.pump(const Duration(milliseconds: 150));
    expect(provider.requests.length, 1);
    search.change('car');
    provider.requests[0].complete(const GifPage([fixtureGif]));
    await t.pump();
    expect(search.items, isEmpty);
    await t.pump(const Duration(milliseconds: 350));
    provider.requests[1].complete(const GifPage([fixtureGif], next: 'next'));
    await t.pump();
    expect(search.items.length, 1);
    final more = search.load(more: true);
    provider.requests[2].complete(const GifPage([fixtureGif]));
    await t.pump();
    await more;
    expect(search.items.length, 2);
    final pending = search.load();
    await t.pump(const Duration(seconds: 9));
    await pending;
    expect(search.error, 'timeout');
    search.change('cancelled');
    search.dispose();
    await t.pump(const Duration(seconds: 1));
    expect(provider.requests.length, 4);
    final other = GifSearch(provider);
    final old = other.load();
    other.dispose();
    provider.requests.last.complete(const GifPage([fixtureGif]));
    await t.pump();
    await old;
  });
  testWidgets('bounded memory LRU clear and approved fixture only', (t) async {
    final cache = GifMediaCache();
    cache.configureImages();
    for (int i = 0; i < 50; i++) {
      cache.put('$i', Uint8List(16));
    }
    expect(cache.entries, 32);
    for (int i = 0; i < 10; i++) {
      cache.put('large$i', Uint8List(2 * 1024 * 1024));
    }
    expect(cache.byteCount, lessThanOrEqualTo(GifMediaCache.maxBytes));
    expect(cache.entries, 4);
    final loaded = await t.runAsync(() => cache.load(fixtureGif));
    expect(loaded!.length, greaterThan(0));
    final giphy = GifDescriptor(
      provider: 'giphy',
      id: 'abc',
      mediaUrl: 'https://media.giphy.com/media/abc/a.gif',
      width: 160,
      height: 100,
    );
    await expectLater(
      cache.load(giphy),
      throwsA(isA<GifFailure>().having((e) => e.code, 'code', 'blocked')),
    );
    cache.clear();
    expect(cache.entries, 0);
    expect(cache.byteCount, 0);
    cache.dispose();
  });
  test('descriptors reject arbitrary locations and invalid dimensions', () {
    for (final url in [
      'file:///tmp/a.gif',
      'data:a',
      'javascript:a',
      'http://localhost/a.gif',
      'https://evil/a.gif',
    ]) {
      final bad = {...fixtureGif.toJson(), 'media_url': url};
      expect(() => GifDescriptor.fromJson(bad), throwsA(isA<GifFailure>()));
    }
    expect(
      () => GifDescriptor.fromJson({...fixtureGif.toJson(), 'height': 4000}),
      throwsA(isA<GifFailure>()),
    );
  });
}
