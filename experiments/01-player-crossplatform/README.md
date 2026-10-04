# 01 — Spike B: Player multimedia real

**Estado: IMPLEMENTADO y PROBADO en Linux; previo a v0.1.** Candidato provisional:
libmpv, clasificación D: suficiente para continuar al vertical slice en Linux.
No es una elección definitiva y no se ha compilado/ejecutado en móvil.

## Objetivo e investigación

Responder si el puerto Player puede controlar vídeo real in-process, midiendo
dispatch, completion y timeline antes de combinarlo con red. La comparación de
libmpv/libVLC/GStreamer/players nativos, licencias y gate previo al código está en
[RESEARCH](RESEARCH.md). FFmpeg se usa como generador/inspector, no como Player.

El plan original de este experimento era dos clientes con vídeo local. Spike A
validó primero el control con FakePlayer; Spike B aísla multimedia real + Core.
El recorrido con dos vídeos/red queda para el **siguiente vertical slice**, junto
con metadatos, hashing y Ready reales. Ese recorrido no se ejecutó en Spike B; se integró después en 06.

## Implementación y arquitectura

Un solo crate [cine-player-mpv](../../adapters/player-mpv/Cargo.toml): FFI acotado
al API C 2.x, carga dinámica mediante libloading y ejecutable cine-player-spike.
Implementa el [puerto existente](../../core/src/player.rs); load, eventos y handle
local pertenecen al adapter. No se modifica la firma del trait. PlayerError añade
Display/Error de std para propagación tipada en Application, sin dependencia SDK.
Core continúa sin dependencias externas; rooms/protocol/server/client no cambian.

El propietario es un solo thread y el tipo deliberadamente no es Send/Sync.
mpv tiene threads internos; el propietario drena wait_event con batches acotados,
copia datos antes del próximo poll y no instala callbacks. Un seek a la vez:
seek dispatch → SEEK → PLAYBACK_RESTART → SeekCompleted → comprobar posición.
Loaded corresponde al evento del motor; el harness espera restart y propiedades
consultables antes de considerar media ready. Playing significa pause=false;
la medición además espera avance de position. No es una medida del frame físico.

load reemplaza el medio solo después de stop/idle y drenaje de eventos anteriores.
Drop/destroy termina el core/decoders antes de descargar la biblioteca. destroy
es idempotente y llamadas posteriores devuelven errores. Las llamadas SDK
síncronas pueden bloquear: los timeouts del harness acotan la espera de eventos,
no interrumpen una llamada C en curso. La ventana/surface de este smoke pertenece
al SDK; un render context embebido tendrá ownership/threading propio por validar.
El CLI manual usa una
cola stdin de 16 comandos; el reader se une tras quit/EOF.

## Instalación y build

Entorno validado: CachyOS/Arch x86_64, Rust 1.99, mpv 0.41.0 (API 2.5), FFmpeg n9.0.2.
**No se instalaron paquetes del SO**: mpv/libmpv, headers, ffmpeg/ffprobe, PipeWire,
pactl y libX11 ya existían. Para otro Arch que carezca de ellos:

```sh
sudo pacman -S mpv ffmpeg
cargo build --workspace
```

La carga dinámica hace que build y tests habituales no requieran libmpv en
link-time. Los tests opt-in y el ejecutable sí necesitan el SDK al ejecutarse.
El paquete local enlaza FFmpeg GPL/version3: no es un build aprobado para distribuir.
Audio aislado requiere pactl; captura visible auxiliar necesita libX11/x11grab.

## Corpus reproducible

```sh
python3 scripts/generate_test_media.py
```

Todo se genera en test-media/, ignorado por Git. No se descargó contenido.
H.264/libx264 y AAC estaban disponibles; si faltan, el script falla explícitamente.
normal.mp4 (30 s, GOP 30), long-gop.mp4 (30 s, GOP 300),
variable-framerate.mkv (30,021 s, intervalos 33/34/66/67 ms), audio-video.mp4
(30 s, GOP 60), long-duration.mp4 (620 s, 160×90/15 fps, GOP 150).
Los demás tienen 320×180/30 fps nominales. Todos incluyen seno sintético 440 Hz,
48 kHz. manifest.json contiene ffprobe y deltas de frames; también se generan
empty.mp4, corrupt.mp4 y unsupported.bin. missing no se crea.

## Comandos automáticos

```sh
mkdir -p experiments/01-player-crossplatform/runs
cargo run -p cine-player-mpv --bin cine-player-spike -- test-media/normal.mp4 --output experiments/01-player-crossplatform/runs/normal.json
cargo run -p cine-player-mpv --bin cine-player-spike -- test-media/long-gop.mp4 --output experiments/01-player-crossplatform/runs/long-gop.json
cargo run -p cine-player-mpv --bin cine-player-spike -- test-media/variable-framerate.mkv --output experiments/01-player-crossplatform/runs/vfr.json
CINE_SPIKE_CLOCK_TICKS=$(getconf CLK_TCK) target/debug/cine-player-spike test-media/long-duration.mp4 --long 600 --output experiments/01-player-crossplatform/runs/long.json
cargo test -p cine-player-mpv --test real_player -- --ignored --test-threads=1 --nocapture
python3 scripts/measure_player_audio.py
```

