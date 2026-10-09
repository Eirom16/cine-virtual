# INFORME — P2P MEDIA DISTRIBUTION PHASE 1

Clasificación: IMPLEMENTED = código; TESTED = ejecución observada; MEASURED =
medición exportada; VISUALLY VERIFIED = captura inspeccionada; PROVISIONAL =
decisión/alcance condicionado; INFERRED = deducción; NOT IMPLEMENTED / NOT TESTED
no significan PASS. BLOCKED solo identifica una barrera concreta, no una excusa.
Las fuentes de evidencia son [RESULTS](RESULTS.md), sus JSON y [VISUAL-QA](VISUAL-QA.md).

## 1. Estado inicial

TESTED: master limpio y sincronizado con origin/master en
1fdd14e904f02491eba9e159d478c33179eb8e4c. Git/remotes, gh auth y runs inspeccionados;
CI 37863493740: success, 11/11. No se supuso el estado de sesiones previas.

## 2. Auditoría arquitectónica

Inspeccionados README, ARCHITECTURE, PROTOCOL, SYNC, MEDIA, UI, SECURITY, TESTING,
DECISIONS y CI, además de experimentos LocalMedia/Linux–Android/UI. Se preservan
RoomService, Client, Replica, WebSocket, LocalMedia, MobilePlayer/Media3,
libmpv, SAF, ApplicationController, Ready y recuperación existentes.
Core/SyncEngine no se reemplazan ni se cambian sus thresholds.

## 3. Tecnologías evaluadas

PROVISIONAL: Quinn/QUIC, WebRTC DataChannels y TCP/TLS 1.3; HTTP range, swarm y
storage central excluidos por alcance. Fuentes primarias vigentes en la
[matriz](TRANSPORT-EVALUATION.md); no benchmarks inventados de alternativas.

## 4. Comparación QUIC/WebRTC/TCP

QUIC: streams/migración y UDP, traversal externo. WebRTC: ICE/DTLS/SCTP y mayor
superficie inicial. TCP/TLS: fiable/binario/Rust compartido con framing propio,
head-of-line y sin NAT traversal. Matriz cubre plataformas, CPU/memoria, latencia,
throughput, cifrado, resume, mantenimiento, multiplexación y voz futura.

## 5. Transporte elegido

IMPLEMENTED / TESTED: TCP directo con TLS 1.3, Rustls/ring/rcgen. Carrier separado
para bytes; WSS existente extendido para signaling/grants. Elección Phase1 LAN,
PROVISIONAL para futuro Internet.

## 6. Justificación

Un único archivo/receptor y Android armv7 favorecen un carrier acotado. No
requiere SCTP/SDP/ICE, reemplazar Player ni dependencia arquitectónica masiva.
No se eligió por modernidad ni por la futura voz. Evidencia real bidireccional;
QUIC/WebRTC deberán reconsiderarse en el spike WAN con condiciones equivalentes.

## 7. Seguridad del transporte

IMPLEMENTED / TESTED: TLS 1.3, certificado efímero anclado por señalización
confiable y SAN validado. Sin aceptar cualquier certificado, TLS 1.2, tickets,
0-RTT ni resumption TLS. Handshake absoluto5 s e idle3 s, control≤4096 bytes.

## 8. Autenticación de peers

Host demuestra posesión de clave del certificado anunciado; receptor presenta
credencial CSPRNG de 256 bits dentro del TLS. Credential ligada a sala/epoch/member/
media/autoridad/manifest/transfer. Comparación del secreto en tiempo constante.
No mTLS: autenticación de receptor mediante grant bearer privado de un uso.
TESTED pin/credential erróneos, scopes incorrectos, replay y revocación.

## 9. Autorización de transferencia

IMPLEMENTED: Host conectado ofrece solo su revisión multimedia actual; confirma
que puede distribuirla. Receptor consiente, solicita; Host acepta/rechaza.
Invite/resume no autorizan descargar. Grant con TTL de 10 min y nuevo grant tras pausa/
reconnect. Disconnect/salida/cambio media/autoridad invalidan. Una autorización
activa por sala; no descargas automáticas.

## 10. Señalización de sala

IMPLEMENTED / TESTED: P2P_TRANSFER_REQUEST + signal offer/request/accept/reject/
cancel/withdraw/status; P2P_TRANSFER_STATE snapshot y grants filtrados. RoomService
solo coordina/membresía/autorización. WSS transporta metadata, jamás los chunks.

## 11. Capability negotiation

IMPLEMENTED / TESTED: p2p_transfer_v1 explícita negociada solo con sala WSS.
Sin capacidad servidor/peer/plataforma/oferta/Host conectado no recibir/resume.
UI explica WSS/compatibilidad y conserva selección manual.

