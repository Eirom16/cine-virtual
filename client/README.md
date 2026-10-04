# Cliente CLI experimental

Instrumento pre-v0.1: WebSocket real, clock sync, réplica/scheduler genéricos,
FakePlayer y libmpv Linux provisional con owner thread. No hay UI. Flutter y el puente siguen provisionales.

```sh
cargo run -p cine-client -- --server ws://127.0.0.1:8765 --name Host
```

Abrir otro proceso con `--name Participant`. Comandos:

```text
create
join <room_id> <room_epoch> <invite_token>
select <ruta local completa>
media
hash-status
player-state
sync-state
media-demo
ready
play [position_ms]
pause
seek <position_ms>
state
sync
disconnect
resume
leave
quit
```

`create` muestra una invitación privada en stdout; copiar los tres campos a
`join`. No copiarla a logs o informes. Tras `media-demo` del Host, ambos ejecutan
`ready`. El reloj reúne ocho muestras antes de admitir operación. Una orden
aceptada todavía espera el deadline: consultar `state` o los logs de ejecución.
`resume` recalibra, rota credenciales, instala snapshot y verifica/prepara de
nuevo el contenido local retenido o el sintético si se usa FakePlayer. No reenvía controles anteriores ni reintenta solo.

`Replica` y `FakePlayer` reciben tiempo numérico inyectable y se prueban sin red
ni timers reales. `runtime` compone el adapter WS y los timers Tokio; no define
la API FFI/UI futura. La salida JSON de comandos va a stdout; tracing redactado
va a stderr. Solo acepta endpoints ws:// loopback en este experimento.

Flujo y resultados en [Spike A](../experiments/03-websocket-sync/README.md).
Contratos en [ARCHITECTURE](../docs/ARCHITECTURE.md) y
[PROTOCOL](../docs/PROTOCOL.md). Plataformas objetivo siguen siendo cinco; aquí
se verificó únicamente Linux, sin toolchains móviles.

## Backend real provisional Linux

```sh
cargo run -p cine-client -- --server ws://127.0.0.1:8765 --name Host --player mpv
# Ventana/audio SDK para smoke manual (no UI de producto):
cargo run -p cine-client -- --player mpv --visible true
```

Requiere libmpv instalado; ffprobe es opcional si libmpv obtiene duración usable.
select acepta el resto de la línea como ruta (incluidos espacios, sin comillas).
Host select prepara/hash completo/carga pausada y publica descriptor sin ruta;
Participant select prepara su archivo y ready declara **su propia identidad**.
Un archivo distinto produce MEDIA_MISMATCH. media/hash-status y tracing de
progreso no exponen digest. Mientras select está pendiente, la CLI espera ese
comando y el runtime de red continúa; no hay barra/TUI ni cancelación interactiva.

`--player fake` es el defecto y conserva media-demo. `fault pause`, `fault clock
<ms>` y `fault seek <ms>` son diagnósticos locales explícitos, nunca comandos de
sala. player-state/sync-state muestran muestra/edad y target autoritativo.
Demo y mediciones en
[vertical slice 1](../experiments/06-real-vertical-slice/README.md).
