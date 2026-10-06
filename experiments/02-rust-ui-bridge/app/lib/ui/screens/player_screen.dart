import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../presentation/application_controller.dart';
import '../../presentation/view_state.dart';
import '../components/product_components.dart';
import '../theme/product_theme.dart';

class PlayerScreen extends StatefulWidget {
  final ApplicationController controller;
  final VoidCallback onLobby;
  const PlayerScreen({
    required this.controller,
    required this.onLobby,
    super.key,
  });
  @override
  State<PlayerScreen> createState() => _PlayerScreenState();
}

class _PlayerScreenState extends State<PlayerScreen> {
  bool visible = true, participants = false;
  Timer? idle;
  void showControls() {
    idle?.cancel();
    if (!visible) setState(() => visible = true);
    idle = Timer(CineTokens.overlayIdle, () {
      if (mounted &&
          widget.controller.playback.value.playing &&
          !widget.controller.busy) {
        setState(() => visible = false);
      }
    });
  }

  @override
  void initState() {
    super.initState();
    showControls();
  }

  @override
  void dispose() {
    idle?.cancel();
    super.dispose();
  }

  void toggle() {
    final view = widget.controller.view;
    final playing = widget.controller.playback.value.playing;
    if (!(playing ? view.canControl : view.canPlay)) return;
    widget.controller.control(playing ? 'pause' : 'play');
    showControls();
  }

  void seekBy(int delta) {
    if (!widget.controller.view.canControl) return;
    final p = widget.controller.playback.value;
    widget.controller.control(
      'seek',
      (p.position + delta).clamp(0, p.duration),
    );
    showControls();
  }

