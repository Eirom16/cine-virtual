import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:cine_mobile_spike/presentation/application_controller.dart';
import 'package:cine_mobile_spike/social/gif_provider.dart';
import 'package:cine_mobile_spike/ui/components/gif_widgets.dart';
import 'package:cine_mobile_spike/ui/components/social_panel.dart';
import 'package:cine_mobile_spike/ui/screens/player_screen.dart';

import 'social_test.dart' show SocialGateway, entry, socialScreen, capture;

class RichGateway extends SocialGateway {
  RichGateway() {
    social['rich_supported'] = true;
  }
  @override
  Map<String, dynamic> call(String type, [Map<String, dynamic>? fields]) {
    final result = super.call(type, fields);
    if (type != 'state' && fields?['action'] == 'message') {
      payloads.add(fields!);
      if (rejection == null) {
        final content = fields['content'] as Map;
        final id = (social['social_sequence'] as int) + 1;
        append({
          ...entry(id, sender: 'host'),
          'content': content,
          'text': content['type'] == 'gif' ? '[GIF]' : content['text'],
          'reply_to_message_id': fields['reply_to_message_id'],
          'sent_at_utc_ms': 1780936920000,
        });
      } else {
        network['error'] = rejection;
      }
    }
    if (type != 'state' && fields?['action'] == 'message_reaction') {
      payloads.add(fields!);
      final e = (social['entries'] as List).cast<Map>().firstWhere(
        (e) => e['message_id'] == fields['message_id'],
      );
      final reactions = (e['message_reactions'] ??= <String, dynamic>{}) as Map;
      final emoji = fields['emoji'];
      if (reactions.containsKey(emoji)) {
        reactions.remove(emoji);
      } else {
        reactions[emoji] = ['host'];
      }
      social['social_sequence'] = (social['social_sequence'] as int) + 1;
    }
    return result;
  }
}

Future<ApplicationController> setup(WidgetTester t) async {
  t.binding.platformDispatcher.accessibilityFeaturesTestValue =
      FakeAccessibilityFeatures(disableAnimations: true);
  addTearDown(t.binding.platformDispatcher.clearAccessibilityFeaturesTestValue);
  final c = ApplicationController(gateway: RichGateway())..session = true;
  c.gifProvider = const FixtureGifProvider();
  await c.initialize(startPolling: false);
  addTearDown(c.dispose);
  return c;
}

Widget reduced(Widget child) => Builder(
  builder: (context) => MediaQuery(
    data: MediaQuery.of(context).copyWith(disableAnimations: true),
    child: child,
  ),
);
Future<void> frames(WidgetTester t) async {
  await t.runAsync(
    () => Future<void>.delayed(const Duration(milliseconds: 100)),
  );
  await t.pump();
}

Future<void> settle(WidgetTester t) async {
  await t.pump(const Duration(milliseconds: 300));
  await frames(t);
  await frames(t);
  await t.pumpAndSettle(
    const Duration(milliseconds: 100),
    EnginePhase.sendSemanticsUpdate,
    const Duration(seconds: 3),
  );
}

