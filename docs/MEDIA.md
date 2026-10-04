# Multimedia e identidad de contenido

## Descriptor portable

MediaDescriptor pertenece al dominio y nunca incluye ruta/URI local. `media_id`
UUIDv4 identifica selección, no bytes. `source_type` expresa origen; `title`
opcional describe sin revelar nombre privado; `duration_ms`, `identity`, `mime`
y `codecs` son metadatos acotados. `identity` v1 es tamaño+SHA-256 completo.
Schema wire exacto en [PROTOCOL](PROTOCOL.md); tipo Rust en
[media.rs](../core/src/media.rs). RoomState vincula descriptor a media_revision.

El enum Rust reserva LocalFile/P2PFile/HTTP/HLS/DASH/Jellyfin/Provider, pero su
validación v1 solo acepta LocalFile. No hay adapters de fuentes futuras. Más
adelante distintos providers podrán usar identificadores y capacidades diferentes;
stream live no tiene necesariamente hash completo ni duración fija.

`LocalMediaHandle` diseñado para Application/adapter: ruta, URI Android o recurso
Apple autorizado, con lifetime/permiso local. Se resuelve a lectura/Player sin
enviarse al servidor. Media resolver e inspector viven fuera del Core. Metadata
no convierte al servidor en un procesador de archivos privados.

## Comprobación exacta inicial

Elección: SHA-256 de todos los bytes + tamaño. Es una garantía práctica bajo la
resistencia a colisiones del algoritmo, no una demostración matemática ni prueba
de posesión contra un cliente malicioso. Archivos con distinto contenedor/tags
no coinciden aunque el vídeo visible sea igual; esto es deliberado en v0.1.

| Opción | Ventaja | Límite | Uso inicial |
| --- | --- | --- | --- |
| Hash completo | Comparación de todos los bytes sencilla | O(n) de I/O antes de Ready | Elegida |
| Hash progresivo completo | I/O por bloques acotados, progreso/cancelación | No hay identidad final hasta EOF | Forma de calcular el hash completo |
| Fingerprint parcial | Rápido como prechequeo | Puede omitir diferencias y aceptar falso positivo | No autoriza Ready |
| Hash por chunks/Merkle | Verificación de descarga/reanudación por partes | Manifest y protocolos adicionales | Reservado P2P |

Implementación futura: biblioteca SHA-256 mantenida, lectura streaming por bloques
de 1 MiB, memoria O(1), worker fuera del hilo UI, progreso y cancelación. Límite
inicial 1 TiB/7 días de duración, configurable pero anunciado en sesión. No hay
hashing de archivos implementado: Core compara identidades ya calculadas.
No escribir una implementación propia de SHA-256 para ahorrar una dependencia.

Verificar tamaño/identidad del handle antes y después de leer; fallo o modificación
invalida claim. Cache local opcional por handle/tamaño/mtime solo acelera un
prechequeo: mtime no prueba integridad. Si archivo cambia después de Ready,
MEDIA_NOT_READY y nuevo hashing. Un filesystem no inmutable mantiene riesgo
TOCTOU; retener handle o invalidar al detectar cambios, documentar límite.

Hash local no se publica en logs: identifica contenido y puede ser sensible.
La sala recibe digest para comparar; no contenido. El hash no es credencial,
no autoriza transferencia y no reemplaza permisos.

## Elección definitiva de Player pendiente

| Candidato | Qué comprobar en spike |
| --- | --- |
| libmpv | Embedding, superficie, precisión de seek/rate, empaquetado y builds móviles |
| libVLC | Bindings nativos, precisión/latencia, tamaño y comportamiento rate móvil |
| GStreamer | Pipelines, relojes, plugins/decoders y complejidad de distribución |
| Players nativos por plataforma | AVPlayer/Media3 y equivalentes desktop, formatos compatibles y consistencia del puerto |
| FFmpeg | Inspector/decodificación si hace falta; no sustituye por sí solo una integración completa de reproducción |

Los documentos de [libmpv](https://mpv.io/manual/stable/) describen embedding;
[VideoLAN](https://www.videolan.org/vlc/libvlc.html) declara APIs de libVLC para
escritorio y móvil. GStreamer documenta integración en
[iOS](https://gstreamer.freedesktop.org/documentation/installing/for-ios-development.html)
y [Android](https://gstreamer.freedesktop.org/documentation/installing/for-android-development.html).
Esto indica opciones a ensayar, no valida nuestra aplicación en esos dispositivos.

El spike registra capacidades reales: codecs/contenedores de prueba, aceleración,
rate con audio, seek exacto/keyframes, buffering, posición con timestamp,
superficie, suspensión, archivos grandes y URI protegido. Se permite una familia
de adapters por plataforma; un único SDK no es requisito arquitectónico.
Licencias de SDKs/codecs dependen del build: [FFmpeg](https://ffmpeg.org/legal.html)
explica componentes LGPL/GPL. Revisar las opciones efectivamente enlazadas antes
de distribuir. Ver ADR-006 y [LICENSING](LICENSING.md).

## Evidencia Spike B

[Experimento 01](../experiments/01-player-crossplatform/README.md) implementa un
adapter libmpv provisional Linux: in-process, load/eventos locales fuera de Player,
seek dispatch/completion separados, headless y ventana SDK visible. SyncEngine
aplica rate/seek reales sin red. No se cambia MediaDescriptor ni se implementa
hashing. Solo Linux tiene build/runtime; Android/iOS y rendering embebido siguen
pendientes. [ADR-006](DECISIONS.md) conserva el historial y gates de promoción.
