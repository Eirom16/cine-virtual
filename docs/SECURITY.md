# Seguridad y observabilidad

Estado: requisitos de frontera documentados; esta pasada no construye un servicio
expuesto. [PROTOCOL](PROTOCOL.md) fija límites y autoridad; validarlos es condición
para abrir el primer servidor a una red externa.

## Fronteras mínimas

- Generar IDs UUIDv4 con CSPRNG; nunca incrementales públicos. IDs no son secretos
  ni permisos. Tokens de invitación y resume independientes de al menos 256 bits.
- Vincular socket a sesión/member_id en el servidor. Validar rol y revisión al
  procesar cada control; cliente no puede falsificar un broadcast autoritativo.
- WSS fuera de localhost, validación de certificado y política explícita de
  origins para clientes browser futuros. No tokens en URLs, logs ni snapshots.
- Antes de mutar: parse, tamaño, schema, versión, rango, epoch, identidad,
  autorización, revisiones e idempotencia; no aplicación parcial.
- Máximo 64 KiB por mensaje reensamblado; profundidad 12, strings/listas acotadas,
  16 miembros. Límite de lectura antes de deserializar para evitar asignaciones
  controladas por atacante. Dedupe/cache y colas también tienen límites.
- Rates iniciales: 20 mensajes/s, burst 40 por sesión; warmup clock ≤10/s,
  telemetría ≤1/s; create/join ≤5/min por origen. Límites por IP no bastan solos
  ante NAT/proxies: se combinan con sesión y presupuesto global de conexiones.
- Separar contenido y control. No aceptar binarios ni URLs de providers en v1.
  Ruta/URI queda local; no ejecutar metadatos como código ni interpolarlos en shell.
- Media no confiable: probe/decoder en adapter con límites de tiempo/memoria,
  worker y librerías actualizadas. Fallos producen PLAYER_ERROR, no caída del Core.
- Duración ≤7 días, tamaño ≤1 TiB, tags/codecs acotados; título opcional elegido,
  nunca ruta completa. Renderer escapa nombres/títulos recibidos.
- Futuras URLs: allowlist de esquemas, impedir file/loopback/link-local/redes
  privadas desde fetch servidor y revalidar DNS/redirects. Credentials de providers
  permanecen en resolver seguro; no en Domain o snapshots.

Ready/hashes son declaraciones del cliente, no prueba de posesión ni DRM.
No prometer resistencia frente a un cliente autorizado que mienta. Un Host
malicioso dentro de su sala tiene los permisos explícitos del rol; no puede
actuar en otra sala. Las invitaciones son capacidades y deben compartirse como
secretos. Persistencia de tokens/secretos en móvil requiere storage de plataforma
en la implementación, no preferencias en texto plano.

## Logs estructurados

Contrato futuro JSON: `timestamp_utc`, `level`, `event`, `component`,
`correlation_id` y campos permitidos. IDs de sala/sesión deben seudonimizarse en
exportaciones. Event IDs pueden correlacionar pedido, aceptación y ejecución;
un ID interno de trace no sustituye la autorización.

| Evento | Campos permitidos |
| --- | --- |
| room_created / room_joined / room_left / room_closed | room_id, conteo, motivo; nunca invite_token |
| client_disconnected / client_resumed | room_id, member_id efímero, duración/gracia |
| media_selected / media_verified / media_mismatch | room_id, media_revision, resultado; sin digest/título/ruta |
| playback_scheduled / playback_started / playback_paused / playback_seeked | room_id, sequence, media_revision, target/execute_at, retraso de ejecución |
| clock_sample / clock_untrusted | RTT, offset, incertidumbre, edad; debug con muestreo |
| sync_correction | drift_ms, tipo, rate, cooldown, capacidades; métricas agregadas |
| host_transferred / permission_denied | authority_revision, permiso, resultado |
| protocol_error / player_error | code, correlation_id, componente; mensaje redactado |

No loguear frames completos, rutas, contenido, tokens, nombres personales,
credenciales, hashes ni IP salvo propósito operativo explícito con retención
limitada. No capturar stderr del decoder sin redacción. Telemetría local por
defecto; no analytics externos en v0.1. Documentar retention al implementar
servidor, valor inicial operativo 7 días configurable, no persistencia de producto.

Métricas de spike: RTT/jitter, p50/p95/p99 de drift, correcciones/minuto, seek
latency, deadlines perdidos, tiempo de resume, uso CPU/memoria. Evitar labels
de cardinalidad ilimitada (UUID/usuario) en métricas.
No hay backend de logs ni dependencia de tracing implementados; esta es su política.
