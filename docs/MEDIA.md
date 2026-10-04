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

Implementado en el vertical slice 1: sha2/RustCrypto, lectura streaming por bloques
de 1 MiB, memoria O(1), worker fuera del runtime de red, progreso y cancelación
por callback local. Límite inicial 1 TiB/7 días de duración, fijo en v1 y
anunciado en sesión. Core compara identidades calculadas fuera de él; no depende
del filesystem.
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

## LocalMedia real del vertical slice

[cine-local-media](../adapters/local-media/README.md) abre un archivo regular no
vacío (≤1 TiB), retiene FD/handle y ruta canónica solo en el dispositivo, lee
bloques de 1 MiB y produce tamaño+SHA-256 final. No usa cache de identidad por
path/mtime. Dentro de la sesión se retienen el digest realmente calculado y el
handle; Ready/resume vuelven a comprobar estabilidad del handle y pathname.
Se detectan tamaño/mtime y, en Unix, device/inode/ctime antes/después de leer y
antes de declarar Ready. No prueba inmutabilidad: cambios posteriores a Ready
no tienen watcher automático y un filesystem mutable conserva TOCTOU, incluido
el momento en que mpv abre por pathname. Rehash explícito mediante select si cambia.

ffprobe es inspector auxiliar detrás del adapter: solo duración/format/codecs,
JSON ≤64 KiB, timeout 10 s con kill/reap, stderr descartado y sin shell. Si falta
o falla, se admite metadata mínima de libmpv después de FILE_LOADED +
PLAYBACK_RESTART; duración positiva acotada es obligatoria, MIME/codecs son
opcionales. Si existe probe, ambas duraciones deben diferir ≤1000 ms. Título se
omite para no enviar el nombre del archivo. El proceso ffprobe no reproduce:
libmpv sigue in-process.

Host select prepara localmente y envía MEDIA_SELECT_REQUEST. Participant select
prepara su copia sin cambiar la selección de sala. Ready envía su propia
MEDIA_METADATA; el servidor verifica, emite MEDIA_VERIFIED y recién entonces
admite MEDIA_READY. MEDIA_VERIFIED nunca se inventa como comando de cliente.
Readiness requiere hash final, revisión vigente, Player cargado/no-error,
duración usable, no seek/buffering y reloj confiable. Buffering/error posterior
o pérdida de confianza del reloj informa MEDIA_NOT_READY (reason=user para
reloj); requiere Ready explícito tras recuperarse.

No se implementan URI Android, security-scoped resources Apple, streaming ni
pipeline P2P. libmpv sigue provisional Linux y móviles sin validar.
