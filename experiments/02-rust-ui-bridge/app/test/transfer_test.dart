import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:cine_mobile_spike/presentation/application_controller.dart';
import 'package:cine_mobile_spike/presentation/view_state.dart';
import 'package:cine_mobile_spike/ui/components/transfer_card.dart';
import 'package:cine_mobile_spike/ui/theme/product_theme.dart';

import 'product_ui_test.dart' show TestGateway;

class TransferGateway extends TestGateway {
  int free = 1024 * 1024 * 1024;
  @override
  Future<Map<String, dynamic>?> transferDestination() async => {
    'path': 'fixture',
    'available_bytes': free,
  };
}

Future<(ApplicationController, TransferGateway)> fixture(
  WidgetTester tester, {
  bool host = false,
  Size size = const Size(500, 900),
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final g = TransferGateway();
  object(g.network['presentation'])['member_id'] = host ? 'host' : 'friend';
  g.network['transfer'] = {
    'supported': true,
    'offer': {'transfer_id': 'transfer', 'size_bytes': 32 * 1048576},
    'receivers': [],
    'progress': <String, dynamic>{},
  };
  final c = ApplicationController(gateway: g)..session = true;
  await c.initialize(startPolling: false);
  await tester.pumpWidget(
    MaterialApp(
      theme: productTheme(),
      home: Scaffold(
        body: SingleChildScrollView(child: TransferCard(controller: c)),
      ),
    ),
  );
  addTearDown(c.dispose);
  return (c, g);
}

void main() {
  testWidgets(
    'completed transfer preserves an already validated playing copy',
    (tester) async {
      final (c, g) = await fixture(tester);
      g.sync['playing'] = true;
      object(g.network['transfer'])['progress'] = {'state': 'completed'};
      c.poll();
      await tester.pumpAndSettle();
      expect(g.intents, isNot(contains('load_transfer')));
      expect(c.view.sync['playing'], true);
    },
  );
  testWidgets(
    'host offline disables resume but local cancel remains available',
    (tester) async {
      final (c, g) = await fixture(tester);
      object((g.room['members'] as List).first)['connected'] = false;
      g.network['connected'] = false;
      object(g.network['transfer'])['progress'] = {'state': 'paused'};
      c.poll();
      await tester.pump();
      final resume = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, 'Reanudar transferencia'),
      );
      expect(resume.onPressed, isNull);
      expect(find.text('Reconectar a la sala'), findsOneWidget);
      await tester.tap(find.text('Cancelar y eliminar parcial'));
      await tester.pumpAndSettle();
      expect(g.intents, contains('transfer_control'));
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'explicit consent before request and insufficient space rejection',
    (tester) async {
      final (c, g) = await fixture(tester);
      expect(g.intents, isEmpty);
      await tester.tap(find.text('Recibir archivo del anfitrión'));
      await tester.pumpAndSettle();
      expect(find.text('Aceptar descarga'), findsOneWidget);
      expect(g.intents, isEmpty);
      await tester.tap(find.text('Ahora no'));
      await tester.pumpAndSettle();
      expect(g.intents, isEmpty);
      g.free = 100;
      await tester.tap(find.text('Recibir archivo del anfitrión'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Aceptar descarga'));
      await tester.pumpAndSettle();
      expect(c.error, 'TRANSFER_SPACE');
      expect(g.intents, isNot(contains('receive')));
      g.free = 1024 * 1024 * 1024;
      await c.receiveMedia();
      expect(g.intents, contains('receive'));
    },
  );
  testWidgets('progress connecting pause resume verify error cancel states', (
    tester,
  ) async {
    final (c, g) = await fixture(tester);
    final t = object(g.network['transfer']);
    for (final state in [
      'waiting_for_acceptance',
      'connecting',
      'transferring',
      'paused',
      'reconnecting',
      'verifying',
      'failed',
      'cancelled',
    ]) {
      t['progress'] = {
        'state': state,
        'verified_bytes': 16 * 1048576,
        'total_bytes': 32 * 1048576,
        'bytes_per_second': 2 * 1048576,
        'eta_seconds': 8,
        'error': state == 'failed' ? 'TRANSFER_Corrupt' : null,
      };
      c.poll();
      await tester.pump();
      expect(find.text(transferLabels[state]!), findsOneWidget);
      expect(find.textContaining('50.0%'), findsOneWidget);
      if (state == 'transferring') {
        expect(find.textContaining('estimado'), findsOneWidget);
        await tester.tap(find.text('Pausar transferencia'));
        await tester.pumpAndSettle();
        expect(g.intents, contains('transfer_control'));
      } else {
        expect(find.textContaining('estimado'), findsNothing);
      }
      if (state == 'paused') {
        await tester.tap(find.text('Reanudar transferencia'));
        await tester.pumpAndSettle();
      }
      if (state == 'verifying') {
        await tester.tap(find.text('Cancelar y eliminar parcial'));
        await tester.pumpAndSettle();
      }
      expect(tester.takeException(), isNull);
    }
  });
  testWidgets(
    '100 percent Verify cannot become Ready before local validation',
    (tester) async {
      final (c, g) = await fixture(tester);
      g.sync['ready'] = false;
      g.sync['room_ready'] = false;
      g.network['media'] = {'identity_match': false};
      g.snapshot['hash'] = {'state': 'running'};
      object(g.network['transfer'])['progress'] = {
        'state': 'verifying',
        'verified_bytes': 32 * 1048576,
        'total_bytes': 32 * 1048576,
      };
      c.poll();
      await tester.pump();
      expect(c.view.canReady, false);
      expect(find.text(transferLabels['completed']!), findsNothing);
      g.sync['ready'] = true;
      g.snapshot['hash'] = {'state': 'complete'};
      g.network['media'] = {'identity_match': true};
      c.poll();
      expect(c.view.canReady, true);
    },
  );
  testWidgets('unsupported server hides receive and explains WSS', (
    tester,
  ) async {
    final (c, g) = await fixture(tester);
    object(g.network['transfer'])['supported'] = false;
    c.poll();
    await tester.pump();
    expect(find.text('Recibir archivo del anfitrión'), findsNothing);
    expect(find.textContaining('WSS'), findsOneWidget);
  });
  for (final size in [const Size(360, 640), const Size(1280, 720)]) {
    testWidgets('host requests responsive $size', (tester) async {
      final (c, g) = await fixture(tester, host: true, size: size);
      object(g.network['transfer'])['receivers'] = [
        {
          'receiver_id': 'friend',
          'state': 'waiting_for_acceptance',
          'verified_bytes': 0,
        },
      ];
      c.poll();
      await tester.pump();
      expect(find.text('Autorizar'), findsOneWidget);
      await tester.tap(find.text('Autorizar'));
      await tester.pumpAndSettle();
      expect(g.intents, contains('transfer_control'));
      expect(tester.takeException(), isNull);
    });
  }
}
