import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:cine_mobile_spike/bridge.dart';
import 'package:cine_mobile_spike/product_app.dart';
import 'package:cine_mobile_spike/presentation/application_controller.dart';
import 'package:cine_mobile_spike/presentation/view_state.dart';
import 'package:cine_mobile_spike/ui/components/product_components.dart';
import 'package:cine_mobile_spike/ui/screens/lobby_screen.dart';
import 'package:cine_mobile_spike/ui/screens/player_screen.dart';
import 'package:cine_mobile_spike/ui/theme/product_theme.dart';

const roomId = '12345678-1234-4123-8123-123456789012';
const credentials = {
  'room_id': roomId,
  'room_epoch': roomId,
  'invite_token': 'test-only-token',
};

class TestGateway implements SessionGateway {
  @override
  bool get android => false;
  @override
  bool get supportsPlayer => true;
  final intents = <String>[];
  final Map<String, dynamic> snapshot = {
    'generation': 1,
    'hash': {'state': 'complete'},
    'network': {
      'connected': true,
      'busy': false,
      'presentation': {
        'member_id': 'host',
        'room': {
          'room_id': roomId,
          'room_epoch': roomId,
          'host_id': 'host',
          'sequence': 1,
          'members': [
            {
              'member_id': 'host',
              'display_name': 'Alex',
              'role': 'host',
              'ready': true,
              'connected': true,
              'status': 'ready',
            },
            {
              'member_id': 'friend',
              'display_name': 'Sam',
              'role': 'participant',
              'ready': true,
              'connected': true,
              'status': 'ready',
            },
          ],
          'media': {
            'descriptor': {'duration_ms': 30000},
          },
          'playback': {'pending': null},
        },
      },
      'sync': {
        'clock_trusted': true,
        'snapshot_required': false,
        'room_ready': true,
        'ready': true,
        'playing': false,
        'position_ms': 5000,
        'buffering': false,
        'seeking': false,
      },
      'media': {
        'identity_match': true,
        'duration_ms': 30000,
        'local_loaded': true,
      },
    },
  };
  Map<String, dynamic> get network => object(snapshot['network']);
  Map<String, dynamic> get room =>
      object(object(network['presentation'])['room']);
  Map<String, dynamic> get sync => object(network['sync']);
  @override
  Future<void> initialize() async {}
  @override
  Map<String, dynamic> call(String type, [Map<String, dynamic>? fields]) {
    if (type != 'state') {
      final action = '${fields?['action']}';
      intents.add(action);
      network['last_action'] = action;
      network['busy'] = false;
      network['result'] = action == 'create' ? credentials : {};
      if (action == 'play') {
        sync['playing'] = true;
      }
      if (action == 'pause') {
        sync['playing'] = false;
      }
      if (action == 'seek') {
        sync['position_ms'] = fields!['position_ms'];
      }
      if (action == 'disconnect') {
        network['connected'] = false;
      }
      if (action == 'reconnect') {
        network['connected'] = true;
      }
    }
    return object(jsonDecode(jsonEncode(snapshot)));
  }

  @override
  Future<Map<String, dynamic>?> pick() async => null;
  @override
  Future<void> prepareAndroid() async {}
  @override
  Future<void> dispose() async {}
}

Future<ApplicationController> fixture(
  WidgetTester tester, {
  bool session = true,
}) async {
  final c = ApplicationController(gateway: TestGateway());
  c.session = session;
  c.filename = session ? 'cine-fixture.mp4' : '';
  await c.initialize(startPolling: false);
  addTearDown(c.dispose);
  return c;
}

Future<void> screen(WidgetTester tester, Widget child) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: productTheme(),
      home: Scaffold(body: child),
    ),
  );
  await tester.pump();
}

