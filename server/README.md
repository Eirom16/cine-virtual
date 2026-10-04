# Servidor futuro

No hay servidor ejecutable ni implementación Axum/Tokio. Candidato: un proceso
Rust, WebSocket JSON, RoomService independiente, store en memoria. Sin DB/cache.

Adapter hará handshake, TLS, sesiones/tokens, validación y límites; RoomService
hará autorización, selección/Ready, orden, snapshots, controles programados y
resume. Inyectar reloj. Una cola por sala serializa mutaciones; backpressure
desconecta clientes lentos y resume con snapshot, sin cola ilimitada.

No usar lib multimedia ni rutas locales en backend. Self-hosting se documentará
cuando exista un binario/config real. Reinicio pierde salas en v0.1.
Ver [ARCHITECTURE](../docs/ARCHITECTURE.md), [PROTOCOL](../docs/PROTOCOL.md) y
[SECURITY](../docs/SECURITY.md) antes de construir el spike WebSocket.