## 12. Compatibilidad de protocolo

protocol_version=1 conservado. Eventos nuevos solo para opt-in. TESTED legacy WS
sin P2P y WSS real con grants; no cambio incompatible ni control binario en JSON.

## 13. Modelo de manifest

IMPLEMENTED / TESTED: v1, transfer/room/epoch/host/media UUIDs, authority/media
revision, nombre genérico sanitizado, size ≤16 GiB, chunk de 1 MiB / count ≤16384, SHA-256
completo, duración, block_hash=sha256. Validación estricta/deny_unknown_fields;
ningún path/URI/token dentro del manifest.

## 14. Media identity

Reutiliza ContentIdentity(size_bytes,sha256) y hash_reader de LocalMedia.
Manifest debe coincidir exactamente con selección/revisión de sala; no identidad
paralela. Fuente FD y size/mtime, SHA final protege reemplazo/cambios inconsistentes.

## 15. Chunking

IMPLEMENTED / TESTED: índice solicitado u32; header de 48 bytes con índice, offset,
len y SHA-256, seguido de datos. Offset/len calculados localmente, último chunk
menor. No archivo completo en RAM ni millones de tareas.

## 16. Integridad por bloques

TESTED corruptos, duplicados, desordenados, truncados, bounds, offsets maliciosos
con TLS real y tests deterministas. Verificar antes de escribir/marcar. Duplicado
idéntico no suma progreso; diferente falla. Hash no sustituye autenticación.

## 17. SHA-256 final

IMPLEMENTED / TESTED en ambos dispositivos: releer archivo completo, identidad
exacta, fsync y commit seguro. Archivo incorrecto nunca cargado/Ready. 100% de
bloques todavía muestra VERIFYING y Ready deshabilitado.

## 18. Backpressure

IMPLEMENTED / TESTED: una solicitud/chunk pendiente, lectura/escritura síncrona
en worker independiente, emisor limitado a 8 MiB/s, canales de capacidad 1, un grant global por sala.
El receptor limita el ritmo; sin cola creciente ni scheduler swarm.

## 19. Buffer limits

Buffer de 1 MiB + TLS de 64 KiB configurado, checkpoint hasta 528 KiB e índices de 64 KiB.
Estos no son un límite RSS total. MEASURED pico Linux transport7260 KiB / 1 thread /
6 FDs para 128 MiB. Players, kernel, TLS y runtime añaden memoria propia.

## 20. Resume/checkpoints

IMPLEMENTED / TESTED: bitmap equivalente de hashes opcionales por chunk en
memoria. Releer bloques marcados antes de confiar; corruptos vuelven a pendientes.
Solo faltantes enviados con manifest idéntico y grant nuevo. Process-death resume
NOT IMPLEMENTED, no afirmado por mantener archivos parciales en disco.

## 21. Pausa

IMPLEMENTED / TESTED Linux y Android: shutdown del socket, cese de trabajo,
conservación de bloques. Resume explícito requiere nueva aprobación Host.
Android: 256 MiB, pausó a 10 MiB; inverso: 128 MiB, a 1 MiB. Pausar durante SHA también cancela
ese trabajo sin marcar película completa.

## 22. Cancelación

IMPLEMENTED / TESTED: local incluso si WSS cae, socket/grant/workers cerrados,
parcial eliminado best effort; nunca elimina completado. Error de cleanup visible.
Linux real prueba cancelwaiting/offline; Android UI prueba cancelwaiting y
eliminación de directorio. No promesa de borrado físico seguro del almacenamiento.

## 23. Errores

Errores tipados de manifest/bounds/corruption/identity/auth/expiry/pause/cancel/
TLS/I/O/storage/space/sourcechange. UI presenta causas concretas y retry de
carga verificada. No logs de credenciales, URI, paths privados o contenido.

## 24. Linux filesystem

Carpeta explícitamente escogida, canonicalizada; directorio UUID 0700, parcial 0600 exclusive,
statvfs con margen de 64 MiB, nombre remoto nunca path. Commit renameat2 no-replace y
fsync del directorio. Symlink/overwrite/conflictos rechazados. Same-UID atacante fuera de
threat model; chooser portal Wayland físico NOT TESTED por harness.

## 25. Android SAF

TESTED Host SAF real ContentResolver/FD retenido y duplicado, random read/seek
regular y size exacta; owner cierra el FD. Destino privado, filesDir + StatFs, no asumir
que content:// admite random write. Providers no seek rechazados; diversos
providers físicos NOT TESTED. Export SAF destino arbitrario NOT IMPLEMENTED.

