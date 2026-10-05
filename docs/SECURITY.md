# Seguridad y observabilidad

Estado: bases documentadas y controles mínimos implementados en Spike A
localhost. Loopback permanece default; bind externo requiere --allow-lan
experimental con warning. No es un servicio público.
[PROTOCOL](PROTOCOL.md) fija límites y autoridad; validarlos es condición
para abrir el primer servidor a una red externa.

## Fronteras mínimas

- Generar IDs UUIDv4 con CSPRNG; nunca incrementales públicos. IDs no son secretos
  ni permisos. Tokens de invitación y resume independientes de al menos 256 bits.
- Vincular socket a sesión/member_id en el servidor. Validar rol y revisión al
  procesar cada control; cliente no puede falsificar un broadcast autoritativo.
- WSS fuera de localhost en producción (excepción de ensayo LAN controlado con
  --allow-lan y ws, sin garantía de confidencialidad), validación de certificado y política explícita de
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

Contrato objetivo JSON: `timestamp_utc`, `level`, `event`, `component`,
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
El spike emite JSON con tracing en stderr: timestamp/level/target y campos event,
room_id/connection_id, sequence o métricas cuando corresponda. No almacena logs
en archivos ni añade un backend. La invitación aparece únicamente como salida
privada explícita de `create` en stdout, para poder copiarla al otro cliente;
no se registra como log estructurado ni aparece en la demo.

Implementado: tamaño/profundidad/rangos, roles ligados a sesión, tokens y rotación,
colas 32, 16 miembros, 128 salas, 256 conexiones y dedup 256 por emisor/120 s
con techo global de 65536 resultados. Rate por conexión 20/s burst 40. Pendiente
antes de Internet: WSS, origins, límites por origen, pruebas de abuso, controles
específicos de TIME_PING/telemetría y política de retención de logs.

## Frontera LocalMedia del slice Linux

El adapter no envía rutas/URI/bytes y omite títulos personales. Solo el descriptor
portable y digest necesario para comparar viajan por el canal de control; nunca
el digest en logs. Hash/probe/load viven fuera de Core/rooms/server. ffprobe tiene
salida acotada y timeout con reap; se ejecuta sin shell. El owner libmpv es un
thread, **no aislamiento de seguridad**: una falla o bloqueo del SDK afecta al
proceso cliente. No se aprueba distribución pública con estos límites locales.

Logs añadidos: hash_progress (bytes), player_dispatch/player_event (sequence,
deadline, posición/latencia), sync_sample (timestamp/edad/drift/confianza),
player_resources (RSS/threads/FDs). La demo exporta solo campos seleccionados,
sin credenciales/digests/rutas; no exporta envelopes ni stdout de create/join.
