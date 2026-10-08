import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../presentation/application_controller.dart';
import '../../presentation/social_view.dart';
import '../../presentation/view_state.dart';
import '../theme/product_theme.dart';
import 'product_components.dart';

Future<void> showSocialSheet(
  BuildContext context,
  ApplicationController controller,
) => showModalBottomSheet<void>(
  context: context,
  useSafeArea: true,
  isScrollControlled: true,
  builder: (context) {
    final media = MediaQuery.of(context);
    final available =
        (media.size.height - media.viewInsets.bottom - media.padding.top).clamp(
          0.0,
          media.size.height,
        );
    return Padding(
      padding: EdgeInsets.only(bottom: media.viewInsets.bottom),
      child: SizedBox(
        height: available < media.size.height * .78
            ? available
            : media.size.height * .78,
        child: SocialPanel(
          controller: controller,
          onClose: () => Navigator.pop(context),
        ),
      ),
    );
  },
);

class ChatButton extends StatelessWidget {
  final ApplicationController controller;
  final VoidCallback onPressed;
  const ChatButton({
    required this.controller,
    required this.onPressed,
    super.key,
  });
  @override
  Widget build(BuildContext context) => ValueListenableBuilder<SocialView>(
    valueListenable: controller.social,
    builder: (context, social, _) => IconButton(
      key: const Key('open-chat'),
      tooltip: social.unread == 0 ? 'Chat' : 'Chat · ${social.unread} sin leer',
      onPressed: onPressed,
      icon: Badge(
        isLabelVisible: social.unread > 0,
        label: Text('${social.unread}'),
        child: const Icon(Icons.chat_bubble_outline),
      ),
    ),
  );
}

class ReactionPicker extends StatelessWidget {
  final ApplicationController controller;
  const ReactionPicker({required this.controller, super.key});
  @override
  Widget build(BuildContext context) => ValueListenableBuilder<SocialView>(
    valueListenable: controller.social,
    builder: (context, social, _) => PopupMenuButton<String>(
      key: const Key('reaction-picker'),
      tooltip: 'Reaccionar',
      enabled: social.canSend,
      icon: const Icon(Icons.add_reaction_outlined),
      onSelected: (emoji) async {
        if (!await controller.sendReaction(emoji) && context.mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text(controller.social.value.errorText)),
          );
        }
      },
      itemBuilder: (_) => [
        for (int i = 0; i < socialEmojis.length; i++)
          PopupMenuItem(
            value: socialEmojis[i],
            child: Semantics(
              label: 'Reaccionar con ${socialLabels[i]}',
              excludeSemantics: true,
              child: Text('${socialEmojis[i]}  ${socialLabels[i]}'),
            ),
          ),
      ],
    ),
  );
}

class SocialPanel extends StatefulWidget {
  final ApplicationController controller;
  final VoidCallback? onClose;
  const SocialPanel({required this.controller, this.onClose, super.key});
  @override
  State<SocialPanel> createState() => _SocialPanelState();
}

class _SocialPanelState extends State<SocialPanel> {
  bool people = false, opened = false;
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && !people) {
        opened = true;
        widget.controller.socialOpened();
      }
    });
  }

  @override
  void dispose() {
    if (opened) widget.controller.socialClosed();
    super.dispose();
  }

  void switchTab(bool value) {
    if (people == value) return;
    if (value && opened) {
      widget.controller.socialClosed();
      opened = false;
    }
    if (!value && !opened) {
      opened = true;
      widget.controller.socialOpened();
    }
    setState(() => people = value);
  }

  @override
  Widget build(BuildContext context) => ColoredBox(
    color: CineTokens.surface,
    child: LayoutBuilder(
      builder: (context, bounds) => Column(
        children: [
          if (bounds.maxHeight >= 180)
            Row(
              children: [
                Expanded(
                  child: TextButton(
                    onPressed: () => switchTab(false),
                    child: Text(
                      'Chat',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: people ? CineTokens.muted : CineTokens.accent,
                      ),
                    ),
                  ),
                ),
                Expanded(
                  flex: 2,
                  child: TextButton(
                    onPressed: () => switchTab(true),
                    child: Text(
                      'Participantes',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: people ? CineTokens.accent : CineTokens.muted,
                      ),
                    ),
                  ),
                ),
                if (widget.onClose != null)
                  IconButton(
                    onPressed: widget.onClose,
                    tooltip: 'Cerrar chat',
                    icon: const Icon(Icons.close),
                  ),
              ],
            ),
          const Divider(height: 1),
          Expanded(
            child: people
                ? AnimatedBuilder(
                    animation: widget.controller,
                    builder: (_, _) => SingleChildScrollView(
                      padding: const EdgeInsets.all(CineTokens.md),
                      child: Participants(widget.controller.view.members),
                    ),
                  )
                : ChatConversation(controller: widget.controller),
          ),
        ],
      ),
    ),
  );
}

