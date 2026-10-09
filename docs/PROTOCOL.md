# Protocolo de control v1

Estado: especificación normativa; codec/servidor implementan el subconjunto de
[Spike A](../experiments/03-websocket-sync/README.md). Este
documento es normativo para los spikes. [ARCHITECTURE](ARCHITECTURE.md) describe
los módulos; [SYNC](SYNC.md) explica el reloj y las correcciones. MAYÚSCULAS
identifican tipos wire, no funciones de UI.

## Transporte, versión y límites

Inicialmente WebSocket con un objeto JSON UTF-8 por mensaje de texto. Control y
multimedia viajan por canales separados: prohibidos bytes de archivos en este
protocolo. WSS fuera de localhost en producción; sin compresión por defecto.
El vertical slice 2 permite excepcionalmente ws en LAN controlada mediante modo
explícito --allow-lan; no altera v1 ni aprueba exposición pública. Máximo 64 KiB por
mensaje reensamblado, 16 miembros, profundidad JSON 12. Enteros JSON dentro de
0..9007199254740991 (rango seguro interoperable), excepto drift_ms signed en
±9007199254740991 y offsets internos signed;
rate finito, 1.0 en v1. Strings limitados por bytes UTF-8, no por caracteres.

`protocol_version: 1` obligatorio desde el primer mensaje. SESSION_HELLO ofrece
versiones soportadas, servidor elige 1 o devuelve PROTOCOL_VERSION_UNSUPPORTED y
cierra con 1002. No hay downgrade silencioso. Campos opcionales nuevos que no
alteren semántica pueden ignorarse en v1, pero nunca se habilitan por ello nuevas
capacidades. Campos requeridos, tipos o semántica incompatibles requieren v2.
Evento desconocido → INVALID_EVENT; no se aplica parcialmente. No se aceptan
claves duplicadas en JSON, números no finitos, enums desconocidos ni payloads
arbitrarios en campos sensibles. P2P/providers/moderators no están habilitados.

## Envelope

| Campo | Regla |
| --- | --- |
| protocol_version | Entero 1 |
| event_id | UUIDv4 nuevo del emisor; una solicitud reintentada reutiliza ID y payload |
| type | Tipo definido aquí |
| room_id | UUIDv4 o null antes de entrar o para reloj de conexión |
| room_epoch | UUIDv4 o null; obligatorio para mensajes en sala |
| sender_id | Cliente: null antes de entrar, después su member_id; servidor: literal `server` |
| sequence | Cliente: null; servidor: secuencia de sala en eventos mutadores/snapshot, null para respuestas privadas/clock |
| sent_at_ms | Reloj monotónico del emisor; diagnóstico, nunca orden ni execute_at |
| payload | Objeto específico del tipo; validado antes de cualquier mutación |

IDs son opacos. UUIDv4 generados con CSPRNG del sistema operativo; ver ADR-004.
El adapter vincula conexión a member_id: un sender_id declarado por el cliente
nunca concede autoridad. Broadcasts del servidor pueden incluir actor_id en
payload para mostrar quién originó un cambio; no se atribuyen a una conexión cliente.
La fuente autoritativa es el canal autenticado con el servidor, no sender_id.

Ejemplo de pedido del Host (IDs ilustrativos, enteros en ms):

```json
{
  "protocol_version": 1,
  "event_id": "08b251ba-e3cc-4db3-8253-f020b7d19b31",
  "type": "PLAY_REQUEST",
  "room_id": "d57e07ba-4e81-4b90-8136-c51d261547ee",
  "room_epoch": "e39c025f-ac56-4af6-8d98-3853f49bde1c",
  "sender_id": "960e191a-423d-4c34-bb37-6bdf3a2d3940",
  "sequence": null,
  "sent_at_ms": 74200,
  "payload": {
    "expected_sequence": 12,
    "authority_revision": 1,
    "media_revision": 1,
    "position_ms": 125000
  }
}
```

No incluye execute_at: lo asigna exclusivamente el servidor. `expected_sequence`
es un control optimista, no una secuencia del cliente. Si cambia la sala entre
pedido y aceptación, se devuelve OUT_OF_SEQUENCE y se solicita snapshot.

## Estado, orden e idempotencia

Toda mutación aceptada incrementa exactamente una secuencia de sala, empezando
en 1 al crearla. Mensajes TIME, ACK, ERROR y snapshots no la incrementan. Snapshot
tiene la secuencia del estado completo que contiene. Comparar solo dentro del
mismo room_epoch; sequence no se deriva de timestamps ni UUIDs.

