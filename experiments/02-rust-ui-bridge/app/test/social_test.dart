import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:cine_mobile_spike/presentation/application_controller.dart';
import 'package:cine_mobile_spike/ui/components/social_panel.dart';
import 'package:cine_mobile_spike/ui/screens/player_screen.dart';

import 'package:cine_mobile_spike/ui/theme/product_theme.dart';

import 'product_ui_test.dart' show TestGateway;

Map<String, dynamic> entry(
  int i, {
  String kind = 'chat',
  String sender = 'friend',
}) => {
  'message_id': 'message-$i',
  'sender_id': sender,
  'display_name': sender == 'host' ? 'Alex' : 'Sam',
  'social_sequence': i,
  'sent_at_ms': i * 1000,
  'kind': kind,
  'text': kind == 'chat' ? 'Mensaje $i 😂' : '',
};

class SocialGateway extends TestGateway {
  String? rejection;
  final payloads = <Map<String, dynamic>>[];
  SocialGateway() {
    network['social'] = {
      'supported': true,
      'social_sequence': 0,
      'entries': [],
      'reactions': [],
    };
  }
  Map<String, dynamic> get social =>
      (network['social'] as Map).cast<String, dynamic>();
  void append(Map<String, dynamic> e) {
    (social['entries'] as List).add(e);
    social['social_sequence'] = e['social_sequence'];
  }

  @override
  Map<String, dynamic> call(String type, [Map<String, dynamic>? fields]) {
    final result = super.call(type, fields);
    if (type != 'state' && ['chat', 'reaction'].contains(fields?['action'])) {
      payloads.add(fields!);
      if (rejection == null) {
        network.remove('error');
      } else {
        network['error'] = rejection;
      }
      if (rejection == null && fields['action'] == 'chat') {
        final id = (social['social_sequence'] as int) + 1;
        append({
          ...entry(id, sender: 'host'),
          'text': (fields['text'] as String).trim(),
        });
      }
    }
    return result;
  }
}

Future<ApplicationController> setup(WidgetTester t) async {
  final c = ApplicationController(gateway: SocialGateway())..session = true;
  await c.initialize(startPolling: false);
  addTearDown(c.dispose);
  return c;
}

Future<void> socialScreen(WidgetTester tester, Widget child) async {
  var theme = productTheme();
  if (Platform.environment['CINE_SOCIAL_CAPTURE'] != null) {
    final text = theme.textTheme.apply(fontFamily: 'Roboto');
    theme = theme.copyWith(
      textTheme: text.copyWith(
        bodyMedium: text.bodyMedium!.copyWith(
          fontFamilyFallback: ['Noto Color Emoji'],
        ),
        bodyLarge: text.bodyLarge!.copyWith(
          fontFamilyFallback: ['Noto Color Emoji'],
        ),
      ),
    );
  }
  await tester.pumpWidget(
    RepaintBoundary(
      key: const Key('social-capture'),
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: theme,
        home: Scaffold(resizeToAvoidBottomInset: false, body: child),
      ),
    ),
  );
  await tester.pump();
}

Future<void> capture(WidgetTester t, String name) async {
  final path = Platform.environment['CINE_SOCIAL_CAPTURE'];
  if (path == null) return;
  final boundary = t.renderObject<RenderRepaintBoundary>(
    find.byKey(const Key('social-capture')),
  );
  await t.runAsync(() async {
    final image = await boundary.toImage(pixelRatio: 1);
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    await File('$path/$name.png').writeAsBytes(bytes!.buffer.asUint8List());
    image.dispose();
  });
}

