import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../../presentation/product_error.dart';
import '../../presentation/view_state.dart';
import '../theme/product_theme.dart';

class StatusPill extends StatelessWidget {
  final String label;
  final Color color;
  final IconData icon;
  const StatusPill(
    this.label, {
    this.color = CineTokens.muted,
    this.icon = Icons.circle,
    super.key,
  });
  @override
  Widget build(BuildContext context) => Semantics(
    label: label,
    child: Container(
      padding: const EdgeInsets.symmetric(
        horizontal: CineTokens.sm,
        vertical: CineTokens.xs,
      ),
      decoration: BoxDecoration(
        color: color.withValues(alpha: .08),
        borderRadius: BorderRadius.circular(CineTokens.controlRadius),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 12, color: color),
          const SizedBox(width: CineTokens.xs),
          Flexible(
            child: Text(
              label,
              style: TextStyle(
                color: color,
                fontSize: 12,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
        ],
      ),
    ),
  );
}

String connectionLabel(ConnectionStatus status) => switch (status) {
  ConnectionStatus.connected => 'Conectado',
  ConnectionStatus.connecting => 'Conectando…',
  ConnectionStatus.reconnecting => 'Reconectando…',
  ConnectionStatus.recovering => 'Recuperando…',
  ConnectionStatus.disconnected => 'Sin conexión',
};

class ConnectionPill extends StatelessWidget {
  final ConnectionStatus status;
  const ConnectionPill(this.status, {super.key});
  @override
  Widget build(BuildContext context) => StatusPill(
    connectionLabel(status),
    color: status == ConnectionStatus.connected
        ? CineTokens.muted
        : CineTokens.warning,
  );
}

class ProductErrorBanner extends StatelessWidget {
  final String code;
  final VoidCallback onDismiss;
  const ProductErrorBanner({
    required this.code,
    required this.onDismiss,
    super.key,
  });
  @override
  Widget build(BuildContext context) {
    final error = ProductError.from(code);
    return Semantics(
      liveRegion: true,
      child: Container(
        margin: const EdgeInsets.only(bottom: CineTokens.md),
        padding: const EdgeInsets.all(CineTokens.md),
        decoration: BoxDecoration(
          color: CineTokens.error.withValues(alpha: .08),
          border: Border.all(color: CineTokens.error.withValues(alpha: .35)),
          borderRadius: BorderRadius.circular(CineTokens.controlRadius),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Icon(Icons.error_outline, color: CineTokens.error),
            const SizedBox(width: CineTokens.sm),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    error.message,
                    style: const TextStyle(fontWeight: FontWeight.w600),
                  ),
                  Text(
                    error.help,
                    style: const TextStyle(color: CineTokens.muted),
                  ),
                ],
              ),
            ),
            IconButton(
              onPressed: onDismiss,
              tooltip: 'Cerrar aviso',
              icon: const Icon(Icons.close),
            ),
          ],
        ),
      ),
    );
  }
}

class BusyButton extends StatelessWidget {
  final String label;
  final bool busy;
  final VoidCallback? onPressed;
  final IconData icon;
  const BusyButton({
    required this.label,
    required this.onPressed,
    this.busy = false,
    this.icon = Icons.arrow_forward,
    super.key,
  });
  @override
  Widget build(BuildContext context) => FilledButton.icon(
    onPressed: busy ? null : onPressed,
    icon: busy
        ? const SizedBox(
            width: 18,
            height: 18,
            child: CircularProgressIndicator(strokeWidth: 2),
          )
        : Icon(icon, size: 20),
    label: Text(busy ? 'Un momento…' : label),
  );
}

class SectionCard extends StatelessWidget {
  final Widget child;
  const SectionCard({required this.child, super.key});
  @override
  Widget build(BuildContext context) => Card(
    child: Padding(padding: CineTokens.pageInsets, child: child),
  );
}