Cada evento autoritativo mutador incluye `payload.state` completo y validado;
v1 evita un reducer de deltas complejo. Incluye además los campos de motivo que
figuran en las tablas. El coste es aceptable para 16 miembros y no incluye media
binaria. Un cliente inicial/resumido aplica ROOM_STATE atómicamente; cancela
timers, normaliza timelines vencidas y reconstruye el timer de pending vigente.
Snapshot de secuencia inferior a la aplicada se ignora; igual puede reconstruir
estado en recuperación. No aceptar snapshot de otro epoch sin un join/resume
autenticado explícito que reinicialice la réplica.

Eventos de secuencia ≤last se ignoran. last+1 se aplica; >last+1 obliga a
SYNC_REQUEST, suspende controles y no se aplica. Mientras recupera, solo snapshot
resuelve el gap. WebSocket ofrece orden dentro de una conexión, pero estos
controles protegen de reintentos, colas viejas y cambios de conexión.

Solicitudes mutadoras se deduplican por `(room_epoch, member_id, event_id)`;
antes de pertenecer a sala, por `(connection_id,event_id)`. Cache de diseño:
120 s, 256 IDs por emisor; al llenar no expulsar IDs vigentes, devolver RATE_LIMITED.
Guardar fingerprint del payload canónico validado y resultado. Duplicado igual
devuelve ACK/ERROR guardado y, si está en sala, ROOM_STATE actual; no rebroadcast
ni incrementa secuencia. Mismo ID con payload/tipo distinto → INVALID_EVENT.
La ventana limita garantía: nunca reintentar controles tras reconexión o timeout
de esa ventana; pedir snapshot y emitir una nueva intención con nuevo ID.
Se revalida sesión activa antes de acceder a la cache; no reejecutar la acción.
create/join reintentados después de completar entrada en la misma conexión
buscan primero su registro preentrada. Un ROOM_CREATE cuya respuesta se perdió
antes de obtener tokens no se recupera en otra conexión; sala huérfana expira.

ACK indica aceptación, no ejecución del reproductor. Un rechazo no consume
secuencia. Servidor serializa autorización, mutación, dedup y asignación de
secuencia por sala. Eventos de sistema (desconexión, cierre, pausa segura) siguen
el mismo orden. Una respuesta privada siempre identifica request_event_id.

## Tipos comunes

`State` es el RoomState de ARCHITECTURE: room_id, room_epoch, sequence,
updated_at_ms, host_id, authority_revision, permissions, members, media nullable,
playback nullable. En wire `media` contiene media_revision y descriptor; playback
contiene current y pending nullable. Cada Timeline lleva status, position_ms,
anchor_time_ms, rate y duration_ms. Pending incluye command_event_id nullable
(null para pausa de sistema), sequence, authority_revision, media_revision,
execute_at_ms y timeline_after. `timeline_after.anchor_time_ms == execute_at_ms`.

`MediaDescriptor` v1:

```json
{
  "media_id": "30332c7f-2a44-43f0-8107-b3e76494edbf",
  "source_type": "local_file",
  "title": "Vídeo compartido",
  "duration_ms": 5400000,
  "identity": {
    "algorithm": "sha256",
    "digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "size_bytes": 734003200
  },
  "mime": "video/mp4",
  "codecs": ["h264", "aac"]
}
```

Hash ilustrativo, no corresponde a un archivo incluido. Digest: exactamente
64 hex minúsculas. Duración >0 y ≤7 días; tamaño >0 y ≤1 TiB. title opcional/null,
≤256 bytes; mime opcional/null ≤128 bytes; codecs ≤16 strings de 1–64 bytes.
Título lo elige el usuario; no copiar automáticamente la ruta o nombre privado.
Metadatos de decodificación no identifican bytes: solo hash completo+tamaño.
El server valida formato, no puede comprobar los bytes privados. Campos provider
y URLs están prohibidos en v1. Ver [MEDIA](MEDIA.md).

Miembro: member_id, display_name (1–64 bytes), role (`host|participant`), connected,
ready, verified_media_revision nullable, status (`idle|loading|ready|buffering|error`),
joined_at_ms, lease_expires_at_ms nullable. No tokens ni rutas en State. Ready
requiere estado preparado; buffering/error lo invalidan. permissions es el perfil
fijo `host_only_v1`: Host tiene seis permisos, Participant ninguno de control.

`ControlContext`: expected_sequence, authority_revision, media_revision.
Todos obligatorios en controles de reproducción. `AdminContext`:
expected_sequence, authority_revision; selección usa además previous_media_revision.
Las revisiones antiguas devuelven STALE_MEDIA/STALE_AUTHORITY,
secuencia distinta OUT_OF_SEQUENCE. Estado completo ≤64 KiB
con límites previos. Invitación compartida contiene room_id, room_epoch e
invite_token; no es suficiente compartir el room_id.

