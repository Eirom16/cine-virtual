# Auditoría y spike Linux

## Cadena anterior

Flutter PlayerScreen → ApplicationController/NativeSessionGateway → CineBridge
(C ABI manual) → Desktop Tokio current-thread → Client<BackendPlayer> →
RealPlayer proxy/cola 32 → cine-mpv-owner → MpvPlayer !Send/!Sync → libmpv.
Config visible=true selecciona vo=gpu; libmpv posee la ventana separada.
No render context ni surface GTK existente. GTK3 runner contiene FlView.
Owner crea/controla/polleea/destruye mpv; último Arc detiene y une el owner.

## Alternativas

| Opción | Wayland/X11 | Overlay/input | Recursos/threading | Decisión |
| --- | --- | --- | --- | --- |
| Render API + FlTextureGL/EGL | Compatible en diseño con ambos backends del embedder | Composición Flutter normal | Contexto GL propio compartido; FBO + copia GPU; resize explícito | Spike elegido |
| Render API directo en contexto raster | Ambos | Normal | Estado GL de Flutter alterable, destruction difícil | Descartado por ownership |
| GtkGLArea/surface nativo | Ambos en GTK | Flutter no compone automáticamente sobre widget hermano | GTK main y contexto independiente; sharing no garantizado | No preferido |
| wid X11 child | X11 ONLY | Z-order/input nativos | Reparent/resize y toolkit/window ownership | Rechazado |
| wl_surface child | Wayland específico | Subsuface nativa; composición/input complejos | Roles/parent/SDK/toolkit | No API portable suficiente para esta tarea |
| Render software/CPU buffer | Ambos | Normal | Copies/CPU por frame | Rechazado para producción |

Flutter fijado usa FlOpenGLManager con EGL propio (no GdkGLContext). Por eso no
se asume sharing implícito GTK. FlTextureGL.populate llega con contexto Flutter
actual; allí se crea un EGLContext propio compartido. Render API no controla
reproducción. Una textura consumidora solo cambia en populate; productor se
sincroniza antes de copia GPU, sin readback/bytes Dart. Callbacks mpv solo despiertan
render worker; control permanece en owner Rust. Liberación render antes de lease.

## Fuentes primarias consultadas

- [mpv render API/threading/lifecycle](https://github.com/mpv-player/mpv/blob/master/include/mpv/render.h).
- [mpv OpenGL FBO/state](https://github.com/mpv-player/mpv/blob/master/include/mpv/render_gl.h).
- [Flutter FlTextureGL](https://api.flutter.dev/linux-embedder/fl__texture__gl_8h_source.html).
- [Flutter texture registrar](https://api.flutter.dev/linux-embedder/fl__texture__registrar_8cc.html).
- [Flutter EGL fuente fijada](https://github.com/flutter/flutter/blob/5fc346839b5d0eef006ed8404392afb4dfae428d/engine/src/flutter/shell/platform/linux/fl_opengl_manager.cc).
- [GTK GLContext](https://docs.gtk.org/gdk3/method.Window.create_gl_context.html).

Spike: iniciar app con CINE_EMBEDDING_SPIKE apuntando exclusivamente a corpus
sintético; ruta Flutter aislada. Probar frame, overlay, resize, play/pause/seek,
destroy/recreate antes de integrar al producto. Estado inicial NOT TESTED.

## Evidencia de spike

TESTED / VISUALLY VERIFIED en KDE KWin 6.7.5, GDK_BACKEND=wayland,
Flutter Impeller OpenGLESSDF, libmpv API 2.5. Se vio vídeo sintético de
long-duration.mp4 y overlay Flutter. Play avanzó a 2.267s, Seek 45s mostró
46.867s durante Play; Pause y destroy/recreate accionados por input real.
Se detectó y corrigió flip vertical (MPV_RENDER_PARAM_FLIP_Y=0 para esta textura).
No ventana mpv: vo=libmpv. No se infiere soporte universal de este compositor.

## Integración y hallazgos posteriores

El spike reveló también que FlTextureGL no comunica automáticamente el tamaño
actual del widget: bootstrap de 1×1 no debe convertirse en el tamaño permanente
del render. `DesktopVideo` envía dimensiones físicas explícitas; mpv conserva
contain y black bars en el FBO, sin deformar al redimensionar.

El control de integridad del FBO debe enlazar el productor en cada render: mpv
puede cambiar el binding GL. El primer chequeo añadía un falso fallo en el
framebuffer default de un contexto surfaceless; se corrigió y se repitió vídeo
real antes de registrar éxito. Errores EGL/FBO/render y cambio de contexto llegan
a Developer y al mensaje de producto; context-loss real NOT TESTED.

La vista nativa Android se mantiene con key estable al ocultar/mostrar controles.
El test regresivo creó 2 platform views sin key, y 1 con key; se dispone una vez
al desmontar. Esto verifica lifecycle, no frames. No se atribuye automáticamente
todo negro observado al final de un clip a esta causa.

## Fullscreen Android: restauración API 28

La revisión física de salida fullscreen detectó la barra de estado todavía
oculta. En el engine Flutter fijado, PlatformPlugin solo aplica EDGE_TO_EDGE
con SDK >=29 y retorna sin cambiar flags por debajo. Se restaura modo manual
con ambos SystemUiOverlay al salir. Fuente primaria:
[PlatformPlugin fijado](https://github.com/flutter/flutter/blob/5fc346839b5d0eef006ed8404392afb4dfae428d/engine/src/flutter/shell/platform/android/io/flutter/plugin/platform/PlatformPlugin.java).
Android 15/16 tiene restricciones adicionales del sistema; NOT TESTED aquí.
