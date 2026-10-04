# 03 — WebSocket y autoridad de sala

Estado actual: **Spike A de control implementado y validado en localhost,
pre-v0.1**. Rust/Axum/Tokio en un proceso; RoomService con tiempo numérico
inyectado/store en memoria separado del adapter. Dos clientes CLI con FakePlayer.

Demostrar create/join/leave, Host/Participant, identidad+Ready, Play/Pause/Seek
programados, snapshots con pending y leases. WS no transporta archivos. Validar
mensajes antes de mutar, permisos, payloads acotados, dedup, versiones y backpressure.

Pruebas: cliente intenta falsificar PLAY, solicitud vieja, duplicado mismo ID,
cambio de payload, gap, resume durante Play futuro, desconexión del Host y
transferencia pausado. Eliminar dependencia de una conexión específica en
RoomService. Resultado: codec/fixtures v1 y métricas de control, sin DB ni login.
Ver [PROTOCOL](../../docs/PROTOCOL.md) y [TESTING](../../docs/TESTING.md).

## Historial: intento bloqueado de Spike A — 2026-10-04

Estado en ese intento anterior: **bloqueado por el entorno, sin implementación**. Se leyeron
las fundaciones y se comprobó Git antes de modificar código. `.git` está vacío
y montado en solo lectura; `git init` falla. Se creó un repositorio de metadatos
en `.git-local`, asociado al mismo working tree, y el commit de las fundaciones
existentes `7a75db4` (`chore: establish project foundations`). Las operaciones
locales requieren `git --git-dir=.git-local ...`; no existe remoto configurado.

Dos requisitos operativos fallaron:

- Cargo no pudo resolver `index.crates.io`; Axum/Tokio/Serde y demás dependencias
  del spike no estaban disponibles en caché. `cargo fetch` terminó con error.
- `socket.socket(AF_INET, SOCK_STREAM)` devolvió `PermissionError: Operation
  not permitted`, antes de intentar bind. No puede ejecutarse un servidor ni
  una prueba WebSocket TCP in-process en este entorno.

Se retiró el scaffolding provisional sin implementación. No se añadieron
dependencias al workspace ni se modificó el Core/protocolo. No hay servidor,
cliente CLI, RoomService o codec del spike, ni demostración de dos clientes.
No existen métricas de RTT, deadlines o recuperación medidas sobre red.

Las verificaciones de las fundaciones siguen aprobadas: 26 tests, formato,
Clippy con warnings como errores, build y validación de documentos. Estas
comprobaciones no demuestran el canal de control. No hay commit `feat` del spike.

Para continuar, usar un entorno que permita TCP localhost y acceso a crates.io
o una caché completa de dependencias. Comprobar primero:

```sh
python3 scripts/check_spike_environment.py
git --git-dir=.git-local status
```

Con caché offline preparada, usar `--offline` en el diagnóstico para omitir DNS;
Cargo con `--offline` deberá verificar después que la caché esté completa. El
diagnóstico de ese intento solo comprobaba TCP/DNS. Ahora también verifica HTTPS;
Cargo sigue siendo responsable de descargar/verificar paquetes.

Después implementar el recorrido solicitado sin cambiar sus contratos. Deben
incluirse MEDIA_METADATA/MEDIA_VERIFIED y MEMBER_STATUS aunque no formen parte
de la lista prioritaria: Ready requiere verificación previa y desconexión/resume
deben comunicar presencia correctamente. No se confirmó ni reemplazó ningún ADR;
Flutter, multimedia, bridge, plataformas móviles y licencia siguen provisionales.

## Objetivo y resultado de la continuación

Full Access eliminó el bloqueo. El diagnóstico TCP/DNS/HTTPS pasó y Cargo pudo
obtener dependencias. `.git` ahora apunta mediante un gitfile a `.git-local`:
funcionan comandos `git` normales, conservando el commit original de fundaciones.
No hay remoto ni push. No se reinicializó ni borró documentación o código bueno.