`permissions` se serializa como objeto con `profile: "host_only_v1"`,
`host_permissions: ["play","pause","seek","change_media","kick_member","manage_permissions"]`
y `participant_permissions: []`. En v1 manage_permissions permite validar que
solo el Host podría administrarlos, pero la edición devuelve FEATURE_NOT_SUPPORTED.
Transferencia usa autoridad de Host y no una ACL adicional.

Ejemplo completo de evento autoritativo: la sala continúa pausada hasta 10500 ms
del reloj servidor. El snapshot y la transición llevan la misma revisión de media.

```json
{
  "protocol_version": 1,
  "event_id": "e8e619ba-3e96-454b-9278-02f54626e7e3",
  "type": "PLAY",
  "room_id": "d57e07ba-4e81-4b90-8136-c51d261547ee",
  "room_epoch": "e39c025f-ac56-4af6-8d98-3853f49bde1c",
  "sender_id": "server",
  "sequence": 13,
  "sent_at_ms": 10000,
  "payload": {
    "command_event_id": "08b251ba-e3cc-4db3-8253-f020b7d19b31",
    "actor_id": "960e191a-423d-4c34-bb37-6bdf3a2d3940",
    "state": {
      "room_id": "d57e07ba-4e81-4b90-8136-c51d261547ee",
      "room_epoch": "e39c025f-ac56-4af6-8d98-3853f49bde1c",
      "sequence": 13,
      "updated_at_ms": 10000,
      "host_id": "960e191a-423d-4c34-bb37-6bdf3a2d3940",
      "authority_revision": 1,
      "permissions": {
        "profile": "host_only_v1",
        "host_permissions": ["play", "pause", "seek", "change_media", "kick_member", "manage_permissions"],
        "participant_permissions": []
      },
      "members": [
        {
          "member_id": "960e191a-423d-4c34-bb37-6bdf3a2d3940",
          "display_name": "Host",
          "role": "host",
          "connected": true,
          "ready": true,
          "verified_media_revision": 1,
          "status": "ready",
          "joined_at_ms": 1000,
          "lease_expires_at_ms": null
        },
        {
          "member_id": "98a428fd-3f65-4835-97ea-e0161e590d16",
          "display_name": "Participant",
          "role": "participant",
          "connected": true,
          "ready": true,
          "verified_media_revision": 1,
          "status": "ready",
          "joined_at_ms": 1500,
          "lease_expires_at_ms": null
        }
      ],
      "media": {
        "media_revision": 1,
        "descriptor": {
          "media_id": "30332c7f-2a44-43f0-8107-b3e76494edbf",
          "source_type": "local_file",
          "title": "Vídeo compartido",
          "duration_ms": 5400000,
          "identity": {
            "algorithm": "sha256",
            "digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "size_bytes": 734003200
          },
          "mime": "video/mp4",
          "codecs": ["h264", "aac"]
        }
      },
      "playback": {
        "current": {
          "status": "paused",
          "position_ms": 125000,
          "anchor_time_ms": 9000,
          "rate": 1.0,
          "duration_ms": 5400000
        },
        "pending": {
          "command_event_id": "08b251ba-e3cc-4db3-8253-f020b7d19b31",
          "sequence": 13,
          "authority_revision": 1,
          "media_revision": 1,
          "execute_at_ms": 10500,
          "timeline_after": {
            "status": "playing",
            "position_ms": 125000,
            "anchor_time_ms": 10500,
            "rate": 1.0,
            "duration_ms": 5400000
          }
        }
      }
    }
  }
}
```

## Sesión y sala

C→S = cliente a servidor; S→C = respuesta privada; S→R = broadcast en sala.
En todas las filas de requests aplican INVALID_EVENT, NOT_AUTHORIZED y RATE_LIMITED
si corresponde; los errores enumerados son adicionales. En todas las filas de
eventos S→R: campos comunes `state`, actor_id nullable; cliente no puede emitirlos.
Emitir un tipo exclusivo del servidor → NOT_AUTHORIZED y ningún cambio.

