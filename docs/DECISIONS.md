# Registro de decisiones arquitectónicas

Fecha inicial: 2026-10-04. `Adoptada` define el diseño actual, no declara una
integración implementada. `Provisional` necesita evidencia; `Pendiente` requiere
decisión explícita. Las modificaciones posteriores deben preservar motivación
y registrar el reemplazo. Documentación canónica: [ARCHITECTURE](ARCHITECTURE.md),
[PROTOCOL](PROTOCOL.md), [SYNC](SYNC.md), [MEDIA](MEDIA.md).

## ADR-001 — Núcleo desacoplado (Adoptada)

**Context:** cinco plataformas y posibles cambios de UI/Player/red. Un callback
de socket que controla directamente el vídeo impide probar sincronización aislada.

**Decision:** UI → Application → Domain; adapters implementan puertos. SyncEngine
devuelve efectos y recibe observaciones; RoomService separado de WebSocket.
Una biblioteca Rust pequeña implementa solo primitivas necesarias.

**Alternatives:** lógica en widgets; aplicación Rust monolítica con SDK multimedia;
decenas de crates vacíos y traits sin uso.

**Consequences:** Core testeable sin UI ni runtime. La composición/lifecycle queda
en Application; una abstracción no elimina diferencias reales de Player.

## ADR-002 — WebSocket de control (Validada en Spike A localhost)

**Context:** MVP requiere eventos bidireccionales de bajo volumen con autoridad
central, sin distribución de bytes de vídeo.

**Decision:** WebSocket/JSON v1 en canal de control, WSS fuera de localhost,
RoomService independiente. Backend Rust/Axum/Tokio implementado en Spike A;
evidencia localhost en
[experimento 03](../experiments/03-websocket-sync/README.md), sin aprobación Internet.

**Alternatives:** polling, UDP propietario, WebRTC data channel desde el primer día.

**Consequences:** operación self-host sencilla y orden por conexión; reconexión
requiere snapshot, secuencia, backpressure y leases. No usar WS para archivos.

## ADR-003 — Timeline sobre reloj monotónico (Adoptada)

**Context:** UTC puede saltar y actuar al recibir Play conserva diferencias de red.

**Decision:** servidor asigna execute_at futuro en reloj monotónico de proceso,
clock_epoch explícito, NTP conceptual para offset, timeline actual+pending. Drift
local con rate/seek, histéresis y cooldown configurables.

**Alternatives:** UTC del sistema como autoridad, llegada del frame como trigger,
el Host elige deadlines, sincronizar cada frame.

**Consequences:** reloj comparable sin ajustar sistema; reinicio invalida epoch.
Asimetría de red/seek impreciso limita precisión. Medir defaults antes de fijarlos.

## ADR-004 — UUIDv4 públicos y secretos separados (Adoptada)

**Context:** IDs públicos deben ser no incrementales y no revelar orden/fecha.
UUIDv7 y ULID ordenables no aportan valor aún sin índices persistentes.