void main() {
  setUpAll(() async {
    final fonts = Platform.environment['CINE_SOCIAL_FONT_DIR'];
    if (Platform.environment['CINE_SOCIAL_CAPTURE'] == null || fonts == null) {
      return;
    }
    for (final pair in [
      ['Roboto', 'Roboto-Regular.ttf'],
      ['MaterialIcons', 'MaterialIcons-Regular.otf'],
    ]) {
      final loader = FontLoader(pair[0]);
      loader.addFont(
        File('$fonts/${pair[1]}')
            .readAsBytes()
            .then((bytes) => ByteData.sublistView(bytes)),
      );
      await loader.load();
    }
    final emoji = Platform.environment['CINE_SOCIAL_EMOJI_FONT'];
    if (emoji != null) {
      final loader = FontLoader('Noto Color Emoji');
      loader.addFont(
        File(emoji).readAsBytes().then((bytes) => ByteData.sublistView(bytes)),
      );
      await loader.load();
    }
  });
  testWidgets(
    'closed chat unread then open renders mine other and system safely',
    (t) async {
      final c = await setup(t);
      final g = c.gateway as SocialGateway;
      await socialScreen(
        t,
        Row(
          children: [
            ChatButton(controller: c, onPressed: () {}),
            const Text('Player'),
          ],
        ),
      );
      expect(find.byKey(const Key('chat-input')), findsNothing);
      g.append(entry(1));
      g.append(entry(2, sender: 'host'));
      g.append(entry(3, kind: 'joined'));
      c.poll();
      await t.pump();
      expect(c.social.value.unread, 1);
      expect(find.byTooltip('Chat · 1 sin leer'), findsOneWidget);
      await socialScreen(t, SocialPanel(controller: c));
      await t.pump();
      expect(c.social.value.unread, 0);
      expect(find.text('Alex · tú'), findsOneWidget);
      expect(find.text('Sam se unió a la sala'), findsOneWidget);
      expect(find.text('Mensaje 1 😂'), findsOneWidget);
      await t.pumpWidget(const SizedBox());
    },
  );
  testWidgets(
    'desktop enter sends authoritative echo Shift Enter is editable and errors retain draft',
    (t) async {
      final c = await setup(t);
      final g = c.gateway as SocialGateway;
      await socialScreen(t, SocialPanel(controller: c));
      await t.enterText(find.byKey(const Key('chat-input')), 'Hola 😂');
      await t.sendKeyEvent(LogicalKeyboardKey.enter);
      await t.pumpAndSettle();
      expect(g.payloads.last['text'], 'Hola 😂');
      expect(find.text('Hola 😂'), findsOneWidget);
      expect(
        (t.widget<TextField>(find.byKey(const Key('chat-input'))))
            .controller!
            .text,
        '',
      );
      await t.enterText(find.byKey(const Key('chat-input')), 'otra');
      await t.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      await t.sendKeyEvent(LogicalKeyboardKey.enter);
      await t.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await t.pump();
      expect(g.payloads.length, 1);
      g.rejection = 'RATE_LIMITED';
      await t.tap(find.byKey(const Key('chat-send')));
      await t.pumpAndSettle();
      expect(find.textContaining('Vas muy rápido'), findsOneWidget);
      expect(c.chatDraft, 'otra');
      final before = g.payloads.length;
      expect(await c.sendChat('😂' * 513), false);
      expect(g.payloads.length, before);
      expect(await c.sendChat(' \n '), false);
      await t.pumpWidget(const SizedBox());
    },
  );
  testWidgets(
    'editor focus blocks playback and fullscreen keyboard shortcuts',
    (t) async {
      t.view.physicalSize = const Size(1366, 768);
      t.view.devicePixelRatio = 1;
      addTearDown(t.view.resetPhysicalSize);
      addTearDown(t.view.resetDevicePixelRatio);
      final c = await setup(t);
      final g = c.gateway as SocialGateway;
      int fullscreen = 0;
      await socialScreen(
        t,
        PlayerScreen(
          controller: c,
          onLobby: () {},
          onFullscreen: () => fullscreen++,
        ),
      );
      await t.tap(find.byKey(const Key('open-chat')));
      await t.pumpAndSettle();
      await t.enterText(find.byKey(const Key('chat-input')), 'texto');
      final editor = t
          .widget<TextField>(find.byKey(const Key('chat-input')))
          .controller!;
      await t.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
      await t.pump();
      expect(editor.selection.baseOffset, 4);
      t.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (call) async {
          if (call.method == 'Clipboard.getData') {
            return {'text': '¿Viste eso? 😂'};
          }
          return null;
        },
      );
      await t.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await t.sendKeyEvent(LogicalKeyboardKey.keyA);
      await t.sendKeyEvent(LogicalKeyboardKey.keyV);
      await t.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await t.pumpAndSettle();
      expect(editor.text, '¿Viste eso? 😂');
      t.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        null,
      );
      for (final key in [
        LogicalKeyboardKey.space,
        LogicalKeyboardKey.arrowLeft,
        LogicalKeyboardKey.arrowRight,
        LogicalKeyboardKey.keyF,
      ]) {
        await t.sendKeyEvent(key);
      }
      await t.pump();
      expect(g.intents, isNot(contains('play')));
      expect(g.intents, isNot(contains('seek')));
      expect(fullscreen, 0);
      await t.sendKeyEvent(LogicalKeyboardKey.escape);
      await t.pumpAndSettle();
      expect(find.byKey(const Key('chat-input')), findsNothing);
      await t.pumpWidget(const SizedBox());
    },
  );
  testWidgets(
    'social notifications arriving during layout wait for the frame',
    (t) async {
      final c = await setup(t);
      var sent = false;
      await socialScreen(
        t,
        Column(
          children: [
            Expanded(child: SocialPanel(controller: c)),
            ValueListenableBuilder(
              valueListenable: c.playback,
              builder: (_, value, _) => Text('${value.position}'),
            ),
            LayoutBuilder(
              builder: (_, _) {
                if (!sent) {
                  sent = true;
                  (c.gateway as SocialGateway).sync['position_ms'] = 6000;
                  unawaited(c.sendReaction('😂'));
                }
                return const SizedBox();
              },
            ),
          ],
        ),
      );
      await t.pumpAndSettle();
      expect(t.takeException(), isNull);
      expect(find.text('6000'), findsOneWidget);
      expect((c.gateway as SocialGateway).payloads.last['emoji'], '😂');
      await t.pumpWidget(const SizedBox());
    },
  );
  testWidgets(
    'auto scroll follows bottom and preserves readers with new messages indicator',
    (t) async {
      final c = await setup(t);
      final g = c.gateway as SocialGateway;
      for (int i = 1; i <= 40; i++) {
        g.append(entry(i));
      }
      c.poll();
      await socialScreen(t, SocialPanel(controller: c));
      await t.pumpAndSettle();
      final list = find.byKey(const Key('chat-messages'));
      final scroll = t.widget<ListView>(list).controller!;
      expect(scroll.position.extentAfter, 0);
      scroll.jumpTo(0);
      await t.pump();
      g.append(entry(41));
      c.poll();
      await t.pumpAndSettle();
      expect(scroll.offset, 0);
      expect(find.text('Nuevos mensajes'), findsOneWidget);
      await t.tap(find.text('Nuevos mensajes'));
      await t.pump();
      expect(scroll.position.extentAfter, 0);
      g.append(entry(42));
      c.poll();
      await t.pumpAndSettle();
      expect(scroll.position.extentAfter, 0);
      await t.pumpWidget(const SizedBox());
    },
  );
  testWidgets('reaction picker names and bounded isolated animation cleanup', (
    t,
  ) async {
    final c = await setup(t);
    final g = c.gateway as SocialGateway;
    await socialScreen(
      t,
      Stack(
        children: [
          ReactionOverlay(controller: c),
          Align(
            alignment: Alignment.topLeft,
            child: ReactionPicker(controller: c),
          ),
        ],
      ),
    );
    await t.tap(find.byKey(const Key('reaction-picker')));
    await t.pumpAndSettle();
    expect(find.bySemanticsLabel('Reaccionar con risa'), findsOneWidget);
    await t.tap(find.text('😂  risa'));
    await t.pumpAndSettle();
    expect(g.payloads.last['emoji'], '😂');
    g.social['reactions'] = [
      for (int i = 0; i < 50; i++) {'reaction_id': 'r$i', 'emoji': '😂'},
    ];
    c.poll();
    await t.pump();
    expect(find.text('😂'), findsNWidgets(12));
    await t.pump(const Duration(milliseconds: 2300));
    expect(find.text('😂'), findsNothing);
    c.poll();
    await t.pump();
    expect(find.text('😂'), findsNothing);
    await t.pumpWidget(const SizedBox());
  });
  for (final size in [
    const Size(1366, 768),
    const Size(1920, 1080),
    const Size(390, 844),
    const Size(844, 390),
    const Size(568, 320),
  ]) {
    testWidgets('social responsive fullscreen at $size', (t) async {
      t.view.physicalSize = size;
      t.view.devicePixelRatio = 1;
      addTearDown(t.view.resetPhysicalSize);
      addTearDown(t.view.resetDevicePixelRatio);
      final c = await setup(t);
      final g = c.gateway as SocialGateway;
      g.append({...entry(1), 'text': 'Unicode 👏 ${'palabra ' * 70}'});
      c.poll();
      await socialScreen(
        t,
        PlayerScreen(controller: c, onLobby: () {}, fullscreen: true),
      );
      await t.tap(find.byKey(const Key('open-chat')));
      await t.pumpAndSettle();
      expect(find.byKey(const Key('chat-input')), findsOneWidget);
      expect(t.takeException(), isNull);
      g.social['reactions'] = [
        {'reaction_id': 'capture-${size.width}', 'emoji': '😂'},
      ];
      c.poll();
      await t.pump();
      await t.pump(const Duration(milliseconds: 400));
      await capture(t, 'widget-${size.width.toInt()}x${size.height.toInt()}');
      if (size.width < 900) {
        t.view.viewInsets = FakeViewPadding(
          bottom: size.height > 400 ? 300 : 200,
        );
        await t.pumpAndSettle();
        expect(t.takeException(), isNull);
        t.view.resetViewInsets();
      }
      await t.pumpWidget(const SizedBox());
      await t.pumpAndSettle();
    });
  }
  testWidgets(
    'social updates do not notify room structure and diagnostics omit text',
    (t) async {
      final c = await setup(t);
      final g = c.gateway as SocialGateway;
      int roomChanges = 0;
      c.addListener(() => roomChanges++);
      g.append({...entry(1), 'text': 'private-text-unique'});
      c.poll();
      g.social['reactions'] = [
        {'reaction_id': 'r', 'emoji': '😂'},
      ];
      c.poll();
      expect(roomChanges, 0);
      expect(
        jsonEncode(c.diagnostics.value),
        isNot(contains('private-text-unique')),
      );
    },
  );
}
