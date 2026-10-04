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

## ADR-005 — Hash completo SHA-256 (Adoptada en diseño)

**Context:** v0.1 exige comprobar exactamente el mismo archivo sin distribuirlo.

**Decision:** tamaño+digest SHA-256 completo, calculado streaming/cancelable antes
de Ready. Fingerprint no habilita reproducción; chunking reservado P2P.

**Alternatives:** nombre/tamaño, hash parcial, hash perceptual, manifest Merkle.

**Consequences:** espera proporcional al archivo e I/O; identidad fuerte pero no
prueba de posesión. Core compara digest; hashing aún no implementado.

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

**Next validation:** dos vídeos Linux + canal control de Spike A como slice
separado, con probe/hash/Ready reales. Validar Android/iOS en dispositivos y un
build distribuible antes de promover el stack. Flutter/bridge/licencia pendientes.

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
