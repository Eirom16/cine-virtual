import 'dart:convert';

import 'package:flutter/material.dart';

import '../../presentation/application_controller.dart';
import '../components/product_components.dart';
import '../theme/product_theme.dart';

class EntryScreen extends StatefulWidget {
  final ApplicationController controller;
  final bool create;
  final VoidCallback onEntered;
  const EntryScreen({
    required this.controller,
    required this.create,
    required this.onEntered,
    super.key,
  });
  @override
  State<EntryScreen> createState() => _EntryScreenState();
}

class _EntryScreenState extends State<EntryScreen> {
  final name = TextEditingController(),
      server = TextEditingController(),
      invite = TextEditingController();
  bool submitted = false;
  String? validation;
  @override
  void initState() {
    super.initState();
    name.text = widget.controller.displayName;
    server.text = widget.controller.endpoint;
  }

  @override
  void dispose() {
    name.dispose();
    server.dispose();
    invite.dispose();
    super.dispose();
  }

  Future<void> submit() async {
    if (widget.controller.busy) return;
    String? invalid;
    if (name.text.trim().isEmpty || utf8.encode(name.text.trim()).length > 64) {
      invalid = 'Escribe tu nombre (hasta 64 bytes).';
    }
    if (!widget.create && invalid == null) {
      try {
        Invitation.parse(invite.text);
      } catch (_) {
        invalid = 'Pega una invitación completa y válida.';
      }
    }
    setState(() {
      submitted = true;
      validation = invalid;
    });
    if (invalid != null) return;
    final ok = await widget.controller.enter(
      create: widget.create,
      name: name.text,
      server: server.text,
      invite: invite.text,
    );
    if (ok && mounted) widget.onEntered();
  }

  @override
  Widget build(BuildContext context) => SingleChildScrollView(
    padding: CineTokens.pageInsets,
    child: Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 520),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const SizedBox(height: CineTokens.lg),
            Text(
              widget.create
                  ? 'La noche empieza aquí.'
                  : 'Tu lugar está reservado.',
              style: Theme.of(context).textTheme.headlineMedium,
            ),
            const SizedBox(height: CineTokens.sm),
            Text(
              widget.create
                  ? 'Crea una sala y comparte la invitación.'
                  : 'Pega la invitación que te compartió el anfitrión.',
              style: const TextStyle(color: CineTokens.muted),
            ),
            const SizedBox(height: CineTokens.xl),
            SectionCard(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  TextField(
                    key: const Key('display-name'),
                    controller: name,
                    enabled: !widget.controller.busy,
                    textCapitalization: TextCapitalization.words,
                    textInputAction: widget.create
                        ? TextInputAction.done
                        : TextInputAction.next,
                    onSubmitted: widget.create ? (_) => submit() : null,
                    decoration: InputDecoration(
                      labelText: 'Tu nombre',
                      hintText: 'Cómo te verán en la sala',
                      errorText: submitted && name.text.trim().isEmpty
                          ? 'Escribe tu nombre'
                          : null,
                    ),
                  ),
                  if (!widget.create) ...[
                    const SizedBox(height: CineTokens.lg),
                    TextField(
                      key: const Key('invitation'),
                      controller: invite,
                      enabled: !widget.controller.busy,
                      obscureText: true,
                      maxLines: 1,
                      autocorrect: false,
                      enableSuggestions: false,
                      decoration: const InputDecoration(
                        labelText: 'Invitación de la sala',
                        hintText: 'Pega aquí la invitación completa',
                      ),
                    ),
                  ],
                  const SizedBox(height: CineTokens.sm),
                  ExpansionTile(
                    tilePadding: EdgeInsets.zero,
                    title: const Text(
                      'Configuración avanzada',
                      style: TextStyle(fontSize: 14, color: CineTokens.muted),
                    ),
                    initiallyExpanded: server.text.isEmpty,
                    children: [
                      TextField(
                        key: const Key('server-endpoint'),
                        controller: server,
                        enabled: !widget.controller.busy,
                        keyboardType: TextInputType.url,
                        autocorrect: false,
                        enableSuggestions: false,
                        decoration: const InputDecoration(
                          labelText: 'Servidor',
                          hintText: 'ws://servidor:puerto',
                          helperText: 'Servicio propio. La invitación puede incluir la conexión.',
                          helperMaxLines: 3,
                        ),
                      ),
                      const SizedBox(height: CineTokens.md),
                    ],
                  ),
                  if (validation != null)
                    Padding(
                      padding: const EdgeInsets.only(bottom: CineTokens.md),
                      child: Text(
                        validation!,
                        style: const TextStyle(color: CineTokens.error),
                      ),
                    ),
                  const SizedBox(height: CineTokens.md),
                  BusyButton(
                    key: const Key('enter-room'),
                    label: widget.create ? 'Crear sala' : 'Unirse',
                    busy: widget.controller.busy,
                    onPressed: submit,
                  ),
                ],
              ),
            ),
            const SizedBox(height: CineTokens.lg),
            const Text(
              'No necesitas una cuenta. Tu nombre se usa solo en esta sala.',
              style: TextStyle(color: CineTokens.muted, fontSize: 12),
            ),
          ],
        ),
      ),
    ),
  );
}