**Decision:** UUIDv4 con aleatoriedad criptográfica para room/member/media/event/
epochs. Token ≥256 bits separado para invitación y resume. Orden solo por sequence.
[RFC 9562](https://www.rfc-editor.org/rfc/rfc9562.html) define UUIDv4 aleatorio y
UUIDv7 con timestamp; esa diferencia motiva la elección, no una superioridad general.

**Alternatives:** UUIDv7 (útil para índices futuros), ULID, IDs incrementales públicos.

**Consequences:** no filtrar hora mediante ID; no ordenar IDs ni usarlos como
contraseñas. Generación en rooms/protocol/adapters con uuid/getrandom, no con reloj
ni PRNG casero. Core conserva independencia de esa generación.

## ADR-005 — Hash completo SHA-256 (Implementada en vertical slice 1)

**Context:** v0.1 exige comprobar exactamente el mismo archivo sin distribuirlo.

**Decision:** tamaño+digest SHA-256 completo, calculado streaming/cancelable antes
de Ready. Fingerprint no habilita reproducción; chunking reservado P2P.

**Alternatives:** nombre/tamaño, hash parcial, hash perceptual, manifest Merkle.

**Consequences:** espera proporcional al archivo e I/O; identidad fuerte pero no
prueba de posesión. Core compara digest; cine-local-media implementa streaming
1 MiB con sha2, progreso/cancelación y verificación de estabilidad del handle.

## ADR-006 — Multimedia por capacidades (Provisional)

**Context:** éxito en Linux no prueba integración móvil ni packaging/licencias.

**Decision:** no elegir motor todavía; Player portable con adapter por plataforma
si hace falta. Spike compara libmpv/libVLC/GStreamer/players nativos; FFmpeg puede
inspeccionar pero no se considera una UI de reproductor completa.

**Alternatives:** libmpv universal por supuesto; solo native sin evaluar formatos;
acoplar sincronización al SDK que primero funcione.

**Consequences:** se difiere binding/rendering; evidencia Android/iOS obligatoria
antes de promover un motor. Rate es capacidad opcional; hard seek fallback.

### Evidencia adicional — Spike B, 2026-10-04

**Evidence:** comparación primaria de cuatro familias en
[RESEARCH](../experiments/01-player-crossplatform/RESEARCH.md). Un adapter libmpv
in-process API 2.5/mpv 0.41.0 decodificó corpus sintético Linux. Headless y ventana
SDK GPU/Xwayland probados; VAAPI reportada en visible. Seek paused exacto según
position en MP4, hasta +12 ms en VFR; rate medido, soft/hard correction reales,
600 s sin reloj acelerado. Métricas y límites en
[RESULTS](../experiments/01-player-crossplatform/RESULTS.md).

**Decision:** se mantiene PROVISIONAL. La decisión inicial de no elegir motor
se conserva arriba como historial; ahora libmpv es candidato suficiente para
continuar al vertical slice **Linux**, no motor universal/definitivo. FFI pequeño
con libloading evita SDK en link-time del workspace. Un solo owner !Send/!Sync,
completion por eventos fuera del puerto y ninguna dependencia SDK en Core.

**Remaining risks:** build local con FFmpeg GPL/version3, superficie embebida,
packaging, URI/lifecycle y audio Android/iOS sin prueba. Position no prueba frame
presentado; thresholds/rate necesitan tuning. RSS retenido requiere diagnóstico
adicional; bindings propios requieren auditoría unsafe/ABI. Calidad perceptual de
audio, hardware variable, corpus HD/4K y entornos sin desktop no están aprobados.

**Next validation (historial Spike B):** integrar dos vídeos Linux + canal control
de Spike A con probe/hash/Ready reales; ese trabajo se registra a continuación.

### Evidencia adicional — Vertical slice 1

**Evidence:** integración de LocalMedia (ffprobe auxiliar, SHA-256 final), proxy de
owner, una réplica/scheduler genéricos y WebSocket v1. Dos ejecutables Linux
headless y prueba prolongada con métricas en
[RESULTS](../experiments/06-real-vertical-slice/RESULTS.md). Server/rooms/protocol
no incorporan multimedia ni rutas. No se cambia el puerto o política del Core.

**Decision:** libmpv sigue **PROVISIONAL FOR LINUX**. El thread owner respeta
!Send/!Sync y usa proxy acotado, eventos de completion y muestras con timestamp.
Se avanza solo con evidencia Linux; el slice no termina v0.1 ni valida móviles.

**Remaining risks:** software decoding/headless en este recorrido no repite la
validación visible/audio de Spike B; no prueba frame presentado. RSS retenido,
C bloqueante, seek con hardware/corpus complejo, filesystem mutable, red WAN,
URI/lifecycle y build/licencias móviles siguen abiertos.

**Next validation:** gate Android/iOS real (recursos locales, owner/lifecycle,
rendering, rate/seek y packaging/licencias) antes de promover motor o fijar bridge.
Flutter/bridge/licencia siguen pendientes.

### Evidencia adicional — Spike C móvil/UI

**Evidence:** comparación Android (mpv/Media3/VLC/GStreamer) e iOS
(mpv/AVPlayer/VLCKit/GStreamer), URI/sandbox, surfaces, rate/seek, threading y
licencias en [RESEARCH](../experiments/02-rust-ui-bridge/RESEARCH.md).
Build del bridge Rust para Android x86_64, arm64 y armv7; APK/runtime emulado
API 35 x86_64 y físico SM-J701M Android 9/API 28 armv7. Runtime y límites registrados
en [RESULTS](../experiments/02-rust-ui-bridge/RESULTS.md).

**Decision:** estrategia híbrida candidata: desktop libmpv **PROVISIONAL FOR LINUX**,
Android **Media3 provisional**, iOS **AVPlayer candidate**. Un SDK universal no
es requisito. Core/Player/SyncEngine/protocolo siguen sin dependencias móviles.
Las capacidades opcionales se declaran en Application; seek completion no prueba
frame presentado y hardware no se infiere por disponibilidad de API.

**Remaining risks:** precisión/rate/formatos distintos, recursos URI y permisos,
recreación y lifecycle completos, codecs y build redistribuible. Build/runtime
Android no valida Apple ni otros dispositivos/versiones Android. No se probó mpv móvil.

**Next validation:** ampliar físico (formatos/URI/rate/lifecycle) y Mac/Xcode/iPhone para AVPlayer;
red móvil v1 + suspend/clock/snapshot antes de promover arquitectura definitiva.

### Evidencia adicional — Vertical slice 2

**Evidence:** regresiones Linux/libmpv y Android físico/Media3 aislado pasan; el
runtime Rust compartido se compila para armv7 y ejecuta controles/clock reales
por Wi-Fi durante 600,08 s. p95 Linux 36 ms, Android 514 ms y diferencia 462 ms;
75 hard seeks Android, gate de precisión fallido. Evidencia completa en
[RESULTS](../experiments/07-linux-android-room/RESULTS.md).

**Decision:** libmpv y Media3 conservan sus estados provisionales; Core y parámetros
SyncEngine no cambian. SDK dispatch, completion READY y frame presentado siguen
siendo observaciones distintas. iOS permanece fuera del gate Linux/Android.

## ADR-007 — Rust inicial, UI/puente candidatos (Provisional)

**Context:** necesitamos pruebas independientes hoy y una base portable.

**Decision:** Rust 2024/std para el núcleo inicial. Flutter es candidato,
no compromiso definitivo. Spike compara FFI/C ABI, bridge generado y canales
nativos, ownership, threading, cancelación, builds y coste de mantenimiento.
[Flutter](https://docs.flutter.dev/reference/supported-platforms) documenta las
plataformas objetivo; eso no verifica las dependencias de multimedia del proyecto.

**Alternatives:** core Dart, Kotlin Multiplatform, UI nativa con core Rust/C++.

**Consequences:** código funcional mínimo sin dependencias. Posible traducción de
API al elegir bridge; no exponer tipos Rust inestables como ABI pública.

### Evidencia adicional — Spike C

**Evidence:** comparación de Flutter/nativo, FRB/C ABI/canales/API C generada y
Players Android/iOS en [RESEARCH](../experiments/02-rust-ui-bridge/RESEARCH.md).
C ABI experimental con DTO v1, handles, buffers caller-owned, validación, hash
por FD y generation/lifecycle. Flutter Linux y Android físico ejecutaron comandos/snapshots a 2/10 Hz.
Evidencia Android y sus límites en [RESULTS](../experiments/02-rust-ui-bridge/RESULTS.md).

**Decision:** Flutter **PROVISIONAL**. Bridge elegido para el ensayo: C ABI manual
+ dart:ffi; canales nativos para SAF/Media3/surface. Rust posee Application de
experimento y SyncEngine; Kotlin posee SDK y permisos; Dart presenta/transporta
intents/efectos, no decide autoridad ni drift. No es API pública de producto y no
se expone ningún struct Core como ABI. FRB permanece alternativa si la API crece.

**Remaining risks:** ABI/unsafe manual, consumidor confiable en proceso, polling
sin stream push, SDK/URI/main thread, lifecycle completo y más dispositivos físicos.
La calibración/snapshot del Spike C aislado usan fixtures; ese experimento
no incluye red móvil.
Windows/macOS/iOS Flutter sin build/runtime; no soporte universal demostrado.

**Next validation:** completar el gate móvil con dispositivos, integración de
Application de red v1 y recovery tras suspend, más AVPlayer en Mac/Xcode. Evitar
UI final/packaging público mientras licencia y estrategia multimedia no se decidan.

### Evidencia adicional — Application móvil de red

**Evidence:** Client<P> reutiliza el canal v1, clock, réplica, scheduler y SyncEngine
sin otro cliente Dart/algoritmo Kotlin. Tests con WebSocket real y SDK fixture
cubren autoridad, Ready, generaciones, suspend/foreground y resume. JNI de driver
nativo armv7 compilado; runtime físico cruzado, SAF/hash, Ready y
background/WS resume ejecutados en
[experimento 07](../experiments/07-linux-android-room/README.md).

**Decision:** C ABI manual + dart:ffi se conserva. Kotlin/main Looper posee
SDK/SAF; Rust tiene un owner de red con cola acotada y cancelación/join. UI consulta
estado a 2 Hz; driver SDK a 50 Hz no publica UI a esa frecuencia. No se expone ABI
Rust ni se fuerza Send/Sync al Player/SDK. Flutter continúa PROVISIONAL.

**Remaining risks:** gate de precisión Android fallido, hard seeks repetidos,
recreación exhaustiva de Activity, múltiples dispositivos y WAN/TLS. Recorrido
funcional no equivale a precisión aprobada ni a stack definitivo.

## ADR-008 — Servidor único y memoria (Implementada en Spike A)

**Context:** cuentas y persistencia no están dentro del MVP.

**Decision:** un servicio autoritativo con store en memoria, secuencia por sala,
snapshots completos, perfil fijo Host/Participant y resume efímero. No elecciones
automáticas; Host puede transferir pausado, expiración del Host cierra la sala.

**Alternatives:** Redis/PostgreSQL iniciales, microservicios, autoridad distribuida.

**Consequences:** reinicio pierde salas; host grace está acotada. Recuperación
básica es v0.1; persistencia/federación requieren decisiones posteriores.

## ADR-009 — Licencia sin asignar aún (Pendiente)

**Context:** objetivo open source/self-host, sin titularidad/política de copyleft
decididas ni build multimedia elegido.

**Decision:** evaluar MIT/Apache-2.0/GPL-3.0/AGPL-3.0 en [LICENSING](LICENSING.md),
no crear LICENSE definitivo ni declarar grants inexistentes. Preferencia a evaluar:
AGPL para servidor si se desea compartir cambios servidos por red; Apache-2.0
si se prioriza adopción permisiva. No aplicar modelo mixto sin revisión explícita.

**Alternatives:** imponer MIT por hábito; imponer AGPL a todo sin validar distribución.

**Consequences:** no publicación/release bajo licencia open source hasta elegir
y aplicar textos/headers/manifest. Esta es la única decisión inmediata de política
que requiere al titular; tecnología y tuning se resuelven con experimentos.

## ADR-010 — CI de compilación multiplataforma (Hosted build validado)

**Context:** runtime Linux/Android ya tiene evidencia; falta descubrir blockers
Windows/macOS/iOS sin convertir build en soporte/runtime ni preparar una release.

**Decision:** un gate base y cinco workflows reutilizables/manuales con herramientas
y Actions fijadas, artifact/checksum/metadata y permisos read-only. iOS enlaza
bridge estático y compila device/simulator sin firma; Player Unsupported explícito.

**Alternatives:** YAML único con matrices grandes, cinco workflows automáticos
duplicados, publicación/signing inmediata. Se prefieren etapas diagnosticables.

**Consequences:** primeros runners pueden fallar; no continue-on-error, no secrets,
pushes de diagnóstico autorizados, sin stores. Core/protocolo/SyncEngine y stacks
provisionales no cambian. Build/runtime y evidencia local/remota se separan en
[CI](CI.md).

**Evidence:** run 37389305915/d22127a: base y nueve variantes PASS. Los artifacts
se inspeccionan con checksums/metadata y arquitectura; iOS main app unsigned y
Player NOT IMPLEMENTED. Historial causal en [CI-HOSTED](CI-HOSTED.json): tests SDK
no deben asumir duración exacta entre builds FFmpeg; Windows necesita file ID;
macOS debe pedir arm64, no arm64e; Simulator debe conservar C ABI en el proceso.
El fixture de Join/control espera la secuencia autoritativa, sin tocar el runtime.

**Operational follow-up:** GitHub solo registró CI y devuelve 404 al despacho
individual reusable, aunque sus archivos están en master. Input platform de CI
permite diagnóstico independiente con base gate. Un linker Simulator puede añadir
firma ad-hoc automática: solo se retira esa firma, se rechazan certificados y
se vuelve a comprobar unsigned. No se crea ninguna firma.

**Remaining risks:** compile PASS no valida Player desktop Windows/macOS ni iOS
runtime. Metadata muestra cambios generados sin falsificar clean. Stack/licencia,
packaging multimedia y precisión Android continúan pendientes.