| Tipo/dirección/emisor | Payload | Conducta/errores específicos | Idempotencia |
| --- | --- | --- | --- |
| SESSION_HELLO C→S cualquiera | supported_versions:[1], client_name ≤64 bytes | Primer mensaje, sin sala. PROTOCOL_VERSION_UNSUPPORTED | Repetición igual antes de operar: misma respuesta; después INVALID_EVENT |
| SESSION_ACCEPT S→C servidor | selected_version:1, connection_id, clock_epoch, limits | Establece sesión efímera sin cuenta; sin secuencia | No modifica sala |
| ROOM_CREATE C→S sesión | display_name | Crea room/epoch/media=null, Host, seq=1; ACK con room_id, room_epoch, member_id, invite_token, resume_token, lease_ms; luego ROOM_STATE. Una sala por conexión; ROOM_LIMIT_REACHED | Cache preentrada |
| ROOM_JOIN C→S sesión | invite_token, display_name | room_id/epoch en envelope obligatorio; valida invitación, añade no preparado. ACK con member_id/resume_token/lease_ms y snapshot. ROOM_NOT_FOUND, ROOM_FULL, INVITE_INVALID, ROOM_CLOSED | Cache preentrada; sin duplicar miembro |
| ROOM_RESUME C→S sesión | resume_token, last_sequence | Envelope sala/epoch. Reasigna misma identidad antes de lease, rota token y revoca socket viejo; Ready=false; ACK con token nuevo y snapshot. RESUME_EXPIRED, ROOM_NOT_FOUND, ROOM_CLOSED | Cache conexión; token viejo solo sirve al retry idéntico en esa conexión durante 120 s |
| ROOM_LEAVE C→S miembro | {} | Elimina identidad y token. Host cierra sala si no transfirió. ACK final; ROOM_NOT_FOUND | Retry cache si misma sesión cerrando; token no permite nuevo resume |
| ROOM_STATE S→C servidor | state, clock_epoch | Snapshot atómico en create/join/resume/sync, sin nueva mutación | Aplicar según secuencia/epoch |
| MEMBER_JOINED S→R servidor | member_id, state, actor_id | Nueva entrada; incluye al nuevo miembro después de ACK y snapshot. Clientes con igual secuencia ignoran duplicado | Secuencia única |
| MEMBER_STATUS S→R servidor | member_id, reason: disconnected/resumed/status_changed, state, actor_id | Actualiza presencia/preparación; Ready=false al desconectar. Host desconectado sustituye pending por Pause seguro en este mismo state | Secuencia única |
| MEMBER_LEFT S→R servidor | member_id, reason: left/expired/kicked, state, actor_id | Retira miembro no Host; para Host que sale/expira se emite ROOM_CLOSED en su lugar | Secuencia única |
| MEMBER_KICK_REQUEST C→S Host | AdminContext, member_id | Retira participante; no puede expulsarse a sí mismo. MEMBER_NOT_FOUND | Dedup de solicitud |
| ROOM_CLOSED S→R servidor | reason: host_left/host_expired/idle_timeout, state, actor_id | Estado final previo a destruir room; cancela timers y tokens, cierra pertenencia. reason actor_id nullable | Evento final secuenciado |

Un ACK de join/resume se encola antes del snapshot y del evento con esa secuencia;
los miembros ya existentes reciben el evento. El nuevo cliente instala snapshot
y puede ignorar el evento repetido. Una expulsión invalida socket/token; la salida
se notifica también al afectado antes de cerrar. RoomState nunca contiene secretos.

Desconexión detectada por cierre WS o heartbeat sin respuesta (ping cada 5 s,
timeout 15 s). lease de 30 s empieza al detectar desconexión, no al último frame.
Host con media: servidor calcula Pause seguro a now+lead, proyectando posición a ese
instante según timeline vigente; reemplaza pending antiguo. MEMBER_STATUS incluye
ese nuevo pending. Sin media solo cambia presencia. Si expira/sale sin transferir, ROOM_CLOSED incrementa una
secuencia y termina. Sala sin conexiones expira también; huérfana creada sin
entrega de credenciales se elimina como máximo en 120 s.

## Media y preparación

| Tipo/dirección/emisor | Payload | Conducta/errores específicos | Idempotencia |
| --- | --- | --- | --- |
| MEDIA_SELECT_REQUEST C→S Host | AdminContext, previous_media_revision (0 si ninguna), descriptor | Solo paused/stopped sin pending; selecciona, aumenta media_revision, borra Ready/claims, timeline paused posición=0 ancla=now. UNSUPPORTED_SOURCE, CONTROL_PENDING, INVALID_STATE, OUT_OF_SEQUENCE | Dedup; mismo archivo seleccionado otra vez es nueva revisión |
| MEDIA_SELECTED S→R servidor | media_revision, state, actor_id | Cancela cargas/timers anteriores; prepara selección nueva | Secuencia única |
| MEDIA_METADATA C→S miembro | media_revision, identity, duration_ms, mime nullable, codecs | Claim de copia local: compara hash+tamaño y duración contra Host (tolerancia ≤1000 ms); guarda verified_media_revision, sin Ready automático. MEDIA_MISMATCH, STALE_MEDIA, INVALID_MEDIA | Dedup; repetir claim igual con ID nuevo acepta sin nueva mutación, ACK secuencia actual |
| MEDIA_VERIFIED S→R servidor | member_id, media_revision, state, actor_id | Informa verificación declarada válida, no demuestra posesión criptográfica | Secuencia única |
| MEDIA_READY C→S miembro | media_revision, clock_uncertainty_ms | Solo claim verificado, Player cargado pausado/preparado, reloj confiable ≤100 ms. MEDIA_NOT_VERIFIED, STALE_MEDIA, CLOCK_UNCERTAIN | Dedup; repetir Ready actual es no-op ACK |
| MEDIA_NOT_READY C→S miembro | media_revision, reason: loading/buffering/player_error/user | Borra Ready; no modifica timeline global. STALE_MEDIA | Dedup; estado igual es no-op ACK |
| MEDIA_READINESS S→R servidor | member_id, media_revision, reason, state, actor_id | Refleja Ready/NotReady y status del miembro | Secuencia única |