class ChatConversation extends StatefulWidget {
  final ApplicationController controller;
  const ChatConversation({required this.controller, super.key});
  @override
  State<ChatConversation> createState() => _ChatConversationState();
}

class _ChatConversationState extends State<ChatConversation> {
  final scroll = ScrollController();
  final focus = FocusNode();
  late final TextEditingController input;
  String lastId = '';
  bool newMessages = false;
  ApplicationController get controller => widget.controller;
  @override
  void initState() {
    super.initState();
    input = TextEditingController(text: controller.chatDraft);
    controller.social.addListener(changed);
    WidgetsBinding.instance.addPostFrameCallback((_) => toBottom());
  }

  void changed() {
    final entries = controller.social.value.entries;
    final id = entries.isEmpty ? '' : '${entries.last['message_id']}';
    if (id == lastId) return;
    lastId = id;
    final atBottom = !scroll.hasClients || scroll.position.extentAfter < 72;
    if (atBottom) {
      WidgetsBinding.instance.addPostFrameCallback((_) => toBottom());
    } else if (mounted) {
      setState(() => newMessages = true);
    }
  }

  void toBottom() {
    if (!mounted || !scroll.hasClients) return;
    scroll.jumpTo(scroll.position.maxScrollExtent);
    if (newMessages) setState(() => newMessages = false);
  }

  Future<void> send() async {
    final text = input.text;
    if (text.trim().isEmpty || !controller.social.value.canSend) return;
    final sent = await controller.sendChat(text);
    if (sent && mounted && input.text == text) {
      input.clear();
      controller.chatDraft = '';
      setState(() {});
    }
    if (mounted && !Platform.isAndroid && !Platform.isIOS) focus.requestFocus();
  }

  @override
  void dispose() {
    controller.social.removeListener(changed);
    input.dispose();
    focus.dispose();
    scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<SocialView>(
    valueListenable: controller.social,
    builder: (context, social, _) => Column(
      children: [
        Expanded(
          child: Stack(
            children: [
              if (social.entries.isEmpty)
                const Center(
                  child: Text(
                    'La conversación empieza aquí.',
                    style: TextStyle(color: CineTokens.muted),
                  ),
                ),
              ListView.builder(
                key: const Key('chat-messages'),
                controller: scroll,
                padding: const EdgeInsets.all(CineTokens.md),
                itemCount: social.entries.length,
                itemBuilder: (context, index) {
                  final entry = social.entries[index];
                  final name = '${entry['display_name']}';
                  final kind = entry['kind'];
                  if (kind != 'chat') {
                    final verb = switch (kind) {
                      'joined' => 'se unió a la sala',
                      'left' => 'salió de la sala',
                      _ => 'se reconectó',
                    };
                    return Padding(
                      key: ValueKey(entry['message_id']),
                      padding: const EdgeInsets.symmetric(
                        vertical: CineTokens.sm,
                      ),
                      child: Text(
                        '$name $verb',
                        style: const TextStyle(
                          color: CineTokens.muted,
                          fontSize: 12,
                        ),
                      ),
                    );
                  }
                  final mine = entry['sender_id'] == social.memberId;
                  return Padding(
                    key: ValueKey(entry['message_id']),
                    padding: const EdgeInsets.only(bottom: CineTokens.md),
                    child: Semantics(
                      label: 'Mensaje de $name',
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            mine ? '$name · tú' : name,
                            style: TextStyle(
                              color: mine
                                  ? CineTokens.accent
                                  : CineTokens.muted,
                              fontWeight: FontWeight.w600,
                              fontSize: 12,
                            ),
                          ),
                          const SizedBox(height: 3),
                          SelectableText('${entry['text']}'),
                        ],
                      ),
                    ),
                  );
                },
              ),
              if (newMessages)
                Positioned(
                  bottom: 8,
                  left: 16,
                  right: 16,
                  child: FilledButton.tonal(
                    onPressed: toBottom,
                    child: const Text('Nuevos mensajes'),
                  ),
                ),
            ],
          ),
        ),
        if (social.errorText.isNotEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: CineTokens.md),
            child: Text(
              social.errorText,
              key: const Key('social-error'),
              style: const TextStyle(color: CineTokens.warning),
            ),
          ),
        if (!social.connected)
          const Padding(
            padding: EdgeInsets.all(8),
            child: Text(
              'Reconecta para conversar.',
              style: TextStyle(color: CineTokens.muted),
            ),
          ),
        if (social.connected && !social.supported)
          const Text('Chat no disponible en este servidor.'),
        Padding(
          padding: const EdgeInsets.all(CineTokens.sm),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Expanded(
                child: Focus(
                  onKeyEvent: (_, event) {
                    // A focused editor consumes player keys, including F and arrows.
                    if (event is KeyDownEvent &&
                        event.logicalKey == LogicalKeyboardKey.enter &&
                        !HardwareKeyboard.instance.isShiftPressed &&
                        !Platform.isAndroid &&
                        !Platform.isIOS &&
                        input.value.composing.isCollapsed) {
                      unawaited(send());
                      return KeyEventResult.handled;
                    }
                    if (event.logicalKey == LogicalKeyboardKey.escape) {
                      return KeyEventResult.ignored;
                    }
                    return KeyEventResult.skipRemainingHandlers;
                  },
                  child: TextField(
                    key: const Key('chat-input'),
                    controller: input,
                    focusNode: focus,
                    minLines: 1,
                    maxLines:
                        MediaQuery.orientationOf(context) ==
                                Orientation.landscape &&
                            MediaQuery.viewInsetsOf(context).bottom > 0
                        ? 1
                        : 4,
                    keyboardType: TextInputType.multiline,
                    textInputAction: TextInputAction.newline,
                    decoration: const InputDecoration(
                      labelText: 'Mensaje a la sala',
                      hintText: 'Escribe un mensaje…',
                    ),
                    onChanged: (value) {
                      controller.chatDraft = value;
                      setState(() {});
                    },
                  ),
                ),
              ),
              IconButton(
                key: const Key('chat-send'),
                tooltip: 'Enviar mensaje',
                onPressed: social.canSend && input.text.trim().isNotEmpty
                    ? send
                    : null,
                icon: Icon(social.pending ? Icons.hourglass_top : Icons.send),
              ),
            ],
          ),
        ),
      ],
    ),
  );
}

