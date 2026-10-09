import 'package:flutter/material.dart';

import '../../presentation/application_controller.dart';
import '../../presentation/view_state.dart';
import '../theme/product_theme.dart';

String transferSize(num bytes) => '${(bytes / 1048576).toStringAsFixed(1)} MiB';
const transferLabels = {
  'waiting_for_acceptance': 'Esperando autorización del anfitrión',
  'connecting': 'Conectando con el anfitrión',
  'transferring': 'Recibiendo archivo',
  'paused': 'Transferencia pausada',
  'reconnecting': 'Conexión interrumpida · puedes reanudar',
  'verifying': 'Verificando SHA-256 del archivo completo',
  'completed': 'Archivo completo · SHA-256 verificado',
  'cancelled': 'Transferencia cancelada',
  'failed': 'La transferencia falló',
};

class TransferCard extends StatelessWidget {
  final ApplicationController controller;
  const TransferCard({required this.controller, super.key});
  Future<void> _receive(BuildContext context, int total) async {
    final accepted = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Recibir película'),
        content: Text(
          'Se descargarán ${transferSize(total)}. Necesitas ese espacio y 64 MiB de margen. La transferencia consume datos de red. El archivo solo se cargará después de verificarlo.\n\nEn Android se guarda en el almacenamiento privado de Cine Virtual. Cancelar elimina la descarga parcial.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Ahora no'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Aceptar descarga'),
          ),
        ],
      ),
    );
    if (accepted == true) await controller.receiveMedia();
  }

  Future<void> _share(BuildContext context) async {
    final address = await controller.transferAddress();
    if (!context.mounted) return;
    final input = TextEditingController(text: address);
    bool authorized = false;
    final accepted = await showDialog<bool>(
      context: context,
      builder: (ctx) => StatefulBuilder(
        builder: (ctx, update) => AlertDialog(
          title: const Text('Compartir película en la LAN'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Text(
                  'Cada participante debe solicitar el archivo y tú autorizas su transferencia. Los dos dispositivos deben estar en una LAN que permita conexiones directas.',
                ),
                CheckboxListTile(
                  contentPadding: EdgeInsets.zero,
                  value: authorized,
                  onChanged: (v) => update(() => authorized = v == true),
                  title: const Text(
                    'Tengo autorización para distribuir este archivo',
                  ),
                ),
                ExpansionTile(
                  title: const Text('Dirección y puerto LAN'),
                  children: [
                    TextField(
                      controller: input,
                      decoration: const InputDecoration(
                        labelText: 'Dirección local:puerto',
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(ctx, false),
              child: const Text('Cancelar'),
            ),
            FilledButton(
              onPressed: authorized ? () => Navigator.pop(ctx, true) : null,
              child: const Text('Ofrecer archivo'),
            ),
          ],
        ),
      ),
    );
    if (accepted == true) await controller.shareMedia(input.text.trim());
    input.dispose();
  }

  @override
  Widget build(
    BuildContext context,
  ) => ValueListenableBuilder<Map<String, dynamic>>(
    valueListenable: controller.transfer,
    builder: (context, data, _) {
      final view = controller.view;
      final offer = object(data['offer']);
      final progress = object(data['progress']);
      final state = '${progress['state'] ?? ''}';
      final total = number(progress['total_bytes']);
      final verified = number(progress['verified_bytes']);
      final enabled = view.connected && !view.busy;
      final hostAvailable = view.members.any((m) => m.host && m.connected);
      final supported =
          data['supported'] == true && controller.gateway.supportsPlayer;
      if (!view.hasMedia) return const SizedBox.shrink();
      return Padding(
        padding: const EdgeInsets.only(top: CineTokens.lg),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Divider(),
            if (!supported)
              const Text(
                'Compartir archivos requiere una sala WSS segura y clientes Linux/Android compatibles.',
                style: TextStyle(color: CineTokens.muted),
              ),
            if (supported && view.isHost) ...[
              Text(
                offer.isEmpty
                    ? 'Película disponible para compartir'
                    : 'Película ofrecida en la LAN',
                style: Theme.of(context).textTheme.titleMedium,
              ),
              if (offer.isEmpty)
                OutlinedButton.icon(
                  onPressed: enabled && view.media['identity_match'] == true
                      ? () => _share(context)
                      : null,
                  icon: const Icon(Icons.share_outlined),
                  label: const Text('Compartir archivo'),
                ),
              if (offer.isNotEmpty) ...[
                Text(
                  '${transferSize(number(offer['size_bytes']))} · identidad verificada',
                ),
                TextButton(
                  onPressed: enabled
                      ? () => controller.transferAction('withdraw')
                      : null,
                  child: const Text('Retirar oferta'),
                ),
                for (final receiver in (data['receivers'] as List? ?? []).map(
                  object,
                ))
                  Padding(
                    padding: const EdgeInsets.only(top: CineTokens.sm),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          '${view.members.where((m) => m.id == receiver['receiver_id']).map((m) => m.name).firstOrNull ?? 'Participante'} · ${transferLabels[receiver['state']] ?? receiver['state']}',
                        ),
                        if (receiver['state'] == 'waiting_for_acceptance')
                          Wrap(
                            spacing: CineTokens.sm,
                            children: [
                              FilledButton(
                                onPressed: enabled
                                    ? () => controller.transferAction(
                                        'accept',
                                        '${receiver['receiver_id']}',
                                      )
                                    : null,
                                child: const Text('Autorizar'),
                              ),
                              TextButton(
                                onPressed: enabled
                                    ? () => controller.transferAction(
                                        'reject',
                                        '${receiver['receiver_id']}',
                                      )
                                    : null,
                                child: const Text('Rechazar'),
                              ),
                            ],
                          )
                        else
                          Text(
                            '${transferSize(number(receiver['verified_bytes']))} verificados',
                          ),
                      ],
                    ),
                  ),
              ],
            ],
            if (supported && !view.isHost && offer.isEmpty)
              const Text(
                'El anfitrión todavía no ha ofrecido un archivo para recibir.',
                style: TextStyle(color: CineTokens.muted),
              ),
            if (supported && !view.isHost && offer.isNotEmpty) ...[
              Text(
                'El anfitrión tiene este archivo',
                style: Theme.of(context).textTheme.titleMedium,
              ),
              Text(
                '${transferSize(number(offer['size_bytes']))} · LAN directa',
              ),
              if (!hostAvailable)
                const Text(
                  'El anfitrión está desconectado. Espera a que regrese.',
                ),
              if (state.isEmpty || ['cancelled', 'failed'].contains(state))
                FilledButton.icon(
                  onPressed: enabled && hostAvailable
                      ? () => _receive(context, number(offer['size_bytes']))
                      : null,
                  icon: const Icon(Icons.download_outlined),
                  label: const Text('Recibir archivo del anfitrión'),
                ),
            ],
            if (!view.isHost && state.isNotEmpty) ...[
              const SizedBox(height: CineTokens.sm),
              if (!view.connected)
                OutlinedButton(
                  onPressed: view.busy ? null : controller.reconnect,
                  child: const Text('Reconectar a la sala'),
                ),
              Text(transferLabels[state] ?? state),
              if (total > 0) ...[
                const SizedBox(height: CineTokens.sm),
                LinearProgressIndicator(
                  backgroundColor: CineTokens.border,
                  value: (verified / total).clamp(0.0, 1.0),
                ),
                Text(
                  '${transferSize(verified)} / ${transferSize(total)} verificados · ${(verified / total * 100).clamp(0, 100).toStringAsFixed(1)}%',
                ),
                if (state == 'transferring' &&
                    progress['bytes_per_second'] is num &&
                    (progress['bytes_per_second'] as num) > 0)
                  Text(
                    '${transferSize(progress['bytes_per_second'] as num)}/s${progress['eta_seconds'] is num ? ' · tiempo restante estimado: ${timeLabel(number(progress['eta_seconds']) * 1000)}' : ''}',
                  ),
              ],
              if (progress['error'] != null)
                Text(
                  state == 'cancelled'
                      ? 'No se pudo eliminar el parcial. Revisa el almacenamiento local.'
                      : 'No se pudo completar la transferencia. Revisa conexión, almacenamiento o solicita de nuevo el archivo.',
                  style: const TextStyle(color: CineTokens.error),
                ),
              Wrap(
                spacing: CineTokens.sm,
                runSpacing: CineTokens.sm,
                children: [
                  if (state == 'completed' &&
                      controller.error == 'TRANSFER_LOAD_FAILED')
                    FilledButton(
                      onPressed: enabled
                          ? controller.retryTransferredMedia
                          : null,
                      child: const Text('Reintentar carga del archivo'),
                    ),
                  if ([
                    'connecting',
                    'transferring',
                    'verifying',
                  ].contains(state))
                    OutlinedButton(
                      onPressed: enabled
                          ? () => controller.transferAction('pause')
                          : null,
                      child: const Text('Pausar transferencia'),
                    ),
                  if (['paused', 'reconnecting'].contains(state))
                    FilledButton(
                      onPressed: enabled && hostAvailable
                          ? () => controller.transferAction('resume')
                          : null,
                      child: const Text('Reanudar transferencia'),
                    ),
                  if (!['completed', 'cancelled'].contains(state))
                    TextButton(
                      onPressed: !view.busy
                          ? () => controller.transferAction('cancel')
                          : null,
                      child: const Text('Cancelar y eliminar parcial'),
                    ),
                ],
              ),
            ],
          ],
        ),
      );
    },
  );
}