Servidor no verifica precisión real del clock/Player de un cliente: son reportes
de una sesión autorizada. El Host también hace MEDIA_METADATA y MEDIA_READY.
Cambiar copia en un dispositivo exige MEDIA_NOT_READY, nuevo hash/claim y Ready.
Un descriptor inválido nunca queda seleccionado parcialmente.

## Controles y sincronización

| Tipo/dirección/emisor | Payload | Conducta/errores específicos | Idempotencia |
| --- | --- | --- | --- |
| PLAY_REQUEST C→S Host | ControlContext, position_ms | Programa playing con ancla execute_at. Todos conectados Ready; NO_MEDIA, MEDIA_NOT_READY, STALE_MEDIA, OUT_OF_SEQUENCE, CONTROL_PENDING, POSITION_OUT_OF_RANGE | Dedup; retry no programa otro Play |
| PAUSE_REQUEST C→S Host | ControlContext | Programa paused en posición proyectada a execute_at; no requiere Ready. NO_MEDIA, STALE_MEDIA, OUT_OF_SEQUENCE, CONTROL_PENDING | Dedup |
| SEEK_REQUEST C→S Host | ControlContext, position_ms | Programa seek, preserva playing/paused. NO_MEDIA, STALE_MEDIA, OUT_OF_SEQUENCE, CONTROL_PENDING, POSITION_OUT_OF_RANGE | Dedup |
| PLAY / PAUSE / SEEK S→R servidor | command_event_id, state, actor_id | state.playback.pending incluye execute_at y timeline_after; futuro con lead configurado. No cambia current todavía | Secuencia única; ejecución tardía proyecta state a now |
| SYNC_REQUEST C→S miembro | last_sequence, reason: initial/gap/resume/clock_change/manual | Devuelve ROOM_STATE actual; no modifica secuencia. ROOM_NOT_FOUND | Repetible, snapshot siempre actual |
| SYNC_STATE S→R servidor | state, actor_id:null | Snapshot periódico cada 5 s; sequence actual, no incrementa. Cancelar/rehacer timers solo si requiere reconciliar | Igual secuencia puede refrescar; inferior ignorar |
| SYNC_REPORT C→S miembro | media_revision, measured_at_ms (dominio servidor estimado), position_ms, drift_ms signed, correction: none/rate/seek, applied_rate, buffering | Telemetría limitada ≤1/s; no cambia timeline ni secuencia. STALE_MEDIA, INVALID_EVENT | Descarta duplicados; no efectos de sala |
| TIME_PING C→S sesión | sample_id UUIDv4, t1_ms | Sin scope de sala; una muestra pendiente por ID. RATE_LIMITED | Un ID de muestra no se reenvía |
| TIME_PONG S→C servidor | sample_id, clock_epoch, t1_ms, t2_ms, t3_ms | Capturar T4 local al recibir, calcular RTT/offset. No secuencia de sala | Respuesta repetida se descarta |

`SYNC_CORRECTION` no es una orden del servidor en v1: cada SyncEngine calcula
correcciones sobre su Player. SYNC_REPORT permite observarlas sin centralizar
el feedback loop. Server no acepta PLAY/PAUSE/SEEK directos de clientes.
La media duration seleccionada gobierna límites; rate nominal no editable.
Pedido repetido con nuevo ID y sin cambio efectivo puede responder ACK no-op
(Play ya playing en posición equivalente no se asume no-op: se programa).

Solo una transición pendiente. Antes de procesar cada mensaje el servidor
normaliza una pending vencida. Snapshot conserva pending futura; timeline_after
no se aplica antes de execute_at. El servidor no inventa secuencia al vencer un
timer. Pausa segura por desconexión del Host puede sustituir pending mediante
MEMBER_STATUS, única excepción de sistema a CONTROL_PENDING.

## Autoridad y permisos