## 26. Ownership/lifecycle

IMPLEMENTED: Client posee workers/socket/control/channel/manifest/checkpoint;
RAII/drop shutdown y join. Worker no Fluttermainisolate/Media3mainLooper.
Cancel/sala/media invalidan; no workers huérfanos por diseño. FDs / threads observados
no equivalen a prueba exhaustiva de fuga durante días.

## 27. Cambio de medio

IMPLEMENTED / TESTED con procesos reales: nueva revision invalida oferta/grant/
parcial y Ready incompatible. No mezcla bloques. Listener pendiente sobrevive
ROOM_STATE inicial solo si room/media siguen iguales; withdraw lo cierra.

## 28. Disconnect/reconnect

TESTED Wi-Fi Android off/on durante chunks; ROOM_RESUME, nuevo grant, checkpoint
conservado y SHA final correcto. Host desconectado revoca/grant y pausa; resume
Host Linux real probado. Sin inventar Host ni implementar host migration.

## 29. UI Host

IMPLEMENTED / VISUALLY VERIFIED Linux y Android: medio disponible, tamaño,
verificación, distribución consent, LANaddress/port avanzado, petición por peer,
approval/reject/withdraw, bytes/estado por participante. Lobby conserva theme/chat.

## 30. UI Participant

IMPLEMENTED / VISUALLY VERIFIED Android: copia propia o recibir, tamaño/espacio/
datos, aceptación explícita, destino privado, progress/pause/resume/cancel,
verificación/carga/Ready. Linux carpeta escogida implementada/widget tested;
recepción UI Linux física NOT TESTED, recepción CLI/libmpv TESTED.

## 31. Progreso/velocidad/ETA

Derivado del transporte: received/verified/total, porcentaje de bloques únicos verificados,
rate del intento y ETA estimada solo mientras transfiriendo. No fakeprogress ni
contador por duplicados. VISUALLY VERIFIED contraste del track, pausado sin ETA y
VERIFYING al 100% sin Ready.

## 32. Integración con Ready

IMPLEMENTED / TESTED: completar SHA → LocalMedia validado → Player loaded →
identidad y revisión coincidentes → clock trusted → Ready existente. No cambiar su semántica
ni pulsarlo automáticamente al acabar transferencia.

## 33. Integración con Player

TESTED Media3 y libmpv reales, no reproductor alternativo. La misma identidad puede
seleccionarse o recibirse. No progressive streaming. Copia válida ya cargada se
preserva si recibe el mismo medio; retry de load tras error sin retransferir.

## 34. Tests de seguridad

TESTED room/epoch/member/revisions/secret, expiry/revocation/replay, pin falso,
manifest/control inválidos, chunk/header malicioso, SHA incorrecto y TLS lento con plazo absoluto.
No auditoría criptográfica externa ni DDoS Internet medido.

## 35. Tests Rust

137 PASS, 8 tests SDK opt-in ignorados existentes. Transfer: 10 tests, grants de sala, capability/
validación estricta del wire, WS/WSS reales e integración Client. fmt/clippy -D warnings / build / test
locked PASS local. CI final se verifica antes de declarar entrega completa.

## 36. Tests Flutter

68 PASS, analyze sin issues. Consent/space/capabilities, solicitud al Host responsive,
connecting/progress/pause/resume/verify/error/cancel,100% sin Ready, copia playing
preservada y cancelación offline / Host desconectado. Rich Chat / platform views regresiones.
Mocks solo en widget tests; física independiente en JSON.

## 37. Tests de integración

TESTED Linux CLI / libmpv ↔ Android Flutter / Media3 en LAN, ambos sentidos. Loopback
WSS/control/lifecycle reales adicionales. QA solo automatiza bootstrap/observación;
acciones de consentimiento/SAF/Ready son UI real. No afirmar que todas las
combinaciones de ambos shells Flutter se ejercitaron juntas.

## 38. Linux → Android

TESTED PASS, 256 MiB en results-product-linux-android.json; 128 MiB inicial separado.
Consent/grant del Host, bloques reales, pausa antes de 100%, recuperación Wi-Fi, SHA / pipeline /
Ready/Play/Pause/Seek/fullscreen/chat/reacciones / background / foreground.

## 39. Android → Linux

TESTED PASS, 128 MiB: Host SAF / UI → participante CLI / libmpv, grant, pausa a 1 MiB,
resume con nuevo grant, SHA/Ready, ambos reproduciendo / chat, Pause desde UI Host. Build release Rust +
Flutter debug en SM-J701M. No afirmar soporte otros Android / providers.

