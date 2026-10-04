# Servidor WebSocket del Spike A

Ejecutable experimental Axum/Tokio para control autoritativo de salas en memoria.
Sin DB, multimedia, cuentas ni TLS. El binario solo permite bind loopback.

```sh
cargo run -p cine-server -- --bind 127.0.0.1:8765
```

También puede ejecutarse `target/debug/cine-server` después de compilar; rutas
WebSocket `/` y `/ws`. El puerto 0 solicita uno libre y lo informa por stderr.

Adapter: handshake v1, codec validado, heartbeat 5 s/timeout 15 s, identidad de
sesión, colas 32, conexiones 256, salas 128 y rate 20/s con burst 40. Los mensajes
no superan 64 KiB, las salas no superan 16 miembros. Una cola llena desconecta
al cliente lento; el servicio mantiene su lease de 30 s para resume con snapshot.

RoomService y RoomStore viven en `cine-rooms`, separados de Axum/Tokio/JSON.
Reciben comandos y tiempo monotónico; devuelven efectos. El adapter serializa
mutación y encolado mediante un mutex corto, sin await dentro. Reiniciar pierde
salas/tokens y cambia clock_epoch. Logs JSON no incluyen tokens, hashes ni frames.

No está listo para Internet: faltan WSS, políticas de origins/origen y pruebas
de abuso/carga. Ver [SECURITY](../docs/SECURITY.md),
[PROTOCOL](../docs/PROTOCOL.md) y [resultados](../experiments/03-websocket-sync/README.md).