| Tipo/dirección/emisor | Payload | Conducta/errores específicos | Idempotencia |
| --- | --- | --- | --- |
| HOST_TRANSFER_REQUEST C→S Host | AdminContext, target_member_id | Paused/stopped, sin pending, destino conectado Ready y media verificada si hay selección. Cambia host/roles, authority_revision+1. MEMBER_NOT_FOUND, MEDIA_NOT_READY, INVALID_STATE, CONTROL_PENDING, OUT_OF_SEQUENCE | Dedup; no transferencia repetida |
| HOST_TRANSFERRED S→R servidor | previous_host_id, host_id, authority_revision, state, actor_id | Cancela timers antiguos y revalida permisos; conserva media/timeline. No election automática | Secuencia única |
| PERMISSION_UPDATE_REQUEST C→S Host (reservado) | AdminContext, member_id, permissions | En v1 siempre FEATURE_NOT_SUPPORTED, ninguna mutación; perfil fijo | Repetir rechazo sin efectos |
| PERMISSION_UPDATED S→R servidor (reservado) | member_id, permissions, state, actor_id | No se emite en v1. Cliente rechaza y pide snapshot si recibe una capacidad no negociada | No habilitado |

Transferencia se puede realizar sin media seleccionada a miembro conectado;
Ready no aplica en ese caso. Mensajes viejos del anterior Host fallan autorización
o STALE_AUTHORITY. Un miembro no hereda Host mediante resume expirado.
Roles moderator, permisos editables y elecciones se reservan para otra versión.

## Respuestas y errores

| Tipo | Dirección/emisor | Campos y conducta | Idempotencia |
| --- | --- | --- | --- |
| ACK | S→C servidor | request_event_id, room_sequence nullable, result objeto acotado; confirma aceptación/no-op. Credenciales solo en result privado de create/join/resume | Cache de resultado durante ventana |
| ERROR | S→C servidor | request_event_id nullable, error:{code,message,details?}; nunca incrementa secuencia ni contiene stack/ruta/token | Cache para solicitudes mutadoras válidas rechazadas |

message es texto legible, nunca contrato de lógica; details objeto tipado y
redactado, ≤2 KiB. code estable, UI traduce por code. Error de parse sin event_id
válido usa request_event_id=null. Ningún payload de ERROR representa contenido.

| Code | Significado y recuperación |
| --- | --- |
| ROOM_NOT_FOUND / ROOM_CLOSED | Sala inexistente/terminada; nueva invitación o sala |
| ROOM_FULL / ROOM_LIMIT_REACHED | Capacidad alcanzada; no se muta estado |
| NOT_AUTHORIZED / INVITE_INVALID | Sesión/rol/invitación incorrectos; no confiar en sender_id |
| RESUME_EXPIRED | Lease/token expirado; join nuevo sin heredar rol |
| MEDIA_MISMATCH | Hash/tamaño distintos; elegir otra copia |
| MEDIA_NOT_READY / MEDIA_NOT_VERIFIED | Preparar/verificar; details puede incluir member_ids no listos |
| STALE_MEDIA / STALE_AUTHORITY | Revisión antigua; snapshot y nueva intención |
| NO_MEDIA / INVALID_MEDIA / UNSUPPORTED_SOURCE | Selección ausente/invalidada/no v1 |
| PROTOCOL_VERSION_UNSUPPORTED | Terminar conexión; actualizar cliente |
| INVALID_EVENT | Formato/tipo/campos inválidos; ninguna aplicación parcial |
| OUT_OF_SEQUENCE | expected_sequence antiguo/gap; details expected/received; snapshot |
| POSITION_OUT_OF_RANGE / INVALID_STATE / CONTROL_PENDING | Control imposible en estado actual; no reintentar ciegamente |
| CLOCK_UNCERTAIN | Recalibrar antes de Ready |
| MEMBER_NOT_FOUND | Destino de transferencia/expulsión inexistente |
| RATE_LIMITED / PAYLOAD_TOO_LARGE | Backoff o reducir payload; límite no consume secuencia |
| FEATURE_NOT_SUPPORTED | Capacidad reservada fuera de v1 |
| PLAYER_ERROR | Error local tipado de adapter; UI+MEDIA_NOT_READY(reason=player_error), sin enviar detalles privados |
| INTERNAL_ERROR | Fallo servidor redactado; log correlacionado, pedir snapshot |

## Seguridad y privacidad de sesión

invite_token y resume_token son secretos independientes aleatorios de ≥256 bits,
representados base64url, nunca room_id ni hash del vídeo. En memoria servidor
conserva verificadores criptográficos, con expiración; emisión/rotación usa
CSPRNG y biblioteca mantenida, nunca criptografía casera. La cache privada de
idempotencia conserva temporalmente request/ACK de credenciales durante 120 s
para responder retries idénticos; no se persiste ni se loguea, y está acotada.
No confundir esa cache de resultados con el store de verificadores de sesión.
Resume exige conexión nueva
con SESSION_HELLO; reemplazar socket anterior es atómico y revoca su autoridad.
Tokens se transmiten en frames privados sobre WSS, nunca en query de logs.

