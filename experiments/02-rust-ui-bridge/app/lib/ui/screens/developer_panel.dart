import 'dart:convert';

import 'package:flutter/material.dart';

import '../../presentation/application_controller.dart';
import '../theme/product_theme.dart';

class DeveloperPanel extends StatelessWidget {
  final ApplicationController controller;
  const DeveloperPanel(this.controller, {super.key});
  @override
  Widget build(BuildContext context) => Dialog(
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 760, maxHeight: 640),
      child: Padding(
        padding: CineTokens.pageInsets,
        child: Column(
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    'Developer · Diagnóstico',
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                ),
                IconButton(
                  onPressed: () => Navigator.pop(context),
                  tooltip: 'Cerrar diagnóstico',
                  icon: const Icon(Icons.close),
                ),
              ],
            ),
            const SizedBox(height: CineTokens.md),
            const Text(
              'Estado de Application · Solo lectura · Sin credenciales ni rutas locales',
              style: TextStyle(color: CineTokens.muted),
            ),
            const SizedBox(height: CineTokens.sm),
            Expanded(
              child: SingleChildScrollView(
                child: ValueListenableBuilder<Map<String, dynamic>>(
                  valueListenable: controller.diagnostics,
                  builder: (context, value, _) => SelectableText(
                    const JsonEncoder.withIndent('  ').convert(value),
                    style: const TextStyle(
                      fontFamily: 'monospace',
                      fontSize: 12,
                    ),
                  ),
                ),
              ),
            ),
            if (controller.session)
              Wrap(
                spacing: CineTokens.sm,
                children: [
                  TextButton(
                    onPressed: controller.busy
                        ? null
                        : controller.disconnectForDiagnostics,
                    child: const Text('Desconectar socket'),
                  ),
                  TextButton(
                    onPressed: controller.busy ? null : controller.reconnect,
                    child: const Text('Resume / snapshot'),
                  ),
                ],
              ),
          ],
        ),
      ),
    ),
  );
}