Map<String, dynamic> gifEntry(int i) => {
  ...entry(i),
  'content': {'type': 'gif', 'gif': fixtureGif.toJson()},
  'text': '[GIF]',
  'sent_at_utc_ms': 1780936920000,
};
void main() {
  setUpAll(() async {
    final fonts = Platform.environment['CINE_SOCIAL_FONT_DIR'];
    if (fonts == null) return;
    for (final pair in [
      ['Roboto', 'Roboto-Regular.ttf'],
      ['MaterialIcons', 'MaterialIcons-Regular.otf'],
    ]) {
      final loader = FontLoader(pair[0]);
      loader.addFont(
        File('$fonts/${pair[1]}').readAsBytes().then(ByteData.sublistView),
      );
      await loader.load();
    }
    final emoji = Platform.environment['CINE_SOCIAL_EMOJI_FONT'];
    if (emoji != null) {
      final loader = FontLoader('Noto Color Emoji');
      loader.addFont(File(emoji).readAsBytes().then(ByteData.sublistView));
      await loader.load();
    }
  });
  testWidgets(
    'picker opens closes searches selects authoritative GIF and unread',
    (t) async {
      final c = await setup(t);
      final g = c.gateway as RichGateway;
      await socialScreen(t, reduced(SocialPanel(controller: c)));
      await t.tap(find.byKey(const Key('gif-picker')));
      await t.pump();
      await frames(t);
      expect(find.byKey(const Key('gif-search')), findsOneWidget);
      expect(g.payloads, isEmpty);
      await t.enterText(find.byKey(const Key('gif-search')), 'not found');
      await t.pump(const Duration(milliseconds: 360));
      await t.pump();
      expect(find.text('No se encontraron GIFs.'), findsOneWidget);
      await t.enterText(find.byKey(const Key('gif-search')), 'cine');
      await t.pump(const Duration(milliseconds: 360));
      await frames(t);
      await t.tap(find.byKey(const ValueKey('gif-select-0')));
      await t.pump();
      await t.pump(const Duration(milliseconds: 300));
      await frames(t);
      expect(find.byKey(const Key('gif-search')), findsNothing);
      expect(g.payloads.last['action'], 'message');
      expect(find.byType(GifImage), findsOneWidget);
      expect(c.social.value.entries.length, 1);
      await t.pumpWidget(const SizedBox());
      g.append(gifEntry(2));
      c.poll();
      expect(c.social.value.unread, 1);
    },
  );
  testWidgets('new GIF autoscroll resumes image stream inside viewport', (
    t,
  ) async {
    t.view.physicalSize = const Size(1366, 768);
    t.view.devicePixelRatio = 1;
    addTearDown(t.view.resetPhysicalSize);
    addTearDown(t.view.resetDevicePixelRatio);
    final c = await setup(t);
    final g = c.gateway as RichGateway;
    await socialScreen(
      t,
      reduced(
        SizedBox(width: 300, height: 600, child: SocialPanel(controller: c)),
      ),
    );
    for (int i = 1; i <= 8; i++) {
      g.append(gifEntry(i));
      c.poll();
      await settle(t);
    }
    final last = find.byKey(const ValueKey<dynamic>('message-8'));
    final image = find.descendant(of: last, matching: find.byType(RawImage));
    expect(image, findsOneWidget);
    final position = t
        .state<ScrollableState>(
          find.descendant(
            of: find.byKey(const Key('chat-messages')),
            matching: find.byType(Scrollable),
          ),
        )
        .position;
    expect(position.extentAfter, lessThan(1));
    await t.drag(find.byKey(const Key('chat-messages')), const Offset(0, 500));
    await settle(t);
    final offset = position.pixels;
    g.append(gifEntry(9));
    c.poll();
    await settle(t);
    expect(position.pixels, closeTo(offset, 1));
    await t.pumpWidget(const SizedBox());
  });
  testWidgets('blocked provider retains text chat and offers retry', (t) async {
    final c = await setup(t);
    c.gifProvider = const UnavailableGifProvider();
    await socialScreen(t, reduced(SocialPanel(controller: c)));
    await t.tap(find.byKey(const Key('gif-picker')));
    await settle(t);
    expect(find.textContaining('Proveedor GIF bloqueado'), findsOneWidget);
    await t.tap(find.text('Reintentar'));
    await settle(t);
    await t.tap(find.byTooltip('Cerrar GIFs'));
    await settle(t);
    await t.enterText(find.byKey(const Key('chat-input')), 'Texto funciona');
    await t.pump();
    await t.tap(find.byKey(const Key('chat-send')));
    await settle(t);
    expect(find.text('Texto funciona'), findsOneWidget);
  });
  testWidgets('reply quote cancel copy reaction aggregate toggle and fallback', (
    t,
  ) async {
    final c = await setup(t);
    final g = c.gateway as RichGateway;
    g.append(gifEntry(1));
    c.poll();
    await socialScreen(t, reduced(SocialPanel(controller: c)));
    await frames(t);
    await t.tap(find.byTooltip('Acciones del mensaje'));
    await settle(t);
    await t.tap(find.text('Responder'));
    await settle(t);
    expect(find.textContaining('Respondiendo a Sam · GIF'), findsOneWidget);
    await t.tap(find.byTooltip('Cancelar respuesta'));
    await settle(t);
    expect(c.replyToMessageId, isNull);
    await t.tap(find.byTooltip('Acciones del mensaje'));
    await settle(t);
    await t.tap(find.text('Responder'));
    await settle(t);
    await t.enterText(find.byKey(const Key('chat-input')), 'JAJAJA');
    await t.pump();
    await t.tap(find.byKey(const Key('chat-send')));
    await settle(t);
    expect(c.social.value.error, '', reason: 'reply send failed');
    expect(
      g.payloads,
      isNotEmpty,
      reason:
          'rich intent missing: ${g.intents} / ${c.social.value.canSend} / ${c.replyToMessageId}',
    );
    expect(g.payloads.last['reply_to_message_id'], 'message-1');
    expect(find.text('Sam · GIF'), findsOneWidget);
    await t.tap(find.byTooltip('Acciones del mensaje').last);
    await settle(t);
    await t.tap(find.text('Reaccionar al mensaje'));
    await settle(t);
    await t.tap(find.text('❤️  amor'));
    await settle(t);
    expect(find.text('❤️ 1'), findsOneWidget);
    await t.tap(find.text('❤️ 1'));
    await settle(t);
    expect(find.text('❤️ 1'), findsNothing);
    String? copied;
    t.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        if (call.method == 'Clipboard.setData') copied = call.arguments['text'];
        return null;
      },
    );
    await t.tap(find.byTooltip('Acciones del mensaje').last);
    await settle(t);
    await t.tap(find.text('Copiar'));
    await settle(t);
    expect(copied, 'JAJAJA');
    t.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      null,
    );
    (g.social['entries'] as List).removeAt(0);
    c.poll();
    await settle(t);
    expect(find.text('Mensaje anterior no disponible'), findsOneWidget);
    expect(jsonEncode(c.diagnostics.value), isNot(contains('JAJAJA')));
    await t.pumpWidget(const SizedBox());
  });
  testWidgets(
    'failed GIF keeps dimensions and reduced motion has one still frame',
    (t) async {
      final c = await setup(t);
      final g = c.gateway as RichGateway;
      g.append(gifEntry(1));
      c.poll();
      await socialScreen(t, reduced(SocialPanel(controller: c)));
      await frames(t);
      await frames(t);
      expect(find.byType(RawImage), findsOneWidget);
      expect(find.byType(Image), findsNothing);
      final before = t.getSize(find.byType(GifImage));
      (g.social['entries'] as List)[0]['content']['gif']['provider'] = 'giphy';
      (g.social['entries']
              as List)[0]['content']['gif']['provider_content_id'] =
          'abc';
      (g.social['entries'] as List)[0]['content']['gif']['media_url'] =
          'https://media.giphy.com/media/abc/a.gif';
      c.poll();
      await t.pump();
      await frames(t);
      expect(find.text('GIF no disponible'), findsOneWidget);
      expect(t.getSize(find.byType(GifImage)), before);
      await t.pumpWidget(const SizedBox());
    },
  );
  for (final size in [
    const Size(1366, 768),
    const Size(1920, 1080),
    const Size(390, 844),
    const Size(844, 390),
    const Size(568, 320),
  ]) {
    testWidgets('rich responsive fullscreen keyboard picker and menu $size', (
      t,
    ) async {
      t.view.physicalSize = size;
      t.view.devicePixelRatio = 1;
      addTearDown(t.view.resetPhysicalSize);
      addTearDown(t.view.resetDevicePixelRatio);
      final c = await setup(t);
      final g = c.gateway as RichGateway;
      g.append(gifEntry(1));
      g.append({
        ...entry(2),
        'reply_to_message_id': 'message-1',
        'message_reactions': {
          '❤️': ['friend'],
        },
      });
      c.poll();
      await socialScreen(
        t,
        reduced(PlayerScreen(controller: c, onLobby: () {}, fullscreen: true)),
      );
      await t.tap(find.byKey(const Key('open-chat')));
      await settle(t);
      await frames(t);
      expect(t.takeException(), isNull);
      await capture(t, 'rich-${size.width.toInt()}x${size.height.toInt()}');
      await t.tap(find.byKey(const Key('gif-picker')));
      await settle(t);
      await frames(t);
      expect(t.takeException(), isNull);
      await capture(t, 'picker-${size.width.toInt()}x${size.height.toInt()}');
      if (size.width < 900) {
        t.view.viewInsets = FakeViewPadding(
          bottom: size.height > 400 ? 300 : 150,
        );
        await settle(t);
        expect(t.takeException(), isNull);
        t.view.resetViewInsets();
      }
      await t.tap(find.byTooltip('Cerrar GIFs'));
      await settle(t);
      await t.ensureVisible(find.byTooltip('Acciones del mensaje').last);
      await settle(t);
      await t.tap(find.byTooltip('Acciones del mensaje').last);
      await settle(t);
      expect(t.takeException(), isNull);
      await t.tap(find.text('Responder'));
      await settle(t);
      expect(find.byTooltip('Cancelar respuesta'), findsOneWidget);
      expect(t.takeException(), isNull);
      await t.pumpWidget(const SizedBox());
      await settle(t);
    });
  }
  testWidgets('Android platform view persists across rich UI interactions', (
    t,
  ) async {
    int created = 0, disposed = 0;
    t.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform_views,
      (call) async {
        if (call.method == 'create') {
          created++;
          return 1;
        }
        if (call.method == 'dispose') disposed++;
        if (call.method == 'resize') {
          return {
            'width': call.arguments['width'],
            'height': call.arguments['height'],
          };
        }
        return null;
      },
    );
    addTearDown(
      () => t.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform_views,
        null,
      ),
    );
    final c = await setup(t);
    final g = c.gateway as RichGateway;
    g.mobile = true;
    g.append(gifEntry(1));
    c.poll();
    await socialScreen(t, reduced(PlayerScreen(controller: c, onLobby: () {})));
    await settle(t);
    expect(created, 1);
    await t.tap(find.byKey(const Key('open-chat')));
    await settle(t);
    await frames(t);
    await t.tap(find.byKey(const Key('gif-picker')));
    await settle(t);
    await frames(t);
    await t.tap(find.byTooltip('Cerrar GIFs'));
    await settle(t);
    await t.tap(find.byTooltip('Acciones del mensaje'));
    await settle(t);
    await t.tap(find.text('Responder'));
    await settle(t);
    expect(created, 1);
    expect(disposed, 0);
    await t.pumpWidget(const SizedBox());
    await t.pump();
    expect(disposed, 1);
  });
}