/// Isolated, capped visual layer; animations never announce individual emojis.
class ReactionOverlay extends StatefulWidget {
  final ApplicationController controller;
  const ReactionOverlay({required this.controller, super.key});
  @override
  State<ReactionOverlay> createState() => _ReactionOverlayState();
}

class _ReactionOverlayState extends State<ReactionOverlay> {
  final seen = <String>{};
  final active = <String, Map<String, dynamic>>{};
  final timers = <String, Timer>{};
  int lane = 0;
  @override
  void initState() {
    super.initState();
    widget.controller.social.addListener(update);
  }

  void update() {
    final social = widget.controller.social.value;
    if (!social.connected) {
      for (final timer in timers.values) {
        timer.cancel();
      }
      timers.clear();
      active.clear();
    }
    for (final reaction in social.reactions) {
      final id = '${reaction['reaction_id']}';
      if (!seen.add(id)) continue;
      if (active.length >= 12) continue;
      active[id] = {...reaction, 'lane': lane++ % 5};
      timers[id] = Timer(const Duration(milliseconds: 2200), () {
        if (mounted) {
          setState(() {
            active.remove(id);
            timers.remove(id);
          });
        }
      });
    }
    // Only keep bounded dedup metadata. Rust has a 3 s delivery queue.
    if (seen.length > 128) {
      final current = social.reactions
          .map((r) => '${r['reaction_id']}')
          .toSet();
      seen.removeWhere(
        (id) => !active.containsKey(id) && !current.contains(id),
      );
    }
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    widget.controller.social.removeListener(update);
    for (final t in timers.values) {
      t.cancel();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => IgnorePointer(
    child: ExcludeSemantics(
      child: LayoutBuilder(
        builder: (context, bounds) => Stack(
          children: [
            for (final item in active.entries)
              Positioned(
                key: ValueKey(item.key),
                left:
                    bounds.maxWidth * (.22 + .14 * number(item.value['lane'])) -
                    22,
                bottom: bounds.maxHeight * .23,
                child: TweenAnimationBuilder<double>(
                  tween: Tween(begin: 0, end: 1),
                  duration: const Duration(milliseconds: 2200),
                  builder: (_, t, child) => Opacity(
                    opacity: (1 - t * t).clamp(0, 1),
                    child: Transform.translate(
                      offset: Offset(0, -70 * t),
                      child: Transform.scale(
                        scale: .85 + .15 * (t * 5).clamp(0, 1),
                        child: child,
                      ),
                    ),
                  ),
                  child: Text(
                    '${item.value['emoji']}',
                    style: const TextStyle(fontSize: 36),
                  ),
                ),
              ),
          ],
        ),
      ),
    ),
  );
}
