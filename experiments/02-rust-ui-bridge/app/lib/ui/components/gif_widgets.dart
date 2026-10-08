import 'dart:async';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import '../../presentation/application_controller.dart';
import '../../social/gif_provider.dart';
import '../../social/gif_search.dart';
import '../theme/product_theme.dart';

Future<void> showGifPicker(
  BuildContext context,
  ApplicationController controller,
) async {
  final mobile = MediaQuery.sizeOf(context).width < 900;
  final picker = GifPicker(controller: controller);
  if (mobile) {
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (context) => Padding(
        padding: EdgeInsets.only(
          bottom: MediaQuery.viewInsetsOf(context).bottom,
        ),
        child: SizedBox(
          height:
              (MediaQuery.sizeOf(context).height -
                      MediaQuery.viewInsetsOf(context).bottom -
                      MediaQuery.paddingOf(context).top)
                  .clamp(0, 460),
          child: picker,
        ),
      ),
    );
  } else {
    await showDialog<void>(
      context: context,
      builder: (_) => Dialog(
        clipBehavior: Clip.antiAlias,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 360, maxHeight: 440),
          child: picker,
        ),
      ),
    );
  }
}

class GifPicker extends StatefulWidget {
  final ApplicationController controller;
  const GifPicker({required this.controller, super.key});
  @override
  State<GifPicker> createState() => _GifPickerState();
}

class _GifPickerState extends State<GifPicker> {
  late final search = GifSearch(widget.controller.gifProvider);
  final query = TextEditingController();
  @override
  void initState() {
    super.initState();
    search.addListener(changed);
    unawaited(search.load());
  }

  void changed() {
    widget.controller.gifLastError = search.error;
    widget.controller.gifSearchLatencyMs = search.latencyMs;
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    search.removeListener(changed);
    search.dispose();
    query.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Column(
    children: [
      Row(
        children: [
          const SizedBox(width: 12),
          const Expanded(child: Text('Enviar GIF')),
          IconButton(
            tooltip: 'Cerrar GIFs',
            onPressed: () => Navigator.pop(context),
            icon: const Icon(Icons.close),
          ),
        ],
      ),
      Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12),
        child: TextField(
          key: const Key('gif-search'),
          controller: query,
          maxLength: 50,
          decoration: const InputDecoration(
            hintText: 'Buscar GIFs',
            counterText: '',
            prefixIcon: Icon(Icons.search),
          ),
          onChanged: search.change,
        ),
      ),
      Expanded(
        child: search.error.isNotEmpty
            ? SingleChildScrollView(
                padding: const EdgeInsets.all(12),
                child: Column(
                  children: [
                    Text(
                      GifFailure(search.error).message,
                      textAlign: TextAlign.center,
                    ),
                    TextButton(
                      onPressed: () => search.load(),
                      child: const Text('Reintentar'),
                    ),
                  ],
                ),
              )
            : search.loading && search.items.isEmpty
            ? const Center(child: CircularProgressIndicator())
            : search.items.isEmpty
            ? const Center(child: Text('No se encontraron GIFs.'))
            : NotificationListener<ScrollNotification>(
                onNotification: (n) {
                  if (n.metrics.extentAfter < 100 &&
                      search.next != null &&
                      !search.loading) {
                    unawaited(search.load(more: true));
                  }
                  return false;
                },
                child: GridView.builder(
                  padding: const EdgeInsets.all(12),
                  gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                    crossAxisCount: 2,
                    crossAxisSpacing: 8,
                    mainAxisSpacing: 8,
                    childAspectRatio: 1.4,
                  ),
                  itemCount: search.items.length,
                  itemBuilder: (context, i) {
                    final gif = search.items[i];
                    return Tooltip(
                      message: 'Enviar GIF: ${gif.alt}',
                      child: InkWell(
                        key: ValueKey('gif-select-$i'),
                        onTap: widget.controller.social.value.canSendRich
                            ? () async {
                                final sent = await widget.controller.sendGif(
                                  gif,
                                );
                                if (!context.mounted) return;
                                if (sent) {
                                  Navigator.pop(context);
                                } else {
                                  ScaffoldMessenger.of(context).showSnackBar(
                                    SnackBar(
                                      content: Text(
                                        widget
                                            .controller
                                            .social
                                            .value
                                            .errorText,
                                      ),
                                    ),
                                  );
                                }
                              }
                            : null,
                        child: GifImage(
                          gif: gif,
                          controller: widget.controller,
                          label: 'Seleccionar GIF: ${gif.alt}',
                          preview: true,
                        ),
                      ),
                    );
                  },
                ),
              ),
      ),
      if (search.loading && search.items.isNotEmpty)
        const LinearProgressIndicator(),
      if (search.next != null && !search.loading)
        TextButton(
          onPressed: () => search.load(more: true),
          child: const Text('Más GIFs'),
        ),
      Padding(
        padding: const EdgeInsets.all(8),
        child: Text(
          widget.controller.gifProvider.attribution,
          style: const TextStyle(fontSize: 11, color: CineTokens.muted),
        ),
      ),
    ],
  );
}

