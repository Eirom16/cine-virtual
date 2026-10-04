# Vertical slice 1 — Reproducción real sincronizada

Integra fundaciones y Spikes A/B, sin rediseñarlos: servidor real, dos clientes
Linux reales, WebSocket v1, LocalMedia/probe/SHA-256 final, Ready, clock/scheduler,
SyncEngine y libmpv in-process. No es un nuevo spike aislado ni v0.1 terminado.
libmpv permanece **PROVISIONAL FOR LINUX**, móviles sin build/runtime validado.

## Organización y requisitos

`adapters/local-media` pertenece al dispositivo; nunca a rooms/server. `client`
compone Replica genérica, FakePlayer o proxy con un thread propietario mpv. Una
sola política/scheduler; Core y sus defaults no se modifican. El codec/RoomService
mantienen MEDIA_METADATA → MEDIA_VERIFIED servidor → MEDIA_READY.

Linux con libmpv (validado mpv 0.41.0, API 2.5), Rust/Cargo, Python3 y ffmpeg/ffprobe
para generar corpus. Build/tests usuales cargan SDK dinámicamente solo si se pide
mpv; FakePlayer sigue ejecutable sin libmpv. No se instalaron nuevos paquetes.

```sh
cargo build --workspace
python3 scripts/generate_test_media.py
python3 scripts/demo_control.py
python3 scripts/demo_real_media.py
# Diagnóstico corto con fallos; no reemplaza la prueba de 10 minutos:
python3 scripts/demo_real_media.py --seconds 25 --faults
cargo test -p cine-player-mpv --test real_player -- --ignored --test-threads=1 --nocapture
cargo test -p cine-client --test replica -- --ignored --test-threads=1 --nocapture
```

## Recorrido reproducible

Demo inicia cine-server loopback/puerto efímero y dos cine-client --player mpv.
Crea/join, carga corpus sintético long-duration.mp4 (620 s, 160×90, 15 fps, H264/AAC),
rechaza Ready antes de load y compara una copia con bytes añadidos. Mismatch
rechazado sin Ready. Ambos seleccionan el mismo archivo, hashean 1 MiB/bloque,
cargan pausados y declaran metadata/Ready. Participant no puede controlar y no
consume sequence. Host ejecuta Play, Pause, Seek pausado y Seek playing. B
se desconecta, A continúa; B recalibra, resume con snapshot, revalida handle/digest
retenidos y vuelve a Ready/converge. Termina pausado y cierra/reapea los procesos.

El escenario largo conserva 600 segundos de reloj real, sin acelerar mpv. La
segunda ejecución corta inyecta seek pequeño/grande, pausa local y confianza de
reloj suspendida (Ready invalidado y recuperado explícitamente). No finge buffering de red/seek lento: se documentan como pendientes.
No usa reproductor externo; ffprobe es solo inspector. Todo corpus está ignorado
por Git, incluida la copia modificada, eliminada al cerrar la demo.

## Métricas y límites

[RESULTS](RESULTS.md) explica método y resultados. [results-linux.json](results-linux.json)
contiene muestras, eventos SDK, dispatch, lifecycle y recursos del ensayo largo;
[results-faults-linux.json](results-faults-linux.json) conserva fallos/transitorios.
Credenciales/digests/rutas no se exportan. CLI privada create muestra invitación
solo para join. Se guardan todas las muestras; percentiles incluyen todas las muestras del Player cargado, pausado y
transitorios de preparación, excluyen solo reloj no confiable/buffering/seek activo/
muestra >100 ms o no cargada. Deadline perdido: dispatch **primario** >50 ms;
RPC de reconciliación posterior no es un nuevo control programado.

Headless usa decoding real y outputs null/software; no demuestra frame presentado,
rendering/audio sincronizados o red WAN. Visible/audio fueron probados aisladamente
en Spike B; este recorrido no los sustituye. Ready es declaración del cliente,
no prueba contra un cliente malicioso. TOCTOU del archivo mutable y bloqueos C
no están resueltos. Meta p95 ≤150 ms experimental, sin ajustar thresholds para pasar.

## Próximo gate

Validación móvil de Player/URI/lifecycle/packaging/licencias antes de elegir motor
universal/bridge. Ampliar corpus y red medida para el slice Linux sin promover
libmpv o v0.1. ADR-005 registra hashing real; ADR-006 conserva historial y provisionalidad.
UI/Flutter, licencia, distribución y versiones futuras siguen pendientes.