class Participants extends StatelessWidget {
  final List<MemberView> members;
  const Participants(this.members, {super.key});
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    mainAxisSize: MainAxisSize.min,
    children: [
      Text(
        'En la sala · ${members.length}',
        style: Theme.of(context).textTheme.titleLarge,
      ),
      const SizedBox(height: CineTokens.lg),
      for (final m in members)
        Padding(
          padding: const EdgeInsets.only(bottom: CineTokens.md),
          child: Row(
            children: [
              CircleAvatar(
                backgroundColor: CineTokens.elevated,
                foregroundColor: CineTokens.accent,
                child: Text(
                  m.name.isEmpty ? '?' : m.name.characters.first.toUpperCase(),
                ),
              ),
              const SizedBox(width: CineTokens.sm),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      '${m.name}${m.you ? ' · Tú' : ''}',
                      style: const TextStyle(fontWeight: FontWeight.w600),
                    ),
                    Text(
                      m.host ? 'Anfitrión' : 'Participante',
                      style: const TextStyle(
                        color: CineTokens.muted,
                        fontSize: 12,
                      ),
                    ),
                  ],
                ),
              ),
              Tooltip(
                message: !m.connected
                    ? 'Desconectado'
                    : m.ready
                    ? 'Listo'
                    : m.status == 'loading'
                    ? 'Preparando'
                    : 'No listo',
                child: Icon(
                  !m.connected
                      ? Icons.link_off
                      : m.ready
                      ? Icons.check_circle
                      : Icons.schedule,
                  color: !m.connected
                      ? CineTokens.warning
                      : m.ready
                      ? CineTokens.healthy
                      : CineTokens.muted,
                  size: 22,
                ),
              ),
            ],
          ),
        ),
      if (members.length == 1)
        const Text(
          'Comparte la invitación para ver la película juntos.',
          style: TextStyle(color: CineTokens.muted),
        ),
    ],
  );
}

class ReadinessStatus extends StatelessWidget {
  final RoomView view;
  final ValueListenable<double?> progress;
  final VoidCallback? onCancel;
  const ReadinessStatus({
    required this.view,
    required this.progress,
    this.onCancel,
    super.key,
  });
  @override
  Widget build(BuildContext context) {
    final (label, help, color, icon) = switch (view.readiness) {
      Readiness.noMedia => (
        'Falta tu archivo',
        view.isHost
            ? 'Selecciona una película de tu dispositivo.'
            : 'Selecciona la misma versión del archivo que el anfitrión.',
        CineTokens.muted,
        Icons.movie_outlined,
      ),
      Readiness.hashing => (
        'Verificando archivo…',
        'Comprobamos que todos tengan exactamente la misma película.',
        CineTokens.accent,
        Icons.fingerprint,
      ),
      Readiness.loading => (
        'Preparando reproductor…',
        'El archivo se está abriendo en tu dispositivo.',
        CineTokens.accent,
        Icons.hourglass_top,
      ),
      Readiness.mismatch => (
        'El archivo no coincide',
        'Selecciona la misma versión que está usando el anfitrión.',
        CineTokens.error,
        Icons.error_outline,
      ),
      Readiness.clock => (
        'Sincronizando conexión…',
        'Esperando una referencia de tiempo confiable.',
        CineTokens.warning,
        Icons.sync,
      ),
      Readiness.verifying => (
        'Confirmando preparación…',
        'La sala está verificando tu copia y tu estado.',
        CineTokens.accent,
        Icons.sync,
      ),
      Readiness.ready => (
        'Listo para ver',
        view.allReady
            ? 'Todos están listos. La película puede comenzar.'
            : 'Esperando a que los demás estén listos.',
        CineTokens.healthy,
        Icons.check_circle_outline,
      ),
      Readiness.waiting => (
        'Archivo preparado',
        'Confirma que estás listo para ver la película.',
        CineTokens.accent,
        Icons.task_alt,
      ),
      Readiness.cancelled => (
        'Verificación cancelada',
        'Selecciona el archivo para volver a verificarlo.',
        CineTokens.muted,
        Icons.pause_circle_outline,
      ),
      Readiness.error => (
        'No se pudo preparar',
        'Vuelve a seleccionar un archivo local válido.',
        CineTokens.error,
        Icons.error_outline,
      ),
    };
    return Semantics(
      liveRegion: true,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(icon, color: color),
              const SizedBox(width: CineTokens.sm),
              Expanded(
                child: Text(
                  label,
                  style: TextStyle(fontWeight: FontWeight.w600, color: color),
                ),
              ),
            ],
          ),
          const SizedBox(height: CineTokens.xs),
          Text(help, style: const TextStyle(color: CineTokens.muted)),
          if (view.readiness == Readiness.hashing)
            ValueListenableBuilder<double?>(
              valueListenable: progress,
              builder: (context, value, _) => Padding(
                padding: const EdgeInsets.only(top: CineTokens.md),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    LinearProgressIndicator(
                      backgroundColor: CineTokens.border,
                      value: value,
                      semanticsLabel: 'Verificación del archivo',
                    ),
                    const SizedBox(height: CineTokens.xs),
                    Text(
                      value == null
                          ? 'Leyendo archivo…'
                          : '${(value * 100).floor()}%',
                    ),
                    if (onCancel != null)
                      TextButton(
                        onPressed: onCancel,
                        child: const Text('Cancelar verificación'),
                      ),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }
}
