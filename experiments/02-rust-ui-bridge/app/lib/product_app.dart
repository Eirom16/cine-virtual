import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'presentation/application_controller.dart';
import 'presentation/view_state.dart';
import 'ui/components/product_components.dart';
import 'ui/screens/developer_panel.dart';
import 'ui/screens/entry_screen.dart';
import 'ui/screens/home_screen.dart';
import 'ui/screens/lobby_screen.dart';
import 'ui/screens/player_screen.dart';
import 'ui/theme/product_theme.dart';

enum ProductPage { home, create, join, lobby, player }

class CineVirtualApp extends StatelessWidget {
  final ApplicationController? controller;
  const CineVirtualApp({this.controller, super.key});
  @override
  Widget build(BuildContext context) => MaterialApp(
    title: 'Cine Virtual',
    debugShowCheckedModeBanner: false,
    theme: productTheme(),
    home: ProductShell(controller: controller),
  );
}

class ProductShell extends StatefulWidget {
  final ApplicationController? controller;
  const ProductShell({this.controller, super.key});
  @override
  State<ProductShell> createState() => _ProductShellState();
}

class _ProductShellState extends State<ProductShell> {
  late final ApplicationController controller;
  ProductPage page = ProductPage.home;
  bool routedPlayback = false, fullscreen = false, changingFullscreen = false;
  @override
  void initState() {
    super.initState();
    controller = widget.controller ?? ApplicationController();
    controller.addListener(observe);
    if (widget.controller == null) unawaited(initialize());
  }

  Timer? qaTimer;
  Future<void> initialize() async {
    await controller.initialize();
    if (!const bool.fromEnvironment('P2P_QA') || !mounted) return;
    if (Platform.isAndroid) {
      final config =
          await NativeSessionGateway.native.invokeMapMethod<String, dynamic>(
            'roomConfig',
          ) ??
          {};
      if ((config['host'] == true || '${config['invite'] ?? ''}'.isNotEmpty) &&
          await controller.enter(
            create: config['host'] == true,
            name: 'Android QA',
            server: '${config['server']}',
            invite: '${config['invite']}',
          )) {
        if (mounted) setState(() => page = ProductPage.lobby);
        if (config['host'] == true) {
          await NativeSessionGateway.native.invokeMethod(
            'qaSaveInvitation',
            controller.invitation,
          );
        }
      }
    } else if (Platform.isLinux &&
        Platform.environment.containsKey('CINE_P2P_QA_SERVER')) {
      if (await controller.enter(
        create: true,
        name: 'Linux QA',
        server: Platform.environment['CINE_P2P_QA_SERVER']!,
      )) {
        if (mounted) setState(() => page = ProductPage.lobby);
        final output = Platform.environment['CINE_P2P_QA_INVITATION_FILE'];
        if (output != null) {
          // Harness supplies an existing mode-0600 file in a private directory.
          await File(output).writeAsString(controller.invitation!);
        }
      }
    }
    final elapsed = Stopwatch()..start();
    qaTimer = Timer.periodic(const Duration(milliseconds: 500), (_) async {
      if (!mounted) return;
      final resources = Platform.isAndroid
          ? await NativeSessionGateway.native.invokeMapMethod<String, dynamic>(
              'qaResources',
            )
          : null;
      final v = controller.view;
      debugPrint(
        'CINE_P2P_SAMPLE ${jsonEncode({'elapsed_ms': elapsed.elapsedMilliseconds, 'resources': resources, 'connected': v.connected, 'trusted': v.trusted, 'host': v.isHost, 'room_ready': v.roomReady, 'player_ready': v.sync['ready'], 'playing': v.sync['playing'], 'position_ms': v.sync['position_ms'], 'drift_ms': v.sync['drift_ms'], 'snapshot_required': v.sync['snapshot_required'], 'identity_match': v.media['identity_match'], 'hash_state': v.hashState, 'error': controller.error, 'busy': controller.busy, 'action': controller.action, 'page': page.name, 'social_count': controller.social.value.entries.length, 'reaction_count': controller.social.value.reactions.length, 'transfer': controller.transfer.value})}',
      );
    });
  }

  void observe() {
    if (!mounted) return;
    // Navigation follows real local playback; no synthetic start-room command.
    final playing = controller.playback.value.playing;
    if (controller.session &&
        page == ProductPage.lobby &&
        playing &&
        !routedPlayback &&
        controller.view.roomReady) {
      page = ProductPage.player;
      routedPlayback = true;
    }
    if (!playing) routedPlayback = false;
    setState(() {});
  }

  void navigate(ProductPage next) {
    if ((next == ProductPage.lobby || next == ProductPage.player) &&
        !controller.session) {
      return;
    }
    setState(() => page = next);
  }