## 40. Transferencia interrumpida

TESTED socket pause / nueva conexión en TLS, disconnect receptor / Host en WSS real,
Wi-Fi físico Android. Se conservan únicamente bloques verificados y se revalidan.
Pérdida de proceso completa NOT IMPLEMENTED.

## 41. Archivo corrupto

TESTED bloques/header corruptos no cuentan, checkpoint alterado se invalida,
SHA final distinto nunca commit / Ready. Corrupción física deliberada en Android
NOT TESTED; no se sustituye por una captura de100%.

## 42. Rendimiento

MEASURED escalera8 KiB / 32 MiB / 128 MiB, producto 256 MiB y SAF 128 MiB. Corpus sintético
propio con padding, no películas ajenas. Builds/red/dispositivo en JSON.
Los tiempos con pausas/hash/lifecycle no se comparan como throughput puro.

## 43. CPU/memoria

Transporte Linux 128 MiB CPU: delta 10.36 s, pico RSS 7260 KiB. Producto Linux RSS ~76 MiB,
Android PSS ~220 MiB antes/~228 MiB pausado (valores exactos en RESULTS). Android CPU ms /
threads/FDs observados cada 500 ms, sin overhead PSS en main Looper. No batería ni
CPU Android del carrier aislada; no claim RAM global acotada por 1 MiB.

## 44. Throughput

MEASURED Linux→Android3.05–3.19 MiB/s; inverso3.09 MiB/s transporte 32 MiB.
Carga playback carrier de 32 MiB: 3.63 MiB/s en otro ensayo, no equivalencia directa.
Emisor limitado a 8 MiB/s, Wi-Fi / armv7 / debug influyen. RTT del carrier aislado NOT MEASURED.

## 45. Impacto multimedia

TESTED: ambos Players avanzan mientras un carrier independiente mueve otro archivo propio de 32 MiB; el JSON inverso contiene CPU/RSS, posiciones y estados. El medio de sala permanece igual. Smoke de controles/chat/fullscreen, sin repetir 600 s ni modificar SyncEngine. Frames, jank y batería bajo carga NOT MEASURED.

## 46. Background Android

TESTED: Player → Lobby → Home → foreground revalida archivo, clock, snapshot y Ready. La transferencia se pausa por lifecycle. No hay foreground service ni garantía de descargas prolongadas en background.

## 47. Screenshots

Capturas reales Linux Host y Android Host/receptor: disponibilidad, consentimiento, solicitud, progreso, pausa, interrupción, VERIFYING, completado, carga, Ready, Player y fullscreen. Solo nombres genéricos/corpus propio, sin invitaciones, tokens ni URI privadas.

## 48. Visual QA

VISUALLY VERIFIED: capturas reales y responsive en widgets de 360/1280. Se corrigió el track que confundía un progreso pequeño con 100%. ETA ausente en pausa, VERIFYING al 100% con Ready deshabilitado. Sin overflow observado; límites en VISUAL-QA.

## 49. Problemas encontrados

Android rechazó hardlink; se detectaron races entre grants/ROOM_STATE y listener, handshake lento sin plazo absoluto, cleanup incompleto al cancelar offline y timeout de hash de 20 s. El harness encontró bloqueo por patrón, timeout de overlay/idle, EOF sin PTY y Seek antes de Player usable. Fallos preservados o descritos, sin reclasificación retroactiva. CI detectó además herencia de sockets no bloqueantes en Windows/macOS, reproducida y corregida con test TLS real; ver [diagnóstico](CI-DIAGNOSIS.md).

## 50. Correcciones implementadas

renameat2 no-replace; listener pendiente ligado a manifest; espera de grant acotada a 2 s; DeadlineSocket de 5 s; secreto en tiempo constante; cancelación centralizada y orden de locks consistente. Resume de sala sin inventar Ready, hash sin timeout arbitrario, retry de carga, preservación de copia cargada, contraste y mensajes de UI concretos.

## 51. Limitaciones

IPv4 LAN privada, pin WSS manual, un grant por sala, máximo 16 GiB, límite emisor 8 MiB/s y resume mientras vive el proceso. Android usa destino privado. Sin discovery, foreground service, catálogo/cleanup de completados, exportación SAF, swarm, streaming, voz ni infraestructura WAN.

## 52. P2P LAN

IMPLEMENTED / TESTED: directo autenticado y cifrado Linux ↔ Android, SHA/Ready/Players reales. MVP LAN PROVISIONAL bajo las condiciones ensayadas: sin aislamiento de AP y puertos alcanzables. No se midió escalabilidad del Host ni soporte universal de plataformas/proveedores.

## 53. P2P WAN

