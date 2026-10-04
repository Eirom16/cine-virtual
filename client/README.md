# Cliente CLI del Spike A

Instrumento pre-v0.1: WebSocket real, clock sync, réplica/scheduler y FakePlayer.
No hay UI ni decodificación multimedia. Flutter y el puente siguen provisionales.

```sh
cargo run -p cine-client -- --server ws://127.0.0.1:8765 --name Host
```

Abrir otro proceso con `--name Participant`. Comandos:

```text
create
join <room_id> <room_epoch> <invite_token>
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
nuevo el contenido sintético. No reenvía controles anteriores ni reintenta solo.

`Replica` y `FakePlayer` reciben tiempo numérico inyectable y se prueban sin red
ni timers reales. `runtime` compone el adapter WS y los timers Tokio; no define
la API FFI/UI futura. La salida JSON de comandos va a stdout; tracing redactado
va a stderr. Solo acepta endpoints ws:// loopback en este experimento.

Flujo y resultados en [Spike A](../experiments/03-websocket-sync/README.md).
Contratos en [ARCHITECTURE](../docs/ARCHITECTURE.md) y
[PROTOCOL](../docs/PROTOCOL.md). Plataformas objetivo siguen siendo cinco; aquí
se verificó únicamente Linux, sin toolchains móviles.
