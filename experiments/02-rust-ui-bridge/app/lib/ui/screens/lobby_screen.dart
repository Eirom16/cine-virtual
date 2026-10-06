import 'package:flutter/material.dart';

import '../../presentation/application_controller.dart';
import '../../presentation/view_state.dart';
import '../components/product_components.dart';
import '../theme/product_theme.dart';

class LobbyScreen extends StatelessWidget {
  final ApplicationController controller;
  final VoidCallback onPlayer, onShare;
  const LobbyScreen({
    required this.controller,
    required this.onPlayer,
    required this.onShare,
    super.key,
  });
  @override
  Widget build(BuildContext context) {
    final view = controller.view;
    return LayoutBuilder(
      builder: (context, constraints) {
        final desktop = constraints.maxWidth >= CineTokens.desktop;
        return SingleChildScrollView(
          padding: CineTokens.pageInsets,
          child: Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(
                maxWidth: CineTokens.maxContent,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Wrap(
                    alignment: WrapAlignment.spaceBetween,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    spacing: CineTokens.lg,
                    runSpacing: CineTokens.sm,
                    children: [
                      Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Sala ${'${view.room['room_id'] ?? ''}'.split('-').first}',
                            style: Theme.of(context).textTheme.headlineMedium,
                          ),
                          const SizedBox(height: CineTokens.xs),
                          const Text(
                            'El punto de encuentro antes de la película.',
                            style: TextStyle(color: CineTokens.muted),
                          ),
                        ],
                      ),
                      OutlinedButton.icon(
                        onPressed: controller.invitation == null
                            ? null
                            : onShare,
                        icon: const Icon(Icons.copy_outlined),
                        label: const Text('Copiar invitación'),
                      ),
                    ],
                  ),
                  const SizedBox(height: CineTokens.xl),
                  Flex(
                    direction: desktop ? Axis.horizontal : Axis.vertical,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      if (desktop)
                        Expanded(flex: 2, child: _media(context, view))
                      else
                        SizedBox(
                          width: double.infinity,
                          child: _media(context, view),
                        ),
                      SizedBox(
                        width: desktop ? CineTokens.lg : 0,
                        height: desktop ? 0 : CineTokens.lg,
                      ),
                      if (desktop)
                        Expanded(
                          child: SectionCard(child: Participants(view.members)),
                        )
                      else
                        SizedBox(
                          width: double.infinity,
                          child: SectionCard(child: Participants(view.members)),
                        ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }

  Widget _media(BuildContext context, RoomView view) => SectionCard(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          width: double.infinity,
          padding: const EdgeInsets.symmetric(vertical: CineTokens.xl),
          decoration: BoxDecoration(
            color: CineTokens.background,
            borderRadius: BorderRadius.circular(CineTokens.controlRadius),
          ),
          child: Column(
            children: [
              const Icon(
                Icons.movie_outlined,
                color: CineTokens.accent,
                size: 48,
              ),
              const SizedBox(height: CineTokens.md),
              Text(
                view.hasMedia
                    ? 'Una película, juntos.'
                    : view.isHost
                    ? 'Elige qué vamos a ver.'
                    : 'Esperando al anfitrión.',
                style: Theme.of(context).textTheme.titleLarge,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: CineTokens.xs),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: CineTokens.md),
                child: Text(
                  view.hasMedia
                      ? 'Cada dispositivo reproduce su propio archivo.'
                      : view.isHost
                      ? 'Empieza con un archivo de tu dispositivo.'
                      : 'El anfitrión seleccionará la película de la sala.',
                  style: const TextStyle(color: CineTokens.muted),
                  textAlign: TextAlign.center,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: CineTokens.lg),
        if (view.filename.isNotEmpty) ...[
          Text(view.filename, style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: CineTokens.xs),
          Wrap(
            spacing: CineTokens.md,
            children: [
              if (number(view.media['duration_ms']) > 0)
                Text(
                  timeLabel(number(view.media['duration_ms'])),
                  style: const TextStyle(color: CineTokens.muted),
                ),
              if ((view.media['codecs'] as List? ?? []).isNotEmpty)
                Text(
                  (view.media['codecs'] as List).join(' · '),
                  style: const TextStyle(color: CineTokens.muted),
                ),
            ],
          ),
          const SizedBox(height: CineTokens.md),
        ],
        ReadinessStatus(
          view: view,
          progress: controller.hashProgress,
          onCancel: controller.gateway.android && view.hashState == 'running'
              ? controller.cancelHash
              : null,
        ),
        const SizedBox(height: CineTokens.lg),
        Wrap(
          spacing: CineTokens.sm,
          runSpacing: CineTokens.sm,
          children: [
            OutlinedButton.icon(
              onPressed: view.canSelect && controller.gateway.supportsPlayer
                  ? controller.selectMedia
                  : null,
              icon: const Icon(Icons.folder_open),
              label: Text(
                view.filename.isEmpty
                    ? 'Seleccionar archivo'
                    : 'Cambiar archivo',
              ),
            ),
            if (!view.roomReady)
              BusyButton(
                label: 'Estoy listo',
                icon: Icons.check,
                onPressed: view.canReady ? controller.ready : null,
                busy: view.busy && view.action == 'ready',
              ),
            if (view.isHost)
              BusyButton(
                label: 'Comenzar película',
                icon: Icons.play_arrow,
                busy: view.busy && view.action == 'play',
                onPressed: view.canPlay
                    ? () async {
                        if (await controller.control('play')) onPlayer();
                      }
                    : null,
              ),
            if (view.roomReady)
              TextButton.icon(
                onPressed: onPlayer,
                icon: const Icon(Icons.ondemand_video),
                label: const Text('Ir al reproductor'),
              ),
          ],
        ),
        if (!controller.gateway.supportsPlayer)
          const Padding(
            padding: EdgeInsets.only(top: CineTokens.md),
            child: Text(
              'Reproducción no disponible en esta plataforma. iOS no tiene runtime de vídeo; Windows y macOS aún no están validados.',
              style: TextStyle(color: CineTokens.warning),
            ),
          ),
        if (!view.isHost && view.roomReady)
          const Padding(
            padding: EdgeInsets.only(top: CineTokens.md),
            child: Text(
              'El anfitrión comenzará la reproducción.',
              style: TextStyle(color: CineTokens.muted),
            ),
          ),
      ],
    ),
  );
}