  void showParticipants(bool desktop) {
    showControls();
    if (desktop) {
      setState(() => participants = !participants);
    } else {
      showModalBottomSheet<void>(
        context: context,
        useSafeArea: true,
        isScrollControlled: true,
        builder: (context) => AnimatedBuilder(
          animation: widget.controller,
          builder: (context, _) => SingleChildScrollView(
            padding: CineTokens.pageInsets,
            child: Participants(widget.controller.view.members),
          ),
        ),
      );
    }
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final desktop = constraints.maxWidth >= CineTokens.desktop;
      final controller = widget.controller;
      final view = controller.view;
      return CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.space): toggle,
          const SingleActivator(LogicalKeyboardKey.arrowLeft): () =>
              seekBy(-10000),
          const SingleActivator(LogicalKeyboardKey.arrowRight): () =>
              seekBy(10000),
          const SingleActivator(LogicalKeyboardKey.escape): () {
            setState(() {
              visible = true;
              participants = false;
            });
          },
        },
        child: Focus(
          autofocus: true,
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Expanded(
                child: Column(
                  children: [
                    Expanded(
                      child: MouseRegion(
                        onHover: (_) => showControls(),
                        child: GestureDetector(
                          onTap: () {
                            if (visible) {
                              idle?.cancel();
                              setState(() => visible = false);
                            } else {
                              showControls();
                            }
                          },
                          child: Stack(
                            fit: StackFit.expand,
                            children: [
                              ColoredBox(
                                color: Colors.black,
                                child:
                                    controller.gateway.android &&
                                        controller.gateway.supportsPlayer
                                    ? const AndroidView(
                                        key: ValueKey('media-surface'),
                                        viewType: 'cine.mobile/surface',
                                      )
                                    : Center(
                                        child: Padding(
                                          padding: CineTokens.pageInsets,
                                          child: Column(
                                            mainAxisSize: MainAxisSize.min,
                                            children: [
                                              Icon(
                                                controller
                                                        .gateway
                                                        .supportsPlayer
                                                    ? Icons.open_in_new
                                                    : Icons
                                                          .videocam_off_outlined,
                                                size: 44,
                                                color: CineTokens.muted,
                                              ),
                                              const SizedBox(
                                                height: CineTokens.md,
                                              ),
                                              Text(
                                                controller
                                                        .gateway
                                                        .supportsPlayer
                                                    ? 'Vídeo en la ventana del reproductor'
                                                    : 'Reproductor no disponible',
                                                textAlign: TextAlign.center,
                                                style: Theme.of(context)
                                                    .textTheme
                                                    .titleLarge,
                                              ),
                                              const SizedBox(
                                                height: CineTokens.xs,
                                              ),
                                              Text(
                                                controller
                                                        .gateway
                                                        .supportsPlayer
                                                    ? 'El vídeo se abre en una ventana aparte.\nLa reproducción sigue coordinada con la sala.'
                                                    : 'iOS: runtime NOT IMPLEMENTED.\nWindows/macOS: reproducción NOT TESTED.',
                                                textAlign: TextAlign.center,
                                                style: const TextStyle(
                                                  color: CineTokens.muted,
                                                ),
                                              ),
                                            ],
                                          ),
                                        ),
                                      ),
                              ),
                              Align(
                                alignment: Alignment.topCenter,
                                child: IgnorePointer(
                                  ignoring: !visible,
                                  child: AnimatedOpacity(
                                    opacity: visible ? 1 : 0,
                                    duration: CineTokens.motion,
                                    child: Container(
                                      color: CineTokens.background.withValues(
                                        alpha: .92,
                                      ),
                                      padding: const EdgeInsets.symmetric(
                                        horizontal: CineTokens.sm,
                                      ),
                                      child: Row(
                                        children: [
                                          IconButton(
                                            onPressed: widget.onLobby,
                                            tooltip: 'Volver a la sala',
                                            icon: const Icon(Icons.arrow_back),
                                          ),
                                          Expanded(
                                            child: Text(
                                              view.filename.isEmpty
                                                  ? 'Cine Virtual'
                                                  : view.filename,
                                              maxLines: 1,
                                              overflow: TextOverflow.ellipsis,
                                            ),
                                          ),
                                          IconButton(
                                            onPressed: () =>
                                                showParticipants(desktop),
                                            tooltip: 'Participantes',
                                            icon: const Icon(
                                              Icons.people_outline,
                                            ),
                                          ),
                                        ],
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                              if (controller.playback.value.buffering)
                                const Center(
                                  child: StatusPill(
                                    'Preparando vídeo…',
                                    color: CineTokens.warning,
                                    icon: Icons.hourglass_top,
                                  ),
                                ),
                              if (!visible)
                                Positioned(
                                  right: CineTokens.sm,
                                  bottom: CineTokens.sm,
                                  child: IconButton(
                                    onPressed: showControls,
                                    tooltip: 'Mostrar controles',
                                    icon: const Icon(
                                      Icons.tune,
                                      color: CineTokens.text,
                                    ),
                                  ),
                                ),
                            ],
                          ),
                        ),
                      ),
                    ),
                    // Controls occupy their own region so Android SurfaceView cannot obscure them.
                    // Keyboard focus keeps controls visible for accessible navigation.
                    Focus(
                      onFocusChange: (focused) {
                        if (focused) {
                          idle?.cancel();
                          if (!visible) setState(() => visible = true);
                        }
                      },
                      child: AnimatedSize(
                        duration: CineTokens.motion,
                        child: visible
                            ? PlayerControls(
                                controller: controller,
                                onInteraction: showControls,
                                onScrubbing: (active) {
                                  if (active) {
                                    idle?.cancel();
                                  } else {
                                    showControls();
                                  }
                                },
                              )
                            : const SizedBox.shrink(),
                      ),
                    ),
                  ],
                ),
              ),
              if (desktop && participants)
                SizedBox(
                  width: 300,
                  child: ColoredBox(
                    color: CineTokens.surface,
                    child: SingleChildScrollView(
                      padding: CineTokens.pageInsets,
                      child: Participants(view.members),
                    ),
                  ),
                ),
            ],
          ),
        ),
      );
    },
  );
}