Límites iniciales diseñados: 20 mensajes/s y burst 40 por sesión; TIME_PING ≤10/s
en warmup, SYNC_REPORT ≤1/s, create/join ≤5/min por origen. Límite global de
conexiones configurable. Antes de exposición pública deben implementarse estos
límites y pruebas de abuso. El spike aplica 20/s con burst 40 por conexión y
colas acotadas; presupuestos por origen y telemetría específica siguen pendientes.
Ver [SECURITY](SECURITY.md) para boundary, observabilidad y contenido no confiable.

## Extensión social v1 negociada

IMPLEMENTED: HELLO puede incluir `capabilities:["social_v1"]`; ACCEPT devuelve
capabilities aceptadas. Ausencia equivale a ninguna capacidad social. Servidor
anterior ignora el campo opcional; cliente nuevo deshabilita social si ACCEPT no
lo confirma. Cliente anterior no recibe los eventos nuevos. protocol_version=1
se conserva, sin alterar eventos/semántica de playback.

| Tipo | Dirección | Payload / conducta |
| --- | --- | --- |
| CHAT_SEND | C→S miembro | `{text}` exclusivamente. Identidad y nombre proceden de binding/membership. |
| CHAT_MESSAGE | S→R negociados | Entrada autoritativa de chat o presencia; envelope scope room/epoch, sequence=null. |
| SOCIAL_STATE | S→C negociado | `{social_sequence,entries}`. Snapshot del historial reciente; enviado después del ROOM_STATE de create/join/resume/sync/retry. |
| REACTION_SEND | C→S miembro | `{emoji}` exclusivamente; allowlist. |
| REACTION | S→R negociados | `{reaction_id,sender_id,emoji,sent_at_ms}`; sin history/sequence. |

Entrada: message_id UUIDv4 generado por servidor; sender_id del miembro ligado a
conexión; display_name copiado del registro autoritativo; social_sequence >0;
sent_at_ms monotónico del servidor; kind `chat|joined|left|resumed`; text plano
(chat) o vacío (sistema). Nombre permanece aunque miembro salga. UUID de request
solo correlaciona ACK/ERROR; nunca concede identidad ni se usa como ID global de
mensaje. Distintos miembros pueden reutilizar el mismo event_id sin colisionar.
Room/epoch ya están en envelope; no se duplican por entrada.

Social no consume room sequence, no exige expected_sequence/media/Ready/reloj
confiable y no aplica timeline/scheduler/SyncEngine. Envelope cliente sequence
no nulo continúa rechazado; epoch/scope/sesión deben ser vigentes. Controles
con expected_sequence obsoleto conservan OUT_OF_SEQUENCE. Secuencia social ordena
chat/presencia determinísticamente; snapshot inferior se ignora y delta repetido
se descarta. Gap pide SYNC_REQUEST existente; snapshot reemplaza buffer acotado.

Texto: máximo 2048 bytes UTF-8 antes de trim; extremos Unicode whitespace se
recortan, vacío se rechaza, hasta 8 saltos de línea interiores (9 líneas).
Se permiten Unicode/emoji, tab y LF; controles restantes se rechazan. No HTML/
Markdown/embeds. Invalid format → INVALID_EVENT; exceso → PAYLOAD_TOO_LARGE.
Payload con sender_id/display_name adicional se rechaza; envelope sender_id no
concede identidad. Validación repetida en RoomService además de codec/cliente.

Cuotas independientes por miembro, token bucket con tiempo servidor inyectado:
chat burst 5, refill 1/2000 ms; reactions burst 8, refill 1/500 ms. Resume no
reinicia cuota. Duplicado idéntico responde resultado cacheado y snapshot sin
rebroadcast/consumir cuota; payload/tipo distinto con mismo ID → INVALID_EVENT.
RATE_LIMITED queda cacheado para ese intent: nuevo intento voluntario usa otro ID.
También aplica límite general existente de conexión y cache de idempotencia.

Allowlist exacta: ❤️ 😂 😮 😢 🔥 👏. Reacciones no se retienen en servidor ni
SOCIAL_STATE. Client conserva máximo 32 recibidas durante 3 s; descarta entrega
con más de 3 s de edad al serializar. Snapshot/resume limpia reacciones locales.
No reproducir reacciones antiguas. UI limita 12 simultáneas y elimina a los 2,2 s.

