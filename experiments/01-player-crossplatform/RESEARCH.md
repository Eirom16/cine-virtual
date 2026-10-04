# Spike B — Investigación y gate multimedia

Fecha de consulta: 2026-10-04. Estado: INVESTIGADO; las afirmaciones de upstream
no equivalen a builds/runtime de Cine Virtual. La evidencia propia se registra
por separado en [RESULTS](RESULTS.md). No se ha seleccionado motor definitivo.

## Fuentes primarias y método

Se consultaron documentación/API, código y licencias upstream; se inspeccionaron
versiones, headers, paquetes y capacidades locales. No se asignan puntuaciones:
se compara el coste de demostrar el puerto Player ahora con los riesgos futuros.

- **M1:** [mpv upstream](https://github.com/mpv-player/mpv), plataformas/build.
- **M2:** [C API de libmpv](https://raw.githubusercontent.com/mpv-player/mpv/master/include/mpv/client.h), ABI, eventos, threading y embedding.
- **M3:** [manual estable](https://mpv.io/manual/stable/), controles, exact seek, speed, pitch, null outputs y hwdec.
- **M4:** [Copyright mpv](https://github.com/mpv-player/mpv/blob/master/Copyright), builds GPL/LGPL y header ISC.
- **M5:** [mpv-android](https://github.com/mpv-android/mpv-android), port comunitario y build NDK/JNI, no SDK AAR consumible.
- **V1:** [libVLC oficial](https://images.videolan.org/vlc/libvlc.html), SDK in-process y plataformas.
- **V2:** [API media player](https://github.com/videolan/vlc/blob/master/include/vlc/libvlc_media_player.h), controles/callbacks; master puede diferir de VLC 3 estable.
- **V3:** [VLCKit oficial](https://github.com/videolan/vlckit), integración Apple y licencia.
- **V4:** [VLC upstream](https://github.com/videolan/vlc), LGPL del engine frente a GPL del reproductor.
- **G1:** [FAQ GStreamer](https://gstreamer.freedesktop.org/documentation/frequently-asked-questions/general.html), plataformas y naturaleza del framework.
- **G2:** [playbin](https://gstreamer.freedesktop.org/documentation/playback/playbin.html), pipeline de reproducción y bus.
- **G3:** [licencias GStreamer](https://gstreamer.freedesktop.org/documentation/application-development/appendix/licensing.html), dependencias/plugins.
- **G4:** [SDK Android](https://gstreamer.freedesktop.org/documentation/installing/for-android-development.html) y [SDK iOS](https://gstreamer.freedesktop.org/documentation/installing/for-ios-development.html).
- **N1:** [AVPlayer](https://developer.apple.com/documentation/avfoundation/avplayer) y [seek con completion/tolerancias](https://developer.apple.com/documentation/avfoundation/avplayer/seek(to:tolerancebefore:toleranceafter:completionhandler:)).
- **N2:** [Media3 Player](https://developer.android.com/reference/androidx/media3/common/Player) y [ExoPlayer](https://developer.android.com/media/media3/exoplayer).
- **N3:** [Media Foundation](https://learn.microsoft.com/en-us/windows/win32/medfound/media-foundation-programming-guide).
- **F1:** [FFmpeg legal/build](https://ffmpeg.org/legal.html).
- **A1:** [App Store Review Guidelines](https://developer.apple.com/app-store/review/guidelines/).

## Matriz de decisión

Las referencias M/V/G/N corresponden a las fuentes anteriores. «No validado»
significa que Cine Virtual no ha ejecutado esa plataforma. En móviles distinguir
la biblioteca de un wrapper comunitario y de una aplicación publicada es esencial.

| Criterio | libmpv | libVLC | GStreamer | Players nativos |
|---|---|---|---|---|
| Linux | Target upstream; instalado localmente (M1) | SDK upstream (V1), no probado | SDK/framework upstream (G1), no probado | No único SDK Linux; exige otra implementación |
| Windows | Build upstream/cross-compilación (M1), no validado | SDK upstream (V1), no validado | SDK oficial (G1), no validado | Media Foundation (N3), no validado |
| macOS | Target upstream (M1), no validado | VLCKit (V3), no validado | SDK oficial (G1), no validado | AVPlayer (N1), no validado |
| Android | Port comunitario mpv-android/NDK; no AAR upstream listo (M5), no validado | SDK/JNI VideoLAN (V1), no validado | SDK oficial NDK (G4), no validado | Media3/ExoPlayer oficial (N2), no validado |
| iOS | Posible port/build/embed; no ruta SDK iOS equivalente a AVPlayer demostrada por nosotros | MobileVLCKit oficial (V3), no validado | SDK oficial/framework (G4), no validado | AVPlayer oficial (N1), no validado |
| Rust bindings | libmpv2 comunitario; FFI C pequeño viable (M2) | vlc-rs comunitario, release antigua | gstreamer-rs mantenido en ecosistema upstream | Bridges separados por SDK; no binding único |
| Mantenimiento bindings | libmpv2 6.0.0 publicado 2026-05-12; nuestro FFI implica mantener símbolos/ABI | vlc-rs 0.3.0 publicado 2018-06-10; fecha no prueba abandono | gstreamer 0.25.4 publicado 2026-09-21; actividad reciente | Coste acumulado de varios SDK y bridges |
| API C estable | API versionada; opciones pueden cambiar aunque ABI sea compatible (M2) | API C pública; considerar versión 3/4 (V2) | API C estable por serie mayor (G1) | Apple ObjC/Swift, Android Java/Kotlin; Windows COM, sin ABI común |
| In-process | Sí, mpv_create (M2) | Sí, SDK (V1) | Sí, pipeline (G2) | Sí, SDK local |
| Play/Pause | pause property (M3) | API específica (V2) | transición de estados (G2) | API específica N1/N2/N3 |
| Seek | exact/keyframe; completion distinto de dispatch (M2/M3) | posición/tiempo y eventos; depende del input (V2) | seek flags, bus ASYNC_DONE (G2) | tolerancias/completion Apple; discontinuidades Android (N1/N2) |
| Precisión position | timeline reportada, no prueba del frame visible (M2/M3) | tiempo reportado; granularidad a medir (V2) | query de posición, depende pipeline (G2) | depende SDK, medio y dispositivo; medir |
| Playback rate | speed disponible (M3) | set_rate puede no funcionar según input (V2) | rate vía seek/eventos (G2) | rate/PlaybackParameters; capacidades dependientes de plataforma |
| Audio durante rate | pitch correction configurable; requiere escucha/medición (M3) | no asumir calidad por retorno API | depende elementos/filtros del pipeline | velocidad/pitch dependientes del SDK |
| Eventos/callbacks | cola wait_event; debe drenarse (M2) | event manager, callbacks no bloqueantes (V2) | bus/GMainContext (G2) | KVO/completion Apple; listeners en Looper Android |
| Buffering | property paused-for-cache (M3) | eventos SDK (V2) | mensajes bus (G2) | estados nativos (N1/N2) |
| Hardware decoding | hwdec configurable, consultar uso real (M3) | SDK/plugins y plataforma | plugins por plataforma | SDK aprovecha plataforma, no garantiza codec concreto |
| Render embebido | render API requiere contexto/surface del consumidor (M2) | drawables/callbacks y wrappers (V2/V3) | video overlay/sinks por plataforma (G2) | surface/view del SDK, lifecycle local |
| URI móviles | descriptor local/Fd o stream callback requiere adapter; no tratar content:// como path | wrappers móviles para recursos, validar URI concretas | elementos/source y bridge; validar SDK | content:// Android mediante DataSource; Apple assets/bookmarks |
| Lifecycle móvil | bridge/surface/background por demostrar | wrappers oficiales ayudan, no sustituyen tests | integración SDK/GMainLoop por demostrar | alineado SDK; mantener estados de foreground/audio interruptions |
| Packaging | libmpv + FFmpeg/libplacebo/etc.; construir cierre | libVLC + plugins/SDK móvil | runtime + plugins seleccionados + SDK | menos runtime incluido, formatos condicionados por sistema |
| Tamaño aproximado | local .so 3,018,168 bytes; paquete mpv 6.35 MiB, excluye dependencias | no medido; depende plugins/ABIs | no medido; depende plugins/ABIs | runtime del SO; tamaño de bridges no medido |
| Complejidad build | Meson y dependencias; Linux ya disponible | SDK/prebuilt o contrib toolchain | Meson/Cerbero, selección de plugins | varios toolchains, integración específica |
| Licencias | API ISC; engine GPL por defecto, LGPL opcional condicionado (M4) | engine LGPL2.1+, app VLC GPL; plugins/build revisar (V4) | framework LGPL2.1+, plugins/dependencias revisar (G3) | términos SDK; Media3 Apache2; codecs/patentes aparte |
| App Store | investigar build/distribución y licencias antes de promover | VLCKit evidencia ruta técnica, no aprobación automática | SDK iOS evidencia ruta técnica, no aprobación automática | API nativa tampoco garantiza revisión aprobada (A1) |
| Testabilidad | null outputs + decoder real, eventos consultables | dummy outputs/control SDK | fakesinks/bus/pipelines deterministas | dispositivos/simuladores y limitaciones por SDK |
| Mantenimiento esperado | FFI reducido más controles, builds por plataforma pendientes | wrappers y plugins, Rust binding a revisar | mayor infraestructura pipeline, bindings fuertes | mayor número de adapters; menor dependencia de motor universal |

## Licencias y distribución (riesgos técnicos, no asesoría legal)

libmpv expone headers ISC, pero eso no cambia la licencia del engine enlazado.
Upstream mpv es GPL por defecto y permite builds LGPL deshabilitando código GPL;
las dependencias también deben ser compatibles con ese objetivo (M4). El paquete
local mpv 0.41.0 enlaza FFmpeg n9.0.2 construido con enable-gpl y enable-version3:
**este build local no es evidencia de una distribución LGPL lista para móvil**.

libVLC es LGPL2.1+; la aplicación VLC tiene licencia GPL. VLCKit documenta LGPL2.1.
Revisar módulos, contrib y build concreto, no inferir licencia de una app del mismo
nombre. GStreamer es LGPL2.1+; la composición de plugins/dependencias puede cambiar
las obligaciones. FFmpeg base es LGPL2.1+, opciones GPL/version3/nonfree cambian la
redistribución (F1); patentes de codecs son otra cuestión.

Bindings evaluados, metadatos primarios de crates.io: [libmpv2 6.0.0](https://crates.io/crates/libmpv2/6.0.0)
LGPL2.1; [vlc-rs 0.3.0](https://crates.io/crates/vlc-rs/0.3.0) MIT;
[gstreamer 0.25.4](https://crates.io/crates/gstreamer/0.25.4) MIT/Apache2;
[libloading 0.9.0](https://crates.io/crates/libloading/0.9.0) ISC.
Las licencias de wrappers no sustituyen las del SDK. Media3 es
[Apache2](https://github.com/androidx/media/blob/release/LICENSE); los SDK Apple y
Windows mantienen sus términos de plataforma.

Linking dinámico de escritorio facilita sustituir bibliotecas, pero no elimina
obligaciones. Linking estático/embedded frameworks, relinking, avisos, fuente del
build y términos de tienda necesitan revisión antes de empaquetar. Las reglas
App Store sobre APIs públicas/código incluido tampoco resuelven compatibilidad de
licencias (A1). No se modifica la licencia pendiente de Cine Virtual, no se crea
LICENSE ni se publica nada. Consultar [LICENSING](../../docs/LICENSING.md).

## FFmpeg: rol limitado

Se utiliza ffmpeg/ffprobe para generar e inspeccionar corpus sintético. Ofrece
codecs/demuxers, pero no resuelve por sí solo rendering, audio clock, surfaces,
callbacks y lifecycle de un Player integrado. No se construye un motor encima de
FFmpeg en este spike. El generador usa solo codecs disponibles localmente.

## Gate de decisión previo a implementación

**Candidato PROVISIONAL para Spike B Linux: libmpv.** Su API cubre el puerto actual,
se pueden observar seek/restart y reproducir con outputs null, y la biblioteca ya
está instalada. Permite responder al objetivo con una única integración pequeña.
GStreamer ofrece mejor ecosistema Rust y SDK móvil explícito, pero añade pipeline
sin necesidad actual. libVLC ofrece wrappers móviles atractivos, pero el binding
Rust evaluado necesita revisión y no aporta ventaja inmediata en esta máquina.
Players nativos siguen siendo una alternativa seria, especialmente móvil, pero
no existe una implementación nativa única para el ensayo Linux.

Se implementará un FFI acotado al C API con libloading, no un binding completo.
Ventaja: cargo build/tests del workspace no requieren SDK multimedia en link-time.
Coste: revisión de unsafe/ABI y mantenimiento propio. No se fuerza Send/Sync:
un propietario bombea eventos y destruye ordenadamente. Solo el adapter accede a
la biblioteca; cine-core sigue std-only. Render embebido, Android/iOS y build
redistribuible son gates posteriores. Ningún candidato queda descartado como
solución futura. Esta decisión precede al código del adapter.
