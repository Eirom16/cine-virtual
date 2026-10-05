import 'package:flutter/material.dart';

import 'bridge.dart';

/// Apple compile gate only: bridge availability never implies a working Player.
class UnsupportedPlayerScreen extends StatefulWidget {
  const UnsupportedPlayerScreen({super.key});
  @override
  State<UnsupportedPlayerScreen> createState() =>
      _UnsupportedPlayerScreenState();
}

class _UnsupportedPlayerScreenState extends State<UnsupportedPlayerScreen> {
  CineBridge? bridge;
  String status = 'BRIDGE_NOT_TESTED';
  @override
  void initState() {
    super.initState();
    try {
      bridge = CineBridge();
      bridge!.call('state');
      status = 'Rust Application connected';
    } catch (_) {
      status = 'BRIDGE_UNAVAILABLE';
    }
  }

  @override
  void dispose() {
    bridge?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('Cine Virtual compile validation')),
    body: Column(
      children: [
        Text(status),
        const Text('IOS PLAYER RUNTIME NOT IMPLEMENTED'),
        const Text('AVPlayer is a candidate. This build does not play video.'),
      ],
    ),
  );
}