Historial compartido de chat/presencia: máximo 100 entradas y 48 KiB de presupuesto
conservador `384 + escaped_json_bytes(text) + escaped_json_bytes(display_name)` por entrada,
incluyendo metadata y escapes JSON sin multiplicar UTF-8 normal por seis. Se expulsa más antiguo primero. El límite de bytes puede reducir
la cantidad efectiva por debajo de 100; wire completo permanece bajo 64 KiB.
Client aplica los mismos límites. La cache de intents existente conserva payloads
hasta 120 s dentro de sus límites; no equivale a historial durable.
Solo joined/left/expired/resumed producen entradas de presencia. Disconnect
transitorio no crea entrada; resumed se coalesce a una por miembro cada 30 s.
Play/Pause/Seek/Ready no llenan chat. No texto social en logs por defecto.

## Extensión opt-in rich_social_v1 (protocol_version 1)

HELLO debe ofrecer social_v1 y rich_social_v1; ACCEPT devuelve ambas. Sin rich,
MESSAGE_SEND/MESSAGE_REACTION_SEND → FEATURE_NOT_SUPPORTED. Clientes social_v1
reciben CHAT_MESSAGE con texto fallback `[GIF]` y SOCIAL_STATE sin contenido rico,
reply, UTC ni reacciones a mensajes. Sin social_v1 no reciben nada social.

MESSAGE_SEND payload contiene content discriminado `{type:"text",text}` o
`{type:"gif",gif:descriptor}` y reply_to_message_id opcional/null. No permite
sender/nombre/ID/secuencia del mensaje: los deriva RoomService. CHAT_MESSAGE
conserva kind chat/text fallback y añade content, reply_to_message_id,
sent_at_utc_ms opcionales. Históricos sin UTC muestran autor sin hora inventada.

Descriptor: provider, provider_content_id (ASCII alfanumérico/guion/underscore
1–64 bytes), media_url (≤1024 bytes), preview_url opcional (mismo límite),
width/height enteros 1–640 y alt_text ≤256 bytes sin controles. Objetos estrictos,
sin claves adicionales; esquema HTTPS, sin usuario/puerto/fragmento. Fixture solo
`celebrate` y URL exacta `https://fixtures.cine.invalid/celebrate.gif`: identificador
sintético que Flutter resuelve al asset incluido, nunca HTTP hacia .invalid.

Validación GIPHY sintáctica restringida a media.giphy.com/media0..4.giphy.com,
path /media/{content_id}/ con extensión GIF/WebP y queries cid/ep/rid/ct acotadas.
RoomService devuelve FEATURE_NOT_SUPPORTED para GIPHY incluso con descriptor
sintácticamente válido mientras no exista aprobación de integración. No arbitrary
remote renderer, redirects, server fetch ni proxy multimedia.

Reply solo referencia UUID de Text/GIF retenido en la misma sala. ID desconocido,
erróneo, de otra sala o ya evicted al enviar → INVALID_EVENT. Una referencia ya
aceptada sigue válida al evictar original; UI muestra mensaje no disponible.

MESSAGE_REACTION_SEND payload `{message_id,emoji}` hace toggle por miembro/emoji;
allowlist ❤️ 😂 😮 😢 🔥 👏. Máximo 16 identidades/emoji, 6 emojis/entry; cuota
independiente burst 6 y refill 1/s por membership, conservada durante resume.
ID desconocido/sistema → INVALID_EVENT. UUID del actor viene del binding, no payload.

Toggle aceptado aumenta social_sequence y entrega SOCIAL_STATE completo con
live_update:true. Entries conservan su secuencia original; watermark puede ser
mayor que la última entry. message_reactions es mapa emoji → lista de member UUIDs,
sin nombres ni contenido duplicado. Cliente conserva floating queue en live_update;
snapshot de reconnect la vacía. Resume recupera replies/reacciones con el mismo
historial 100/48 KiB de presupuesto conservador JSON; metadata/reacciones también
consumen presupuesto. Dedup existente evita segundo toggle/rebroadcast al retry.
UTC es epoch ms del servidor capturado al crear mensaje, solo presentación.

## Extensión opt-in p2p_transfer_v1 (protocol_version 1)

Solo WSS con certificado anclado acepta capability `p2p_transfer_v1`. WS anterior
no la anuncia; ausencia preserva selección manual y eventos anteriores.
`P2P_TRANSFER_REQUEST` contiene exclusivamente `signal` discriminado por `action`:
offer/request/accept/reject/cancel/withdraw/status. Schema estricta, UUIDs/revisiones
y manifest se validan antes de RoomService. `P2P_TRANSFER_STATE` es server-only.
Snapshot público para peers opt-in; grant secreto solo Host y receptor autorizados.
Sequence de playback permanece sin cambios. Byte stream usa TCP/TLS separado.

Manifest/grant/status, límites, expiración/revocación y reconexión se especifican
en [P2P](P2P.md). Invitación nunca autoriza un archivo automáticamente. Credencial
de un uso ligada a room/epoch/member/manifest/media/autoridad; resume requiere
grant nuevo. Cambiar medio o autoridad invalida transferencias incompatibles.
