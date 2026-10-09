# Modelo de seguridad — Phase 1

Implementación revisable: [P2P](../../docs/P2P.md). Estado de pruebas en RESULTS.

| Amenaza | Control | Límite |
| --- | --- | --- |
| Máquina conoce puerto | TLS + grant CSPRNG independiente | Listener recibe handshakes acotados; no DDoS público |
| Invite usado para archivo | Solicitud explícita + aprobación Host | Miembro autorizado puede guardar archivo que recibe |
| Otra sala/epoch/member/media | Credential bind + manifest fingerprint + room checks | Revocación remota depende de señalización/retraso |
| Replay | Un consumo por grant, sin TLS tickets/0-RTT | Resume rota grant |
| Peer falsifica Host | Hoja TLS anclada entregada por WSS | Distribuir pin WSS por canal de confianza |
| Bloque corrupto/offset malicioso | Offset/longitud local + SHA por chunk | Host autorizado puede mentir; SHA final exige identidad anunciada |
| Archivo reemplazado | Descriptor FD, length/mtime check, SHA final | No DRM ni prueba legal de derechos |
| Path traversal/symlink/conflicto | Nombre genérico, UUID exclusivo, permissions, commit no-replace | Destino autorizado; mismo UID malicioso fuera del threat model |
| Agotamiento RAM/disco | 16 GiB, 1 MiB, grants/colas/conexiones acotadas, espacio previo | Espacio puede cambiar; errores de escritura detienen |
| Lifecycle/archivo parcial | Worker ownership, pause/cancel, partial nunca Player | Proceso-muerte cleanup/recovery pendiente |
| Logs | UI/QA redacted; secret sin Debug; stdout create/config privado | Operador conserva secretos fuera de Git/CI |

Las pruebas deterministas verifican scope, expiración, revocación, replay, chunks,
checkpoint, SHA final y no sobrescritura. La integración usa conexiones reales
TLS y WSS; clientes antiguos siguen en WS sin capability P2P. No exportar configs,
invitaciones, digests, URIs, rutas locales o material multimedia a artifacts.
Las capturas deben inspeccionarse antes de commitear, excluyendo invitaciones.
