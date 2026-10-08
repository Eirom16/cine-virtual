import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../presentation/application_controller.dart';
import '../../presentation/view_state.dart';
import '../components/product_components.dart';
import '../components/desktop_video.dart';
import '../components/social_panel.dart';
import '../theme/product_theme.dart';

class PlayerScreen extends StatefulWidget {
  final ApplicationController controller;
  final VoidCallback onLobby;
  final VoidCallback? onFullscreen;
  final bool fullscreen;
  const PlayerScreen({
    required this.controller,
    required this.onLobby,
    this.onFullscreen,
    this.fullscreen = false,
    super.key,
  });
  @override
  State<PlayerScreen> createState() => _PlayerScreenState();
}

class _PlayerScreenState extends State<PlayerScreen> {
  bool visible = true, participants = false, scrubbing = false, sheet = false;
  Timer? idle;
  void showControls() {
    idle?.cancel();
    if (!visible) setState(() => visible = true);
    idle = Timer(CineTokens.overlayIdle, () {
      if (mounted &&
          widget.controller.playback.value.playing &&
          !widget.controller.busy &&
          !scrubbing &&
          !participants &&
          !sheet &&
          widget.controller.view.error.isEmpty &&
          (widget.controller.gateway.videoDiagnostics['error'] ?? 0) == 0 &&
          (ModalRoute.of(context)?.isCurrent ?? true)) {
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

  Future<void> showParticipants(bool desktop) async {
    showControls();
    if (desktop) {
      setState(() => participants = !participants);
      return;
    }
    sheet = true;
    await showSocialSheet(context, widget.controller);
    sheet = false;
    if (mounted) showControls();
  }

  void escape() {
    if (participants) {
      setState(() => participants = false);
    } else if (widget.fullscreen) {
      widget.onFullscreen?.call();
    } else {
      setState(() {
        visible = true;
        participants = false;
      });
    }
  }

  Widget header(bool desktop) => Container(
    color: CineTokens.background.withValues(alpha: .92),
    padding: const EdgeInsets.symmetric(horizontal: CineTokens.sm),
    child: Row(
      children: [
        IconButton(
          onPressed: widget.onLobby,
          tooltip: 'Volver a la sala',
          icon: const Icon(Icons.arrow_back),
        ),
        Expanded(
          child: Text(
            widget.controller.view.filename.isEmpty
                ? 'Cine Virtual'
                : widget.controller.view.filename,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        ),
        ReactionPicker(controller: widget.controller),
        ChatButton(
          controller: widget.controller,
          onPressed: () => showParticipants(desktop),
        ),
      ],
    ),
  );
  Widget controls() => Focus(
    onFocusChange: (focused) {
      if (focused) {
        idle?.cancel();
        if (!visible) setState(() => visible = true);
      }
    },
    child: PlayerControls(
      controller: widget.controller,
      onInteraction: showControls,
      fullscreen: widget.fullscreen,
      onFullscreen: widget.onFullscreen,
      onScrubbing: (active) {
        scrubbing = active;
        if (active) {
          idle?.cancel();
        } else {
          showControls();
        }
      },
    ),
  );
  // Fade presentation only; hidden controls stop receiving input immediately.
  Widget overlay(Widget child) => IgnorePointer(
    ignoring: !visible,
    child: ExcludeSemantics(
      excluding: !visible,
      child: AnimatedSwitcher(
        duration: CineTokens.motion,
        child: visible ? child : const SizedBox.shrink(),
      ),
    ),
  );
  Widget video() {
    final controller = widget.controller;
    return ColoredBox(
      color: Colors.black,
      child: Stack(
        fit: StackFit.expand,
        children: [
          if (controller.gateway.android && controller.gateway.supportsPlayer)
            const AndroidView(
              key: ValueKey('media-surface'),
              viewType: 'cine.mobile/surface',
            )
          else if (controller.gateway.videoTexture != null)
            DesktopVideo(
              texture: controller.gateway.videoTexture!,
              onFailure: showControls,
            )
          else
            const Center(child: Text('Reproductor no disponible')),
          ReactionOverlay(controller: controller),
          ValueListenableBuilder<PlaybackView>(
            valueListenable: controller.playback,
            builder: (context, p, _) {
              if (controller.view.sync['failed'] == true) {
                return const ColoredBox(
                  color: Colors.black,
                  child: Center(child: Text('No se pudo mostrar el vídeo.')),
                );
              }
              if (controller.view.picking ||
                  controller.view.media['local_loaded'] == false) {
                return const ColoredBox(
                  color: Colors.black,
                  child: Center(
                    child: StatusPill(
                      'Preparando vídeo…',
                      color: CineTokens.warning,
                      icon: Icons.hourglass_top,
                    ),
                  ),
                );
              }
              return p.buffering
                  ? const Center(
                      child: StatusPill(
                        'Cargando…',
                        color: CineTokens.warning,
                        icon: Icons.hourglass_top,
                      ),
                    )
                  : const SizedBox.shrink();
            },
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, bounds) {
      final desktop = bounds.maxWidth >= CineTokens.desktop;
      // Android keeps reliable touch regions outside its Media3 SurfaceView.
      final composite =
          !widget.controller.gateway.android &&
          widget.controller.gateway.videoTexture != null;
      final surface = MouseRegion(
        onHover: (_) => showControls(),
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onDoubleTap: composite ? widget.onFullscreen : null,
          onTap: () {
            if (visible && !scrubbing) {
              idle?.cancel();
              setState(() => visible = false);
            } else {
              showControls();
            }
          },
          child: video(),
        ),
      );
      return CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.space): toggle,
          const SingleActivator(LogicalKeyboardKey.arrowLeft): () =>
              seekBy(-10000),
          const SingleActivator(LogicalKeyboardKey.arrowRight): () =>
              seekBy(10000),
          const SingleActivator(LogicalKeyboardKey.keyF): () {
            widget.onFullscreen?.call();
            showControls();
          },
          const SingleActivator(LogicalKeyboardKey.escape): escape,
        },
        child: Focus(
          autofocus: true,
          child: Stack(
            children: [
              Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Expanded(
                    child: composite
                        ? Stack(
                            fit: StackFit.expand,
                            children: [
                              surface,
                              Positioned(
                                top: 0,
                                left: 0,
                                right: 0,
                                child: overlay(header(desktop)),
                              ),
                              Positioned(
                                bottom: 0,
                                left: 0,
                                right: 0,
                                child: overlay(controls()),
                              ),
                            ],
                          )
                        : Column(
                            children: [
                              if (visible) header(desktop),
                              Expanded(
                                key: const ValueKey('player-video-region'),
                                child: surface,
                              ),
                              if (visible)
                                controls()
                              else
                                Align(
                                  alignment: Alignment.centerRight,
                                  child: IconButton(
                                    onPressed: showControls,
                                    tooltip: 'Mostrar controles',
                                    icon: const Icon(Icons.tune),
                                  ),
                                ),
                            ],
                          ),
                  ),
                  if (desktop && participants && !widget.fullscreen)
                    SizedBox(
                      width: 300,
                      child: SocialPanel(
                        controller: widget.controller,
                        onClose: () => setState(() => participants = false),
                      ),
                    ),
                ],
              ),
              if (desktop && participants && widget.fullscreen)
                Positioned(
                  top: 48,
                  bottom: 110,
                  right: 12,
                  width: 320,
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(CineTokens.radius),
                    child: SocialPanel(
                      controller: widget.controller,
                      onClose: () => setState(() => participants = false),
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
  final VoidCallback? onFullscreen;
  final bool fullscreen;
  const PlayerControls({
    required this.controller,
    required this.onInteraction,
    this.onScrubbing,
    this.onFullscreen,
    this.fullscreen = false,
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
                IconButton(
                  key: const Key('player-fullscreen'),
                  onPressed: widget.onFullscreen,
                  tooltip: widget.fullscreen
                      ? 'Salir de pantalla completa'
                      : 'Pantalla completa',
                  icon: Icon(
                    widget.fullscreen
                        ? Icons.fullscreen_exit
                        : Icons.fullscreen,
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
