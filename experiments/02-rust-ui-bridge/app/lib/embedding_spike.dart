import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'ui/components/desktop_video.dart';

class EmbeddingSpike extends StatefulWidget {
  const EmbeddingSpike({super.key});
  @override
  State<EmbeddingSpike> createState() => _EmbeddingSpikeState();
}

class _EmbeddingSpikeState extends State<EmbeddingSpike> {
  static const channel = MethodChannel('cine.desktop/video');
  int? texture;
  String status = 'Initializing EGL texture';
  @override
  void initState() {
    super.initState();
    unawaited(start());
  }

  Future<void> start() async {
    final id = await channel.invokeMethod<int>('spike');
    if (!mounted) return;
    setState(() => texture = id);
    for (int i = 0; i < 500; i++) {
      final s = await channel.invokeMapMethod<String, dynamic>('status');
      if (s?['error'] != 0) {
        setState(() => status = 'Render error: $s');
        return;
      }
      if (s?['ready'] == true) break;
      await Future<void>.delayed(const Duration(milliseconds: 10));
    }
    await command(['loadfile', Platform.environment['CINE_EMBEDDING_SPIKE']!]);
    setState(() => status = 'Real libmpv / EGL / Flutter texture');
  }

  Future<void> command(List<String> command) =>
      channel.invokeMethod('command', command);
  @override
  void dispose() {
    unawaited(channel.invokeMethod<void>('dispose'));
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    body: Column(
      children: [
        Text(status),
        Expanded(
          child: Stack(
            fit: StackFit.expand,
            children: [
              if (texture != null) DesktopVideo(texture: texture!),
              const Align(
                alignment: Alignment.topRight,
                child: Card(
                  child: Padding(
                    padding: EdgeInsets.all(24),
                    child: Text('Flutter overlay over REAL VIDEO'),
                  ),
                ),
              ),
            ],
          ),
        ),
        Row(
          children: [
            TextButton(
              onPressed: () => command(['set', 'pause', 'no']),
              child: const Text('Play'),
            ),
            TextButton(
              onPressed: () => command(['set', 'pause', 'yes']),
              child: const Text('Pause'),
            ),
            TextButton(
              onPressed: () => command(['seek', '45', 'absolute+exact']),
              child: const Text('Seek 45s'),
            ),
            TextButton(
              onPressed: start,
              child: const Text('Destroy / recreate'),
            ),
          ],
        ),
      ],
    ),
  );
}