class PlayerControls extends StatefulWidget {
  final ApplicationController controller;
  final VoidCallback onInteraction;
  final ValueChanged<bool>? onScrubbing;
  const PlayerControls({
    required this.controller,
    required this.onInteraction,
    this.onScrubbing,
    super.key,
  });
  @override
  State<PlayerControls> createState() => _PlayerControlsState();
}

class _PlayerControlsState extends State<PlayerControls> {
  double? scrubbing;
  @override
  Widget build(BuildContext context) => ValueListenableBuilder<PlaybackView>(
    valueListenable: widget.controller.playback,
    builder: (context, p, _) {
      final view = widget.controller.view;
      return Container(
        color: CineTokens.surface,
        padding: const EdgeInsets.symmetric(
          horizontal: CineTokens.md,
          vertical: CineTokens.xs,
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                SizedBox(
                  width: 64,
                  child: Text(
                    timeLabel((scrubbing ?? p.position.toDouble()).round()),
                    style: const TextStyle(
                      fontFeatures: [FontFeature.tabularFigures()],
                    ),
                  ),
                ),
                Expanded(
                  child: Semantics(
                    label: view.isHost
                        ? 'Posición de reproducción'
                        : 'Progreso; el anfitrión controla la reproducción',
                    child: Slider(
                      key: const Key('player-seek'),
                      min: 0,
                      max: p.duration > 0 ? p.duration.toDouble() : 1,
                      value: (scrubbing ?? p.position.toDouble()).clamp(
                        0,
                        p.duration > 0 ? p.duration.toDouble() : 1,
                      ),
                      label: timeLabel(
                        (scrubbing ?? p.position.toDouble()).round(),
                      ),
                      onChangeStart: view.canControl && p.duration > 0
                          ? (_) {
                              widget.onInteraction();
                              widget.onScrubbing?.call(true);
                            }
                          : null,
                      onChanged: view.canControl && p.duration > 0
                          ? (value) => setState(() => scrubbing = value)
                          : null,
                      onChangeEnd: view.canControl && p.duration > 0
                          ? (value) {
                              setState(() => scrubbing = null);
                              widget.onScrubbing?.call(false);
                              widget.controller.control('seek', value.round());
                            }
                          : null,
                    ),
                  ),
                ),
                SizedBox(
                  width: 64,
                  child: Text(
                    timeLabel(p.duration),
                    textAlign: TextAlign.right,
                  ),
                ),
              ],
            ),
            Row(
              children: [
                IconButton(
                  key: const Key('player-toggle'),
                  tooltip: p.playing ? 'Pausar' : 'Reproducir',
                  onPressed: (p.playing ? view.canControl : view.canPlay)
                      ? () {
                          widget.onInteraction();
                          widget.controller.control(
                            p.playing ? 'pause' : 'play',
                          );
                        }
                      : null,
                  icon: Icon(
                    p.playing ? Icons.pause : Icons.play_arrow,
                    size: 30,
                  ),
                ),
                const SizedBox(width: CineTokens.xs),
                Expanded(
                  child: Text(
                    view.isHost
                        ? 'Controlas la reproducción'
                        : 'El anfitrión controla la reproducción',
                    style: const TextStyle(
                      color: CineTokens.muted,
                      fontSize: 12,
                    ),
                  ),
                ),
                if (view.connected)
                  StatusPill(
                    view.sync['snapshot_required'] == true ||
                            !view.trusted ||
                            !view.roomReady ||
                            p.buffering
                        ? 'Sincronizando'
                        : 'Conexión lista',
                    color: view.trusted && view.roomReady && !p.buffering
                        ? CineTokens.healthy
                        : CineTokens.warning,
                    icon: Icons.sync,
                  ),
              ],
            ),
          ],
        ),
      );
    },
  );
}