Objetivo demostrado: un servidor real y dos clientes CLI reales crean/comparten
sala; Host controla, Participant no; descriptor sintético idéntico y Ready;
Play/Pause/Seek con execute_at futuro; desconexión, resume y convergencia mediante
snapshot. No hay vídeo, cálculo de hash de archivos ni precisión multimedia medida.

## Implementación utilizada

- `cine-core` conserva sus primitivas std y 26 tests originales sin modificaciones.
- `cine-rooms`: RoomService/RoomStore, tiempo numérico inyectado, autorización,
  perfiles Host/Participant, secuencias, revisiones, dedup y leases. No sockets,
  Tokio, Axum ni JSON en la lógica. Comandos producen efectos con destinatarios
  opacos, no envían conexiones.
- `cine-protocol`: serde/serde_json, DTOs separados, validación explícita, claves
  duplicadas/profundidad/rangos/UUIDs, fingerprint del payload canónico validado.
- `cine-server`: Axum 0.8.9/Tokio 1.53.2, mutex corto de store+encolado, heartbeat,
  colas 32, 64 KiB, 16 miembros, 128 salas, 256 conexiones, rate 20/s burst 40.
- `cine-client`: tokio-tungstenite 0.29, CLI de líneas, réplica pura, scheduler Tokio,
  reloj T1–T4 de ocho muestras, FakePlayer con posición/rate y corrección de drift.
- UUIDv4 vía uuid 1.27.0; tokens getrandom 0.3/base64url 256 bits con verificadores
  SHA-256. SHA-256 también identifica requests canónicas, nunca archivos en este spike.
- Cargo.lock fija versiones. Linux, Rust 1.99.0. Workspace declara mínimo 1.89,
  todavía no ensayado con esa toolchain; Core conserva mínimo 1.85.

Subconjunto v1 implementado: SESSION_HELLO/ACCEPT; ROOM_CREATE/JOIN/RESUME/LEAVE,
ROOM_STATE/CLOSED; MEMBER_JOINED/STATUS/LEFT; MEDIA_SELECT_REQUEST/SELECTED,
METADATA/VERIFIED, READY/NOT_READY/READINESS; PLAY_REQUEST/PLAY,
PAUSE_REQUEST/PAUSE, SEEK_REQUEST/SEEK; TIME_PING/PONG; SYNC_REQUEST/STATE;
HOST_TRANSFER_REQUEST/TRANSFERRED; ACK/ERROR. Kicks, edición de permisos y
SYNC_REPORT no están implementados en este spike. Tipos desconocidos se rechazan.

## Comandos reproducibles

```sh
python3 scripts/check_spike_environment.py
cargo build --workspace
python3 scripts/demo_control.py
```

La demo crea tres procesos con los binarios de `target/debug`, usa puerto efímero,
verifica sus respuestas/estados/logs y limpia todos los procesos. No imprime
invitaciones ni credenciales en el informe. Recompilar después de modificar Rust.

Para operar manualmente, tres terminales:

```sh
cargo run -p cine-server -- --bind 127.0.0.1:8765
cargo run -p cine-client -- --server ws://127.0.0.1:8765 --name Host
cargo run -p cine-client -- --server ws://127.0.0.1:8765 --name Participant
```

Host: `create`. Copiar room_id, room_epoch e invite_token privados al comando
`join <room_id> <room_epoch> <invite_token>` del Participant. Host: `media-demo`.
Ambos: `ready`. Host: `play 100000`, esperar ejecución, `pause`, esperar,
`seek 120000`. Consultar `state` entre órdenes. Participant: `disconnect`;
Host puede iniciar otra reproducción. Participant: `resume`, después `sync`.
Terminar ambos con `quit`; servidor con Ctrl-C. `leave` elimina pertenencia;
si sale el Host, cierra sala. La invitación privada solo aparece en stdout explícito.

