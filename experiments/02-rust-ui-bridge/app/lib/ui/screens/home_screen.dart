import 'package:flutter/material.dart';

import '../theme/product_theme.dart';

class HomeScreen extends StatelessWidget {
  final VoidCallback onCreate, onJoin;
  const HomeScreen({required this.onCreate, required this.onJoin, super.key});
  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final desktop = constraints.maxWidth >= CineTokens.desktop;
      return SingleChildScrollView(
        child: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 960),
            child: Padding(
              padding: EdgeInsets.symmetric(
                horizontal: desktop ? CineTokens.xxl : CineTokens.lg,
                vertical: desktop ? 80 : CineTokens.md,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  if (desktop) ...[
                    const Icon(
                      Icons.theaters_outlined,
                      size: 40,
                      color: CineTokens.accent,
                    ),
                    const SizedBox(height: CineTokens.xl),
                  ],
                  const Text(
                    'TU PELÍCULA. SU COMPAÑÍA.',
                    style: TextStyle(
                      color: CineTokens.accent,
                      letterSpacing: 2,
                      fontSize: 11,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: CineTokens.md),
                  Text(
                    'El cine se disfruta\nmejor juntos.',
                    style: desktop
                        ? Theme.of(context).textTheme.headlineLarge
                        : Theme.of(context).textTheme.headlineMedium,
                  ),
                  const SizedBox(height: CineTokens.md),
                  Text(
                    desktop
                        ? 'Una sala compartida. El mismo archivo en cada dispositivo.\nReproducción coordinada, estés donde estés.'
                        : 'El mismo archivo en cada dispositivo.\nUna sala para verlo juntos.',
                    style: TextStyle(
                      color: CineTokens.muted,
                      fontSize: 16,
                      height: 1.6,
                    ),
                  ),
                  SizedBox(height: desktop ? CineTokens.xxl : CineTokens.lg),
                  Flex(
                    direction: desktop ? Axis.horizontal : Axis.vertical,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      if (desktop)
                        Expanded(child: _action(context, true))
                      else
                        _action(context, true, compact: true),
                      SizedBox(
                        width: desktop ? CineTokens.md : 0,
                        height: desktop ? 0 : CineTokens.md,
                      ),
                      if (desktop)
                        Expanded(child: _action(context, false))
                      else
                        _action(context, false, compact: true),
                    ],
                  ),
                  const SizedBox(height: CineTokens.xl),
                  const Row(
                    children: [
                      Icon(
                        Icons.folder_outlined,
                        size: 16,
                        color: CineTokens.muted,
                      ),
                      SizedBox(width: CineTokens.xs),
                      Expanded(
                        child: Text(
                          'Usa tu copia local o recibe el archivo del anfitrión en una LAN compatible.',
                          style: TextStyle(
                            color: CineTokens.muted,
                            fontSize: 12,
                          ),
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ),
      );
    },
  );
  Widget _action(BuildContext context, bool create, {bool compact = false}) {
    final content = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          create ? 'Crear sala' : 'Unirse a una sala',
          style: Theme.of(context).textTheme.titleLarge,
        ),
        const SizedBox(height: CineTokens.xs),
        Text(
          create
              ? 'Elige la película e invita a los demás.'
              : 'Entra con la invitación de tu anfitrión.',
          style: const TextStyle(color: CineTokens.muted),
        ),
      ],
    );
    final icon = Icon(
      create ? Icons.add : Icons.arrow_forward,
      color: CineTokens.accent,
    );
    return SizedBox(
      width: double.infinity,
      child: Card(
        child: InkWell(
          borderRadius: BorderRadius.circular(CineTokens.radius),
          onTap: create ? onCreate : onJoin,
          child: Padding(
            padding: EdgeInsets.all(compact ? CineTokens.md : CineTokens.lg),
            child: compact
                ? Row(
                    children: [
                      icon,
                      const SizedBox(width: CineTokens.md),
                      Expanded(child: content),
                    ],
                  )
                : Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      icon,
                      const SizedBox(height: CineTokens.md),
                      content,
                    ],
                  ),
          ),
        ),
      ),
    );
  }
}