void main() {
  test(
    'invitations use actual v1 credentials, native and product envelopes',
    () {
      expect(
        Invitation.parse(jsonEncode(credentials)).credentials,
        credentials,
      );
      final invite = Invitation.parse(
        jsonEncode({
          'server': 'ws://localhost:8765',
          'invitation': credentials,
        }),
      );
      expect(invite.server, 'ws://localhost:8765');
      expect(invite.credentials.keys, unorderedEquals(credentials.keys));
      expect(() => Invitation.parse('123456'), throwsA(isA<BridgeFailure>()));
    },
  );
  testWidgets('Home offers two entry actions without fictional content', (
    tester,
  ) async {
    final c = await fixture(tester, session: false);
    await tester.pumpWidget(CineVirtualApp(controller: c));
    expect(find.text('Crear sala'), findsOneWidget);
    expect(find.text('Unirse a una sala'), findsOneWidget);
    expect(find.textContaining('Drift'), findsNothing);
    expect(find.textContaining('Trending'), findsNothing);
  });
  testWidgets(
    'Create validates then dispatches connect/create and renders real members',
    (tester) async {
      final c = await fixture(tester, session: false);
      await tester.pumpWidget(CineVirtualApp(controller: c));
      await tester.tap(find.text('Crear sala'));
      await tester.pump();
      await tester.tap(find.byKey(const Key('enter-room')));
      await tester.pump();
      expect(find.text('Escribe tu nombre'), findsOneWidget);
      expect((c.gateway as TestGateway).intents, isEmpty);
      await tester.enterText(find.byKey(const Key('display-name')), 'Alex');
      await tester.tap(find.byKey(const Key('enter-room')));
      await tester.pumpAndSettle();
      expect((c.gateway as TestGateway).intents, ['connect', 'create']);
      expect(find.text('Copiar invitación'), findsOneWidget);
      expect(find.text('Alex · Tú'), findsOneWidget);
      expect(find.text('Anfitrión'), findsOneWidget);
    },
  );
  testWidgets('Join rejects invented codes and accepts full invitation', (
    tester,
  ) async {
    final c = await fixture(tester, session: false);
    await tester.pumpWidget(CineVirtualApp(controller: c));
    await tester.ensureVisible(find.text('Unirse a una sala'));
    await tester.tap(find.text('Unirse a una sala'));
    await tester.pump();
    await tester.enterText(find.byKey(const Key('display-name')), 'Sam');
    await tester.enterText(find.byKey(const Key('invitation')), '123456');
    await tester.ensureVisible(find.byKey(const Key('enter-room')));
    await tester.tap(find.byKey(const Key('enter-room')));
    await tester.pump();
    expect(find.text('Pega una invitación completa y válida.'), findsOneWidget);
    expect((c.gateway as TestGateway).intents, isEmpty);
    await tester.enterText(
      find.byKey(const Key('invitation')),
      jsonEncode(credentials),
    );
    await tester.ensureVisible(find.byKey(const Key('enter-room')));
    await tester.tap(find.byKey(const Key('enter-room')));
    await tester.pumpAndSettle();
    expect((c.gateway as TestGateway).intents, ['connect', 'join']);
  });
  testWidgets('Lobby empty and participant waiting states', (tester) async {
    final c = await fixture(tester);
    final g = c.gateway as TestGateway;
    g.room.remove('media');
    g.network['media'] = {};
    c.filename = '';
    c.poll();
    await screen(
      tester,
      LobbyScreen(controller: c, onPlayer: () {}, onShare: () {}),
    );
    expect(find.text('Elige qué vamos a ver.'), findsOneWidget);
    g.network['presentation']['member_id'] = 'friend';
    c.poll();
    await screen(
      tester,
      LobbyScreen(controller: c, onPlayer: () {}, onShare: () {}),
    );
    expect(find.text('Esperando al anfitrión.'), findsOneWidget);
    expect(find.text('Comenzar película'), findsNothing);
  });
  testWidgets('Ready covers real hash progress mismatch clock waiting ready', (
    tester,
  ) async {
    final c = await fixture(tester);
    final g = c.gateway as TestGateway;
    Future<void> render() async {
      c.poll();
      await screen(
        tester,
        ReadinessStatus(view: c.view, progress: c.hashProgress),
      );
    }

    g.snapshot['hash'] = {
      'state': 'running',
      'read_bytes': 78,
      'size_bytes': 100,
    };
    await render();
    expect(find.text('78%'), findsOneWidget);
    expect(
      tester
          .widget<LinearProgressIndicator>(find.byType(LinearProgressIndicator))
          .value,
      .78,
    );
    g.snapshot['hash'] = {'state': 'complete'};
    g.network['media']['identity_match'] = false;
    await render();
    expect(find.text('El archivo no coincide'), findsOneWidget);
    g.network['media']['identity_match'] = true;
    g.sync['clock_trusted'] = false;
    await render();
    expect(find.text('Sincronizando conexión…'), findsOneWidget);
    g.sync['clock_trusted'] = true;
    g.sync['room_ready'] = false;
    await render();
    expect(find.text('Archivo preparado'), findsOneWidget);
    g.sync['room_ready'] = true;
    await render();
    expect(find.text('Listo para ver'), findsOneWidget);
  });
  testWidgets(
    'Host controls dispatch play pause seek; participant has no controls',
    (tester) async {
      final c = await fixture(tester);
      final g = c.gateway as TestGateway;
      Future<void> render() =>
          screen(tester, PlayerControls(controller: c, onInteraction: () {}));
      await render();
      await tester.tap(find.byKey(const Key('player-toggle')));
      await tester.pump();
      await render();
      expect(g.intents.last, 'play');
      await tester.tap(find.byKey(const Key('player-toggle')));
      await tester.pump();
      await render();
      expect(g.intents.last, 'pause');
      tester.widget<Slider>(find.byKey(const Key('player-seek'))).onChangeEnd!(
        12000,
      );
      await tester.pump();
      expect(g.intents.last, 'seek');
      expect(c.playback.value.position, 12000);
      g.network['presentation']['member_id'] = 'friend';
      c.poll();
      await render();
      expect(
        tester
            .widget<IconButton>(find.byKey(const Key('player-toggle')))
            .onPressed,
        isNull,
      );
      expect(
        tester.widget<Slider>(find.byKey(const Key('player-seek'))).onChanged,
        isNull,
      );
      expect(
        find.text('El anfitrión controla la reproducción'),
        findsOneWidget,
      );
      final count = g.intents.length;
      await c.control('play');
      expect(g.intents.length, count);
    },
  );
  testWidgets('Disconnect preserves lobby; retry invokes Rust resume', (
    tester,
  ) async {
    final c = await fixture(tester, session: false);
    await tester.pumpWidget(CineVirtualApp(controller: c));
    await tester.tap(find.text('Crear sala'));
    await tester.pump();
    await tester.enterText(find.byKey(const Key('display-name')), 'Alex');
    await tester.tap(find.byKey(const Key('enter-room')));
    await tester.pumpAndSettle();
    final g = c.gateway as TestGateway;
    g.network['connected'] = false;
    c.poll();
    await tester.pump();
    expect(
      find.text('Se perdió la conexión. Tu sala permanece aquí.'),
      findsOneWidget,
    );
    expect(find.text('Copiar invitación'), findsOneWidget);
    await tester.tap(find.text('Reintentar'));
    await tester.pumpAndSettle();
    expect(g.intents.last, 'reconnect');
    expect(find.text('Conectado'), findsOneWidget);
  });
  testWidgets('Human errors and redacted diagnostics', (tester) async {
    final c = await fixture(tester);
    c.error = 'MEDIA_MISMATCH';
    c.poll();
    await screen(
      tester,
      ProductErrorBanner(code: c.error, onDismiss: c.dismissError),
    );
    expect(
      find.text('El archivo no coincide con el de la sala.'),
      findsOneWidget,
    );
    expect(find.text('MEDIA_MISMATCH'), findsNothing);
    expect(jsonEncode(c.diagnostics.value), isNot(contains('invite_token')));
  });
  testWidgets('Position ticks do not rebuild room tree', (tester) async {
    final c = await fixture(tester);
    int rebuilds = 0;
    c.addListener(() => rebuilds++);
    final g = c.gateway as TestGateway;
    for (var i = 0; i < 20; i++) {
      g.sync['position_ms'] = i * 500;
      c.poll();
    }
    expect(rebuilds, 0);
    expect(c.playback.value.position, 9500);
  });
  testWidgets('Home and Lobby work at 360x640 and 150% text', (tester) async {
    tester.view.physicalSize = const Size(360, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final c = await fixture(tester, session: false);
    await tester.pumpWidget(CineVirtualApp(controller: c));
    await tester.pump();
    expect(tester.takeException(), isNull);
    c.session = true;
    c.filename = 'cine-fixture.mp4';
    c.poll();
    await tester.pumpWidget(
      MaterialApp(
        theme: productTheme(),
        home: MediaQuery(
          data: const MediaQueryData(textScaler: TextScaler.linear(1.5)),
          child: Scaffold(
            body: LobbyScreen(controller: c, onPlayer: () {}, onShare: () {}),
          ),
        ),
      ),
    );
    await tester.pump();
    expect(tester.takeException(), isNull);
  });
  test('Connection labels follow actual operation state', () {
    expect(const RoomView().connection, ConnectionStatus.disconnected);
    expect(
      const RoomView(busy: true, action: 'reconnect').connection,
      ConnectionStatus.reconnecting,
    );
    expect(
      const RoomView(recovering: true).connection,
      ConnectionStatus.recovering,
    );
  });
  testWidgets(
    'Host waits for every connected member and for a pending transition',
    (tester) async {
      final c = await fixture(tester);
      final g = c.gateway as TestGateway;
      g.room['members'][1]['ready'] = false;
      c.poll();
      expect(c.view.canPlay, false);
      g.room['members'][1]['ready'] = true;
      g.room['playback'] = {
        'pending': {'sequence': 2},
      };
      c.poll();
      expect(c.view.canControl, false);
      final before = g.intents.length;
      await c.control('seek', 12000);
      expect(g.intents.length, before);
    },
  );
  testWidgets('Explicit exit works after connection has been lost', (
    tester,
  ) async {
    final c = await fixture(tester);
    final g = c.gateway as TestGateway;
    g.network['connected'] = false;
    c.poll();
    expect(await c.leave(), true);
    expect(g.intents.last, 'disconnect');
    expect(c.session, false);
  });
  testWidgets('Participant keyboard shortcuts never send authority intents', (
    tester,
  ) async {
    final c = await fixture(tester);
    final g = c.gateway as TestGateway;
    g.network['presentation']['member_id'] = 'friend';
    c.poll();
    await screen(tester, PlayerScreen(controller: c, onLobby: () {}));
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    expect(g.intents, isEmpty);
    await tester.pumpWidget(const SizedBox());
  });
  testWidgets('Player fits 640x360 landscape without overflow', (tester) async {
    tester.view.physicalSize = const Size(640, 360);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final c = await fixture(tester);
    await screen(tester, PlayerScreen(controller: c, onLobby: () {}));
    await tester.pump();
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });
  testWidgets(
    'Readiness distinguishes player loading, server verification and errors',
    (tester) async {
      final c = await fixture(tester);
      final g = c.gateway as TestGateway;
      g.sync['ready'] = false;
      c.poll();
      expect(c.view.readiness, Readiness.loading);
      g.sync['ready'] = true;
      c.busy = true;
      c.action = 'ready';
      c.poll();
      expect(c.view.readiness, Readiness.verifying);
      c.busy = false;
      c.error = 'HASH_FAILED';
      c.poll();
      expect(c.view.readiness, Readiness.error);
    },
  );
  testWidgets('Diagnostics removes authentication from endpoint', (
    tester,
  ) async {
    final c = await fixture(tester);
    c.endpoint = 'wss://name:password@example.test/room?token=private#secret';
    c.poll();
    final value = jsonEncode(c.diagnostics.value);
    expect(value, isNot(contains('password')));
    expect(value, isNot(contains('token=private')));
    expect(value, isNot(contains('#secret')));
  });
  testWidgets('Cancelled verification never enables Ready', (tester) async {
    final c = await fixture(tester);
    final g = c.gateway as TestGateway;
    g.sync['room_ready'] = false;
    g.snapshot['hash'] = {'state': 'cancelled'};
    c.poll();
    expect(c.view.canReady, false);
    expect(c.view.readiness, Readiness.cancelled);
    await screen(
      tester,
      ReadinessStatus(view: c.view, progress: c.hashProgress),
    );
    expect(find.text('Verificación cancelada'), findsOneWidget);
  });
  testWidgets('Idle controls can be restored and navigation stays available', (
    tester,
  ) async {
    final c = await fixture(tester);
    (c.gateway as TestGateway).sync['playing'] = true;
    c.poll();
    var returned = false;
    await screen(
      tester,
      PlayerScreen(controller: c, onLobby: () => returned = true),
    );
    await tester.pump(const Duration(seconds: 5));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('player-toggle')), findsNothing);
    await tester.tap(find.byTooltip('Mostrar controles'));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('player-toggle')), findsOneWidget);
    await tester.tap(find.byTooltip('Volver a la sala'));
    expect(returned, true);
    expect(tester.takeException(), isNull);
  });
}