  Future<void> toggleFullscreen() async {
    if (changingFullscreen) return;
    changingFullscreen = true;
    final enabled = !fullscreen;
    try {
      await controller.gateway.fullscreen(enabled);
      if (mounted) setState(() => fullscreen = enabled);
    } on PlatformException {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('No se pudo cambiar la pantalla completa.'),
          ),
        );
      }
    } finally {
      changingFullscreen = false;
    }
  }

  void showLobby() {
    if (fullscreen) {
      unawaited(controller.gateway.fullscreen(false));
      fullscreen = false;
    }
    routedPlayback = true;
    navigate(ProductPage.lobby);
  }

  Future<void> share() async {
    final invitation = controller.invitation;
    if (invitation == null) return;
    await Clipboard.setData(ClipboardData(text: invitation));
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'Invitación copiada. Compártela solo con quienes entrarán a la sala.',
          ),
        ),
      );
    }
  }

  Future<void> exitRoom() async {
    final leave = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(
          controller.view.isHost ? '¿Cerrar la sala?' : '¿Salir de la sala?',
        ),
        content: Text(
          controller.view.isHost
              ? 'Eres el anfitrión. Al salir, la sala se cerrará para todos.'
              : 'Puedes volver con la invitación si la sala sigue disponible.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancelar'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Salir'),
          ),
        ],
      ),
    );
    if (leave != true || !mounted) return;
    if (await controller.leave() && mounted) navigate(ProductPage.home);
  }

  @override
  void dispose() {
    qaTimer?.cancel();
    controller.removeListener(observe);
    if (fullscreen) unawaited(controller.gateway.fullscreen(false));
    if (widget.controller == null) controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final view = controller.view;
    final body = switch (page) {
      ProductPage.home => HomeScreen(
        onCreate: () => navigate(ProductPage.create),
        onJoin: () => navigate(ProductPage.join),
      ),
      ProductPage.create || ProductPage.join => EntryScreen(
        key: ValueKey(page),
        controller: controller,
        create: page == ProductPage.create,
        onEntered: () => navigate(ProductPage.lobby),
      ),
      ProductPage.lobby => LobbyScreen(
        controller: controller,
        onPlayer: () => navigate(ProductPage.player),
        onShare: share,
      ),
      ProductPage.player => PlayerScreen(
        controller: controller,
        onLobby: showLobby,
        fullscreen: fullscreen,
        onFullscreen: toggleFullscreen,
      ),
    };
    return PopScope(
      canPop: page == ProductPage.home,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) {
          if (page == ProductPage.player && fullscreen) {
            toggleFullscreen();
          } else if (page == ProductPage.player) {
            showLobby();
          } else if (controller.session) {
            exitRoom();
          } else if (!controller.busy) {
            navigate(ProductPage.home);
          }
        }
      },
      child: Scaffold(
        resizeToAvoidBottomInset: page != ProductPage.player,
        appBar:
            page == ProductPage.player &&
                (fullscreen ||
                    MediaQuery.sizeOf(context).width >
                        MediaQuery.sizeOf(context).height)
            ? null
            : AppBar(
                leading: page == ProductPage.home
                    ? const Padding(
                        padding: EdgeInsets.only(left: CineTokens.md),
                        child: Icon(
                          Icons.theaters_outlined,
                          color: CineTokens.accent,
                        ),
                      )
                    : IconButton(
                        onPressed: controller.busy
                            ? null
                            : () {
                                if (page == ProductPage.player) {
                                  showLobby();
                                } else if (controller.session) {
                                  exitRoom();
                                } else {
                                  navigate(ProductPage.home);
                                }
                              },
                        tooltip: page == ProductPage.player
                            ? 'Volver a la sala'
                            : 'Volver',
                        icon: const Icon(Icons.arrow_back),
                      ),
                title: const Text(
                  'Cine Virtual',
                  style: TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
                ),
                actions: [
                  if (controller.session)
                    Padding(
                      padding: const EdgeInsets.symmetric(
                        vertical: CineTokens.xs,
                      ),
                      child: ConnectionPill(view.connection),
                    ),
                  PopupMenuButton<String>(
                    tooltip: 'Opciones',
                    onSelected: (value) {
                      if (value == 'developer') {
                        showDialog<void>(
                          context: context,
                          builder: (_) => DeveloperPanel(controller),
                        );
                      }
                      if (value == 'leave') exitRoom();
                    },
                    itemBuilder: (_) => [
                      const PopupMenuItem(
                        value: 'developer',
                        child: Text('Developer · Diagnóstico'),
                      ),
                      if (controller.session)
                        const PopupMenuItem(
                          value: 'leave',
                          child: Text('Salir de la sala'),
                        ),
                    ],
                  ),
                ],
              ),
        body: Stack(
          children: [
            SafeArea(
              child: Column(
                children: [
                  if (view.error.isNotEmpty)
                    Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: CineTokens.md,
                      ),
                      child: ProductErrorBanner(
                        code: view.error,
                        onDismiss: controller.dismissError,
                      ),
                    ),
                  if (controller.session &&
                      view.connection != ConnectionStatus.connected)
                    Container(
                      width: double.infinity,
                      color: CineTokens.warning.withValues(alpha: .08),
                      padding: const EdgeInsets.symmetric(
                        horizontal: CineTokens.md,
                        vertical: CineTokens.xs,
                      ),
                      child: Row(
                        children: [
                          Expanded(
                            child: Text(
                              view.connection == ConnectionStatus.disconnected
                                  ? 'Se perdió la conexión. Tu sala permanece aquí.'
                                  : connectionLabel(view.connection),
                              style: const TextStyle(color: CineTokens.warning),
                            ),
                          ),
                          if (!controller.busy && !controller.recovering)
                            TextButton(
                              onPressed: controller.reconnect,
                              child: const Text('Reintentar'),
                            ),
                        ],
                      ),
                    ),
                  Expanded(child: body),
                ],
              ),
            ),
            // Bootstrap EGL before load; Player then uses the persistent texture.
            if (page != ProductPage.player &&
                controller.gateway.videoTexture != null)
              Positioned(
                left: 0,
                top: 0,
                width: 1,
                height: 1,
                child: IgnorePointer(
                  child: Texture(textureId: controller.gateway.videoTexture!),
                ),
              ),
          ],
        ),
      ),
    );
  }
}