Automatic usa corpus de al menos 25 s y ejecuta carga, pausa, seek 5 s, play,
sampling, pausa, seek 20 s, rate 0.98/1/1.02/0.95/1.05/1, soft/hard correction,
seek cerca del final y EOF. Targets se acotan a duración. El ensayo largo reproduce
600 s reales, sin reloj acelerado; samples a ~500 ms y SyncEngine por defecto.
CINE_SPIKE_CLOCK_TICKS permite reportar CPU Linux con frecuencia confirmada;
sin ella el nuevo harness deja CPU como no medida. El run registrado usó 100 Hz,
confirmado por getconf. CPU está expresada respecto de un core.

## Manual, visible y audio

```sh
target/debug/cine-player-spike test-media/normal.mp4 --manual
target/debug/cine-player-spike test-media/audio-video.mp4 --visible --hwdec --manual
```

Comandos: load <path>, play, pause, seek <ms>, position, duration, rate <value>,
state, quit. Headless por defecto: vo=null, ao=null, decoding real, software.
Visible usa una ventana del SDK y audio real; no constituye rendering embebido
para Flutter. En este entorno también se probó gpu-context=x11egl sobre Xwayland:

```sh
target/debug/cine-player-spike test-media/audio-video.mp4 --visible --gpu-context x11egl --hwdec --output experiments/01-player-crossplatform/runs/visible.json
# Mientras la ventana reproduce, en otra terminal:
python3 scripts/capture_player_window.py
```

La captura auxiliar limita x11grab al ID de la ventana del SDK, nunca al desktop;
visible-window.png y visible-frame.png quedan ignorados. Audio automatizado usa
un sink temporal aislado y lo elimina al terminar; no captura micrófono ni audio
privado. El run visible medido usó un sink aislado creado con pactl y
--audio-device pulse/cine_spike_b, eliminado posteriormente. Sin ese flag el modo
manual usa el dispositivo habitual. --diagnostic-logs activa logs upstream solo
para diagnóstico explícito: pueden contener el path local, no incluirlos en informes.

## Pruebas, métricas y resultados

Resultados y límites de medición en [RESULTS](RESULTS.md); evidencia machine-readable
en [results-linux.json](results-linux.json). Se registran versión, plataforma,
propiedades de corpus, carga, play/pause, seeks, samples, rates, correcciones,
audio, recursos, errores probados y tiempo real de ejecución, sin rutas personales.
[scripts/collect_player_results.py](../../scripts/collect_player_results.py)
consolida runs revisados; no inventa valores ausentes. Los SDK tests son opt-in y
usan timers reales; los tests deterministas Core/Spike A conservan sus fake clocks.

## Problemas, límites y conclusiones

Se corrigieron selección errónea de ao=auto (driver inexistente), atribución de
errores de playback a load y protección frente a eventos del medio anterior.
SeekCompleted no prueba presentación del frame ni salida de audio física. La
posición está cuantizada y rate de ventanas cortas tiene error de muestreo.
Los defaults de SyncEngine funcionan, pero aparecen cambios de rate alrededor
de la deadband; necesitan tuning con vídeo real y más corpus. RSS retenido tras
lifecycle no se interpreta automáticamente como fuga o como ausencia de fugas.

Linux/software y Linux/visible/VAAPI tienen evidencia. No se ejecutó Windows,
macOS, Android ni iOS; tampoco URI móviles, interrupciones/background, surfaces
embebidas ni distribución LGPL/App Store. No se cambian Flutter, bridge ni licencia.
Decisión afectada: [ADR-006](../../docs/DECISIONS.md), todavía provisional.

## Próximos pasos

Vertical slice Linux con control autoritativo existente + dos Player reales,
load/probe/hash completo/Ready, scheduled controls y resume. Mantener otro gate
obligatorio para dispositivos Android/iOS y packaging/licencias antes de promover
un stack multimedia definitivo. Este spike no termina v0.1.

## Integración posterior: vertical slice 1

Este documento conserva la evidencia histórica del spike aislado. El recorrido
posterior de red + dos Player reales + LocalMedia/probe/hash/Ready se registra en
[06-real-vertical-slice](../06-real-vertical-slice/README.md), sin declarar v0.1
terminado ni promover libmpv fuera de su condición provisional Linux. ADR-005
ahora registra hashing real; móviles, UI/bridge y licencia siguen pendientes.
