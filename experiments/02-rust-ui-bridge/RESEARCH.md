# Spike C — Investigación móvil, UI y puente

Consulta: 2026-10-04. INVESTIGADO no significa ejecutado. Evidencia propia y
limitaciones en [RESULTS](RESULTS.md). Este gate precede a los adapters móviles.

## Opciones de UI y Application

| Arquitectura | Beneficio | Coste / gate |
| --- | --- | --- |
| Flutter + Rust | Presentación común, política temporal única y tests del Core reutilizables | Toolchains por plataforma, frontera FFI y ownership explícito |
| Flutter + Player nativo | URI, surface y lifecycle alineados con el SDK; Rust decide efectos | Dos adapters móviles, precisión/formatos distintos por medir |
| Flutter + libmpv | Controles y codecs parecidos a Linux | Build NDK/Apple, surface, codec closure y GPL/LGPL no resueltos |
| UI nativa + Rust | Integración directa con lifecycle/surface y accesibilidad nativa | Varias UIs y pipelines de build; misma necesidad de API Application |

Flutter mantiene las cinco plataformas objetivo en su documentación. Su engine
incluye embedders y Dart FFI está disponible en targets nativos; no hace portable
un SDK multimedia. PlatformView permite alojar vistas nativas, pero composición,
performance y lifecycle dependen del embedder.
Fuentes: [arquitectura Flutter](https://docs.flutter.dev/resources/architectural-overview),
[plataformas](https://docs.flutter.dev/reference/supported-platforms),
[Android views](https://docs.flutter.dev/platform-integration/android/platform-views),
[iOS views](https://docs.flutter.dev/platform-integration/ios/platform-views).

## Bridges evaluados

| Criterio | flutter_rust_bridge | C ABI + dart:ffi manual | Platform channels + capa nativa | API C generada (cbindgen) |
| --- | --- | --- | --- | --- |
| Android/iOS/Win/Linux/macOS | Documentados por upstream; no equivalen a nuestros builds | Dart FFI nativo, linker por OS | Embedder/plugin por OS; Swift/Kotlin/C++ adicionales | ABI C consumible por esos toolchains; integración Dart aparte |
| Ownership | Wrappers/opaque y disposal; reglas del generador | Handles numéricos privados, buffers propiedad del caller, destroy explícito | Objetos de SDK permanecen nativos | Genera declaraciones, no gestión de lifetime |
| Async | Runtime/executor y futures integrados | Worker + polling de estado; manual | Future/event channels; SDK en su hilo | No aporta runtime |
| Callbacks/eventos | Streams generados | Cola/polling o callbacks FFI con disciplina | EventChannel/listeners nativos | Solo firmas; implementación aparte |
| Threading | Tipos async/opaque deben satisfacer las reglas del executor; revisar versión | Ningún SDK cruza ABI; actor/owner necesario para !Send | Main thread/task queues definidos por embedder | No convierte !Send en Send |
| Errores | Traducción generada Result | Codes/envelope versionado explícitos | PlatformException requiere normalización | DTOs deben diseñarse previamente |
| Cancelación | No inferir cancelación de decoder/future al desechar wrapper | Token cooperativo/generation, probado en Rust | Intents a capa nativa; callbacks viejos invalidables | No incluida |
| Build | Codegen Dart/Rust y versiones coordinadas | cdylib/NDK, bindings pequeños mantenidos manualmente | Plugin + JNI si Rust queda detrás del nativo | Herramienta adicional y header reproducible |
| Mantenimiento | Buen candidato si la API crece; dependencia del generador | Menos tooling hoy, más revisión unsafe y evolución ABI propia | Duplica glue por plataforma, útil para surfaces/SAF | Evita drift del header; no elimina JSON/DTO ni errores de uso |

Fuentes primarias: [FRB concurrencia](https://cjycode.com/flutter_rust_bridge/guides/concurrency/overview),
[FRB opaque](https://cjycode.com/flutter_rust_bridge/guides/types/arbitrary/rust-auto-opaque),
[Dart FFI](https://dart.dev/interop/c-interop),
[canales Flutter](https://docs.flutter.dev/platform-integration/platform-channels),
[cbindgen](https://github.com/mozilla/cbindgen).
No se expondrán structs cine-core ni punteros de Player. Codegen tampoco justifica
exponer el dominio: siempre se necesita una API Application con DTOs de frontera.

## Android: evaluación de Players

| Criterio | libmpv | Media3/ExoPlayer | libVLC | GStreamer |
| --- | --- | --- | --- | --- |
| Integración | Port mpv-android comunitario, JNI/NDK y cierre de dependencias | SDK AndroidX oficial y módulos Maven | SDK/JNI VideoLAN | SDK Android NDK oficial, pipeline y JNI |
| Surface | Ventana/Surface + render API por integrar | SurfaceView/TextureView/Surface y PlayerView | Surface/drawable vía wrapper | Video overlay/sinks y bridge |
| Thread | SDK owner y eventos, wrapper nuestro !Send | Un application Looper; UI exige main | Callback manager + wrapper Android | Bus/GMainContext + hilo UI |
| Lifecycle/background | Debe construirse el adapter | Activity/MediaSession/Service; este spike pausa en background | Integración Android específica | Integración específica |
| Seek/completion | Exact/keyframe, restart; no frame físico | seekTo, discontinuity y READY/rendered-first-frame separados | Eventos de tiempo; medir latencia real | Seek/evento ASYNC_DONE según pipeline |
| Rate/position | speed y time-pos, viable upstream | PlaybackParameters(speed,pitch), currentPosition en ms | setRate condicionado por medio | Seek rate y query de posición |
| Hardware | Build/hwdec/surface determinantes | MediaCodec del dispositivo; inspeccionar decoder elegido | Plugins/build/dispositivo | Plugins MediaCodec y SDK |
| URI/SAF | FD/stream callback/adapter, content URI no es path | DataSource/ContentResolver para content URI | JNI Android URI/FD, probar proveedor | Source/FD/bridge, probar proveedor |
| Packaging | Muchos .so, NDK y ABIs | Java/Kotlin + codecs del SO para este corpus | libVLC + plugins/ABIs | SDK/plugins seleccionados, no framework entero |
| Licencia | Engine default GPL, LGPL build condicionado | Apache-2.0; formatos/patentes por separado | Engine LGPL, plugins/contrib revisar | LGPL y cierre plugins/contrib |

Fuentes: [mpv Android](https://github.com/mpv-android/mpv-android),
[Media3 threading y surface](https://developer.android.com/media/media3/exoplayer/hello-world),
[Media3 eventos](https://developer.android.com/media/media3/exoplayer/listening-to-player-events),
[PlaybackParameters](https://developer.android.com/reference/androidx/media3/common/PlaybackParameters),
[libVLC](https://www.videolan.org/vlc/libvlc.html),
[SDK GStreamer Android](https://gstreamer.freedesktop.org/documentation/installing/for-android-development.html).
Media3 no sustituye todos los codecs de mpv: el corpus H264/AAC permite validar
el puerto; archivos MKV/subtítulos/codecs exóticos necesitarán otra matriz.

## Android: archivos y suspensión

ACTION_OPEN_DOCUMENT retorna content:// y permiso de lectura; persistir solo si
el proveedor/flags lo permiten. No resolverlo a /storage. ContentResolver abre
un ParcelFileDescriptor; un FD duplicado puede transferirse a worker Rust,
reteniendo permiso y cerrando ambos lados explícitamente. Proveedores pueden dar
pipes/medios remotos: este prototipo puede limitarse a FD regular seekable y
rechazar el resto; streaming/copia temporal son decisiones futuras.
Hash SHA-256 por bloques reutiliza LocalMedia; solo identidad final permite una
futura declaración Ready. URI/FD permanecen privados al dispositivo.
Fuente: [SAF oficial](https://developer.android.com/training/data-storage/shared/documents-files).

Background invalida reloj/deadlines/corrección y pausa/restaura rate. Resume no
rehabilita sync automáticamente: requiere clock resync + snapshot en integración
futura. Este spike aislado no simula autoridad de sala en Dart.

## iOS: comparación y recursos

| Criterio | AVPlayer | libmpv | libVLC/MobileVLCKit | GStreamer |
| --- | --- | --- | --- | --- |
| Integración/surface | AVPlayerLayer/AVPlayerViewController; ownership Apple | Build/embed Apple y render API; no validación propia | Wrapper ObjC/framework VideoLAN | SDK/framework oficial iOS |
| Seek | Tolerancias y completion handler, cancelar seeks pendientes | Restart/position no prueba frame | Eventos wrapper, precisión por medir | ASYNC_DONE según pipeline |
| Rate | rate/capacidades AVPlayerItem; pitch/timePitchAlgorithm según SDK | speed/audio dependerá del build | Rate según input/SDK | Pipeline/elementos |
| Position/events | CMTime, timeControlStatus, periodic observer que debe retirarse | Cola de eventos SDK | Delegate/events | Bus/pipeline |
| Hardware | AVFoundation/VideoToolbox bajo control del SO | VideoToolbox/build/surface | Plugins/contrib | Plugins Apple |
| Archivos | Document picker, URL security-scoped y bookmarks | FD/URL autorizado con lifetime externo | Recurso autorizado en wrapper | Source con recurso autorizado |
| Suspensión | Notification/scene, audio interruption y lifecycle | Lifecycle externo | Lifecycle externo | Lifecycle externo |
| Packaging | SDK nativo; no runtime GPL incluido | Cierre codec/build/linking complejo | XCFramework/SDK, revisar relinking | Framework/plugins y linking |
| Distribución | APIs públicas/entitlements/review siguen aplicando | GPL/LGPL/build y tienda: riesgo abierto | LGPL/plugins/relinking: revisión | LGPL/plugins/relinking: revisión |

Fuentes: [AVPlayer](https://developer.apple.com/documentation/avfoundation/avplayer),
[guía Apple de playback](https://developer.apple.com/library/archive/documentation/AudioVideo/Conceptual/AVFoundationPG/Articles/02_Playback.html),
[seek Apple](https://developer.apple.com/documentation/avfoundation/avplayer/seek(to:tolerancebefore:toleranceafter:completionhandler:)),
[recursos security-scoped](https://developer.apple.com/documentation/foundation/nsurl/startaccessingsecurityscopedresource()),
[document picker](https://developer.apple.com/documentation/uikit/providing-access-to-directories),
[VLCKit upstream](https://github.com/videolan/vlckit),
[GStreamer iOS](https://gstreamer.freedesktop.org/documentation/installing/for-ios-development.html).
Apple SDK documentado no equivale a runtime validado. Desde Linux, sin macOS,
Xcode/signing/dispositivo, estado: BLOCKED BY REQUIRED TOOLCHAIN. No hacks.
Abrir y retener security scope durante lectura/reproducción, coordinar cambios
si el proveedor los permite; cerrar scope/observer con lifecycle. Background
no puede preservar deadlines de red: misma invalidación y recovery que Android.

## Render embebido y capacidades

Desktop mpv requerirá render API/contexto OpenGL o superficie nativa con owner;
no se presume que Flutter Texture posea el SDK. Android PlatformView puede
poseer SurfaceView; iOS UiKitView puede poseer AVPlayerLayer. Texture reduce
ciertos costes de composición pero exige gestión de frames/textures/lifecycle.
No se decide un render universal sin pruebas propias.
Capacidades mínimas para este spike: playback_rate y content_uri_input. Precisión
de seek/hardware son resultados observables, no booleanos que prometen frames
exactos. SyncEngine ya admite supports_rate; no se cambian sus thresholds.

## Licencias/App Store — TECHNICAL RISK, LEGAL REVIEW REQUIRED

[Copyright mpv](https://github.com/mpv-player/mpv/blob/master/Copyright): API header
ISC no relicencia engine; default GPL y LGPL condicionado por fuentes/build.
FFmpeg local GPL/version3 permanece solo herramienta/engine Linux experimental;
no se copia ese cierre a un APK/IPA. [FFmpeg legal](https://ffmpeg.org/legal.html).
LibVLC/VLCKit y GStreamer LGPL requieren revisar plugins y build concreto,
avisos/fuentes y posibilidad de sustituir/relinkear biblioteca. Linking estático
puede requerir objetos relinkables; una tienda o firma no demuestra cumplimiento.
[LGPL 2.1 sección 6](https://github.com/FFmpeg/FFmpeg/blob/master/COPYING.LGPLv2.1)
y [licencias GStreamer](https://gstreamer.freedesktop.org/documentation/application-development/appendix/licensing.html).
Restricciones de distribución/DRM/firmado deben contrastarse con términos Apple y
licencia concreta; no se emite conclusión legal universal GPL/LGPL ↔ App Store.
[App Review](https://developer.apple.com/app-store/review/guidelines/) y
[acuerdo Apple](https://developer.apple.com/support/terms/apple-developer-program-license-agreement/).
Media3 es [Apache-2.0](https://github.com/androidx/media/blob/release/LICENSE),
Flutter BSD-3-Clause, FRB MIT y cbindgen MPL-2.0 según repositorios respectivos.
Licencias del wrapper no sustituyen SDK/codecs. Cine Virtual continúa sin LICENSE,
sin releases ni contribuciones públicas solicitadas.

## Gate provisional

Decisión previa al runtime: Flutter es candidato razonable. Evidencia posterior
y clasificación final en [RESULTS](RESULTS.md). Prototipo con
**C ABI pequeña + Dart FFI** para comandos/observaciones y **platform channels**
para SAF/surface/Media3. Application de experimento Rust posee SyncEngine y estado
de lifecycle; nativo posee Player/permiso/surface; Dart presenta y transporta
intents/efectos. Se mantiene como API experimental v1, no ABI pública de producto.
FRB queda alternativa si streams/API crecen: no se necesita integrar dos bridges.
Android candidato **Media3**; iOS **AVPlayer**, solo investigado. Desktop libmpv
PROVISIONAL FOR LINUX. No se descartan VLC/GStreamer/mpv móvil universalmente;
se difieren por coste de cierre y falta de ventaja para este gate H264/AAC.