## Verificaciones y fallos corregidos

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
python3 scripts/check_docs.py
git diff --check
git status
```

62 tests: Core 26, RoomService 21, codec 6, réplica/FakePlayer 7 y WebSocket 2.
Pruebas puras sin sleeps: autoridad, 16 miembros incluyendo leases, errores sin
secuencia, dedup/ventana/capacidad, revisiones/epoch obsoletos, Ready/claim/clock,
controles programados, pausa segura Host, cambio de Host, tokens rotados y revocación.
La integración TCP prueba el flujo completo y sender_id falsificado. JSON de
PROTOCOL se valida con el codec real, sin mantener fixtures duplicadas.

Fallos detectados durante implementación y corregidos: gap dejaba el cliente
marcado desconectado y/o permitía reconstruir un timer antiguo; ahora pausa y
bloquea ejecución hasta snapshot. SYNC_REQUEST esperaba un ACK inexistente;
ahora se correlaciona con ROOM_STATE o ERROR. Retry de Leave perdía su cache;
usa scope de conexión. Fingerprint inicialmente perdía campos ignorados del
payload; ahora digest canónico cubre todo el payload validado. Cierre de tarea
cliente agotado por timeout ahora aborta el actor, sin dejarlo vivo.

## Límites y conclusiones

Este resultado valida el canal de control, no completa v0.1 ni prueba móvil.
No hay ensayo largo, p95 de vídeo, carga/abuso ni medición de CPU/memoria.
RTT en ms puede redondear a cero; no significa latencia física nula. Offsets
incluyen los diferentes orígenes de arranque de cada proceso.

Deuda explícita: reconexión/backoff automáticos, suavizado y recuperación ante
saltos de offset, fallos de red inducidos sobre el adapter, graceful shutdown
con clientes todavía abiertos, WSS/origins/presupuestos por origen y validación
Windows/macOS/Android/iOS. El mutex global basta para el spike; medir contención
antes de dividir por sala. La cache privada de credenciales dura 120 s y contiene
resultados necesarios para retries; no se persiste ni loguea. Revisar protección
de memoria y política de cache al preparar exposición pública.

Decisiones afectadas: ADR-001 conservada y ejercitada; ADR-002 validada solo para
control localhost; ADR-003/004 ejercitadas; ADR-008 implementada en memoria.
Al cerrar Spike A, ADR-005 seguía pendiente de hashing de archivos; ADR-006/007 multimedia/UI/bridge
siguen provisionales; ADR-009 licencia sigue pendiente. No se cambió la semántica
v1; se aclaró la excepción de cache de resultados respecto al store de verificadores.

Siguiente trabajo recomendado dentro de control: harness de jitter/latencia/cierre
inducidos, resume durante pending y recuperación de reloj. No iniciar multimedia
como parte de esta tarea. Sin licencia no publicar releases ni solicitar aportes.

## Métricas de tres procesos — Linux, 2026-10-04

Evidencia: [resultado JSON](results-localhost.json), sin secretos. Una ejecución
local corta; no es un benchmark ni una garantía de precisión.

| Acción | Sequence | execute_at servidor (ms) | Retraso A (ms) | Retraso B (ms) | Diferencia de ejecución (ms) |
| --- | --- | --- | --- | --- | --- |
| play 100000 | 8 | 2173 | 2.0 | 1.0 | 1.0 |
| pause | 9 | 2677 | 1.0 | 1.0 | 0.0 |
| seek 120000 | 10 | 3180 | 3.0 | 1.0 | 2.0 |

RTT filtrado A/B: 0.0/0.0 ms; offset A/B:
12.0/720.0 ms, ocho muestras por cliente tras conectar/resume.
Diferencia de posición tras resume: 2 ms. Secuencia final compartida:
15; ambos pausados en 121300 ms. Participant rechazado sin incremento
de secuencia; no tokens/hash/payloads en logs estructurados. Resultado: **PASS**.

## Integración posterior: vertical slice 1

Este documento conserva la evidencia histórica del spike aislado. El recorrido
posterior de red + dos Player reales + LocalMedia/probe/hash/Ready se registra en
[06-real-vertical-slice](../06-real-vertical-slice/README.md), sin declarar v0.1
terminado ni promover libmpv fuera de su condición provisional Linux. ADR-005
ahora registra hashing real; móviles, UI/bridge y licencia siguen pendientes.