/// Removes the animated image stream outside the actual scroll viewport.
/// Reduced motion decodes only the first frame and disposes the codec.
class GifImage extends StatefulWidget {
  final GifDescriptor gif;
  final ApplicationController controller;
  final String label;
  final bool preview;
  const GifImage({
    required this.gif,
    required this.controller,
    required this.label,
    this.preview = false,
    super.key,
  });
  @override
  State<GifImage> createState() => _GifImageState();
}

class _GifImageState extends State<GifImage> {
  Uint8List? bytes;
  ui.Image? still;
  bool failed = false, visible = true, reduce = false;
  ScrollPosition? position;
  int generation = 0;
  @override
  void initState() {
    super.initState();
    widget.controller.gifCache.configureImages();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final next = Scrollable.maybeOf(context)?.position;
    if (position != next) {
      position?.removeListener(scheduleVisibility);
      position = next;
      position?.addListener(scheduleVisibility);
    }
    final reduced =
        MediaQuery.disableAnimationsOf(context) ||
        MediaQuery.of(context).accessibleNavigation;
    if (bytes == null && !failed || reduced != reduce) {
      reduce = reduced;
      unawaited(load());
    }
    scheduleVisibility();
  }

  @override
  void didUpdateWidget(covariant GifImage old) {
    super.didUpdateWidget(old);
    if (old.gif.mediaUrl != widget.gif.mediaUrl) {
      bytes = null;
      still?.dispose();
      still = null;
      failed = false;
      unawaited(load());
    }
  }

  Future<void> load() async {
    final current = ++generation;
    try {
      final data = await widget.controller.gifCache.load(widget.gif);
      ui.Image? image;
      if (reduce) {
        final codec = await ui.instantiateImageCodec(
          data,
          targetWidth: widget.gif.width.clamp(1, 320),
        );
        try {
          image = (await codec.getNextFrame()).image;
        } finally {
          codec.dispose();
        }
      }
      if (!mounted || generation != current) {
        image?.dispose();
        return;
      }
      setState(() {
        still?.dispose();
        still = image;
        bytes = data;
        failed = false;
      });
    } catch (_) {
      if (mounted && generation == current) setState(() => failed = true);
    }
  }

  bool visibilityScheduled = false;
  void scheduleVisibility() {
    if (visibilityScheduled) return;
    visibilityScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      visibilityScheduled = false;
      if (!mounted) return;
      final box = context.findRenderObject();
      final viewport = Scrollable.maybeOf(context)?.context.findRenderObject();
      if (box is! RenderBox ||
          viewport is! RenderBox ||
          !box.hasSize ||
          !viewport.hasSize) {
        return;
      }
      final rect = box.localToGlobal(Offset.zero) & box.size;
      final view = viewport.localToGlobal(Offset.zero) & viewport.size;
      final next = rect.overlaps(view);
      if (next != visible) setState(() => visible = next);
    });
  }

  @override
  void dispose() {
    generation++;
    position?.removeListener(scheduleVisibility);
    still?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Semantics(
    label: widget.label,
    excludeSemantics: true,
    child: ClipRRect(
      borderRadius: BorderRadius.circular(CineTokens.radius),
      child: ColoredBox(
        color: CineTokens.surface,
        child: SizedBox.expand(
          child: failed
              ? const Center(
                  child: Text('GIF no disponible', textAlign: TextAlign.center),
                )
              : bytes == null
              ? const Center(
                  child: SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                )
              : !visible
              ? const Center(child: Icon(Icons.gif_box_outlined))
              : reduce
              ? RawImage(image: still, fit: BoxFit.contain)
              : Image.memory(
                  bytes!,
                  fit: BoxFit.contain,
                  cacheWidth: widget.gif.width.clamp(1, 320),
                  errorBuilder: (_, _, _) =>
                      const Center(child: Text('GIF no disponible')),
                ),
        ),
      ),
    ),
  );
}