NOT IMPLEMENTED / NOT TESTED. Una IP privada no demuestra P2P por Internet. Los anuncios de dirección pública se rechazan en esta fase.

## 54. NAT traversal

NOT IMPLEMENTED. NAT, CGNAT, firewalls y UDP bloqueado investigados con fuentes primarias. TCP actual no incluye ICE; Quinn tampoco lo incorpora automáticamente.

## 55. STUN/TURN/relay

NOT IMPLEMENTED / NOT TESTED; no deployment. Phase 2 comparará ICE/WebRTC, traversal externo/QUIC y fallback TCP/relay, con cuotas, consentimiento, cifrado, self-hosting y costes. STUN descubre mappings: no retransmite ni garantiza ruta.

## 56. Seguridad y privacidad

Pins WSS compartidos por canal confiable, credenciales efímeras privadas y revocación ligada a sala. Nombres genéricos, secreto sin Debug, métricas redacted y configs temporales fuera de Git. Distribución consentida; ningún DRM bypass. Revocación remota sujeta a bytes en vuelo y tiempo de observación.

## 57. Windows/macOS/iOS

La matriz de build se conserva; runtime de producto P2P NOT TESTED. iOS Player NOT IMPLEMENTED. Los tests TLS/WSS de CI no validan UI/Player físico; no se reemplazaron Players ni plataformas.

## 58. Regresiones

fmt, clippy workspace/all-targets/locked con -D warnings, cargo test/build, docs/CI structure y Flutter analyze/test PASS local; 10 tests Python PASS. Regresiones de chat, reacciones, clock, Ready, FFI y Player incluidas. Ocho tests SDK opt-in ignorados, explícitos. Sin continue-on-error.

## 59. GitHub Actions

Matriz de once jobs conservada. El run inicial verde pertenece al HEAD antiguo. Se realiza push autorizado y se verifica el run del último HEAD antes de cerrar. El enlace y HEAD exactos figuran en la respuesta final; este documento no inventa un hash autorreferencial ni hereda el PASS anterior.

## 60. ADR/documentación

[docs/P2P](../../docs/P2P.md), ADR-014 en DECISIONS, matriz, SECURITY, WAN roadmap, resultados/JSON/capturas. README, ARCHITECTURE, PROTOCOL, MEDIA, UI, SECURITY, TESTING y CI actualizados. No se sobrescribieron experimentos previos.

## 61. Commits

f7c8467: transfer bounded; 14332ac: grants de sala WSS; fe28478: secreto en tiempo constante; ad0a6e2: UI/SAF/pipeline; f250266: deadline/preservar playback; 668a75c: cancelación offline y resume de sala. Documentación/evidencia en commit separado; historial revisable sin force push ni rebase destructivo.

## 62. Estado Git final

Master. Fetch confirmó que origin no tenía commits ajenos (0 izquierda / 6 derecha al cerrar código). La respuesta final registra HEAD, sincronización, árbol limpio y CI correspondiente; esos estados no se deducen de una observación anterior.

## 63. ¿Está P2P LAN listo para MVP?

PROVISIONAL sí para el vertical slice LAN probado Linux/Android: consentimiento, autenticación/TLS, chunks/SHA, resume/UI/Ready/Player reales. El gate CI del último HEAD es obligatorio antes de entregar. Mantener visibles los límites de almacenamiento/plataformas y no anunciar P2P global, background prolongado ni recovery tras muerte del proceso.

## 64. ¿Qué falta para P2P por Internet?

Candidate orchestration, ICE/STUN, CGNAT/firewalls/IPv6 y fallback relay cifrado, autorizado y con cuotas. Seguridad pública por origen/rate, operación/self-hosting y pruebas entre redes/operadores. Evaluación independiente del carrier; la futura voz no decide automáticamente WebRTC para archivos.

## 65. Próxima fase recomendada

P2P MEDIA DISTRIBUTION — PHASE 2: INTERNET CONNECTIVITY. Spike comparativo y diseño revisado antes de infraestructura. No desplegar TURN ahora ni comenzar voz u otra gran fase.

## 66. Resumen para otro arquitecto

Core, SyncEngine, LocalMedia y Player se conservan. RoomService autoriza y WSS señaliza; Client posee workers; Rust Transfer mueve chunks TLS y valida SHA; Flutter consiente, muestra y controla; Kotlin integra SAF/FD/lifecycle. Manifest ligado a identidad/revisión/epoch, checkpoint en memoria y grant nuevo permiten resume. LAN físicamente probado; WAN/relay fuera. JSON, capturas, regresiones y CI distinguen evidencia real de inferencia y pendientes.
