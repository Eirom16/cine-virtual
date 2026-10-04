# 03 — WebSocket y autoridad de sala

Estado: plan, sin servidor. Candidato Rust/Axum/Tokio en un proceso. RoomService
con reloj/store inyectados separado del adapter WebSocket; clientes CLI suficientes.

Demostrar create/join/leave, Host/Participant, identidad+Ready, Play/Pause/Seek
programados, snapshots con pending y leases. WS no transporta archivos. Validar
mensajes antes de mutar, permisos, payloads acotados, dedup, versiones y backpressure.

Pruebas: cliente intenta falsificar PLAY, solicitud vieja, duplicado mismo ID,
cambio de payload, gap, resume durante Play futuro, desconexión del Host y
transferencia pausado. Eliminar dependencia de una conexión específica en
RoomService. Resultado: codec/fixtures v1 y métricas de control, sin DB ni login.
Ver [PROTOCOL](../../docs/PROTOCOL.md) y [TESTING](../../docs/TESTING.md).
