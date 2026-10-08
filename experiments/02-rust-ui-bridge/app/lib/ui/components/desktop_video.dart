import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Presentation only: the texture and mpv owner survive this widget's lifetime.
class DesktopVideo extends StatefulWidget {
  final int texture;
  const DesktopVideo({required this.texture, super.key});
  @override
  State<DesktopVideo> createState() => _DesktopVideoState();
}

class _DesktopVideoState extends State<DesktopVideo> {
  static const channel = MethodChannel('cine.desktop/video');
  Size? size;
  Timer? timer;
  bool failed = false;
  @override
  void initState() {
    super.initState();
    timer = Timer.periodic(const Duration(seconds: 1), (_) => observe());
  }

  Future<void> observe() async {
    try {
      final state = await channel.invokeMapMethod<String, dynamic>('status');
      final error = state?['error'] != 0;
      if (mounted && failed != error) setState(() => failed = error);
    } on PlatformException {
      if (mounted && !failed) setState(() => failed = true);
    }
  }

  @override
  void dispose() {
    timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, bounds) {
      final next =
          Size(bounds.maxWidth, bounds.maxHeight) *
          MediaQuery.devicePixelRatioOf(context);
      if (size != next) {
        size = next;
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) {
            unawaited(
              channel.invokeMethod<void>('resize', {
                'width': next.width.round(),
                'height': next.height.round(),
              }),
            );
          }
        });
      }
      return failed
          ? Center(
              child: Semantics(
                liveRegion: true,
                child: const Text(
                  'No se pudo mostrar el vídeo.\nVuelve a la sala para revisar el reproductor.',
                  textAlign: TextAlign.center,
                ),
              ),
            )
          : Texture(textureId: widget.texture);
    },
  );
}
