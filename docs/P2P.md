# Distribución P2P de multimedia — Phase 1

Estado de implementación: TCP/TLS directo para Linux y Android, con control de
sala WSS separado. La evidencia y clasificación de pruebas están en el
[experimento 13](../experiments/13-p2p-media-distribution/README.md).
No hay conectividad P2P universal por Internet ni reproducción progresiva.

## Arquitectura y autoridad

Core y SyncEngine mantienen sus responsabilidades y thresholds. `transfer-model`
contiene DTOs estrictos sin I/O; `transfer` contiene TLS, framing, bloques y
almacenamiento; Client posee workers y coordina el lifecycle. RoomService autoriza
ofertas/pedidos y entrega grants por WSS. Flutter presenta consentimiento,
progreso y acciones. Kotlin conserva SAF/FD/Media3 e integra archivos privados.
Los bytes binarios nunca atraviesan RoomService ni su WebSocket.

Solo el Host conectado ofrece el medio exacto de la revisión actual. La UI exige
confirmar derechos de distribución. Un participante conectado solicita después
de consentir y escoger almacenamiento; el Host autoriza cada solicitud. Una
invitación permite entrar a la sala, no descargar archivos. No se extrae DRM.
Una transferencia activa por receptor y un grant activo global por sala. Los
otros pedidos pueden esperar; no hay swarm ni scheduler masivo.

## Transporte y configuración LAN

TCP fiable con TLS 1.3, Rustls/ring y rcgen. Certificados efímeros; pin de hoja
específico, nombre SAN validado, sin verificadores permisivos, TLS 1.2, 0-RTT o
TLS resumption. La decisión y alternativas están en
[ADR-014](DECISIONS.md) y la [matriz técnica](../experiments/13-p2p-media-distribution/TRANSPORT-EVALUATION.md).

Servidor local seguro de ensayo: `cine-server --bind IP_LAN:1729 --allow-lan --tls`.
Emite un endpoint WSS con certificado público en el fragmento `#tls=`. El pin se
comparte por un canal de confianza junto a la invitación. No hay trust-on-first-use
silencioso. Reiniciar el servidor genera otro pin. El cliente retira el fragmento
antes del handshake. Un endpoint WSS sin pin falla de forma explícita.

P2P se anuncia en la IP IPv4 privada concreta del Host, puerto sugerido 1730,
editable al ofrecer. Abrir acceso local a TCP 1729 (sala) y 1730 (Host emisor) si el
firewall lo bloquea. No cambiar firewall automáticamente. AP isolation, redes de
invitados y VPN pueden impedir el acceso. No discovery, UPnP ni port forwarding.
`ws://` existente sigue disponible sin P2P ni confidencialidad de sala.

## Protocolo y compatibilidad

`protocol_version=1` permanece. HELLO negocia `p2p_transfer_v1` únicamente en WSS.
`P2P_TRANSFER_REQUEST` contiene `signal`: offer, request, accept, reject, cancel,
withdraw o status. `P2P_TRANSFER_STATE` lleva snapshot público y, solo a Host y
receptor autorizado, grant privado. Direcciones y certificados son información
de conexión pública dentro de la sala; credenciales son privadas. Clientes que
no negocian la capacidad no reciben estos eventos y seleccionan su propia copia.
No se cambia sequence de playback por eventos de transferencia.

El servidor reutiliza binding connection/member, epoch, autoridad, revisiones,
roles, schema estricto y dedupe existentes. Grant aleatorio 256 bits separado de
invite/resume, vinculado a room/epoch/receiver/transfer/manifest/revisiones. Expira
a los 10 minutos; una conexión lo consume una sola vez. Pausa, rechazo, cancelación,
salida, desconexión, cambio de medio o autoridad revocan el grant. Resume requiere
solicitud y nueva autorización del Host. La red puede entregar bytes ya en vuelo
hasta que el peer observe revocación; no se afirma revocación instantánea remota.

## Manifest, integridad y progreso

Manifest v1: UUIDs transfer/room/epoch/host/media, revisiones media/autoridad,
nombre genérico sin filesystem, tamaño, chunk size/count, SHA-256 completo,
duración y `block_hash=sha256`. Reutiliza `ContentIdentity(size_bytes, sha256)`
y el algoritmo LocalMedia; no hay otra identidad. Máximo 16 GiB, chunks 1 MiB,
16384 chunks, manifest coherente antes de reservar almacenamiento.

El receptor pide un índice. Respuesta: índice u32, offset u64, longitud u32,
SHA-256 32 bytes y datos binarios. Verifica índice esperado, offset y longitud
calculados localmente antes de leer datos; luego hash antes de escribir/marcar.
Duplicado idéntico no aumenta progreso; contenido diferente se rechaza. Último
chunk puede ser menor. TCP/TLS autentica/confidencializa; checksum no sustituye
la autenticación. Cada chunk tiene un hash ligado al contenido enviado; el SHA
completo comprometido en manifest detecta contenido distinto al medio ofrecido.

`received_bytes` cuenta bytes del intento actual; `verified_bytes` cuenta bloques
únicos del archivo parcial completo. Porcentaje usa verificados/total. Velocidad
usa nuevos bytes verificados/tiempo del intento. ETA aproximada solo durante
transferencia; no se muestra en pausa/verificación/error. Final SHA requiere leer
el archivo entero fuera del runtime/UI y utiliza LocalMedia.

## Almacenamiento, pausa y recuperación

Linux: el usuario escoge una carpeta explícita. Se canonicaliza, crea directorio
`cine-UUID` exclusivo 0700 y `media.part` exclusivo 0600. Nunca interpretar el
nombre remoto como ruta. No sobrescribir nombres existentes. Se comprueba espacio
(tamaño + 64 MiB). Errores de disco/permiso detienen la transferencia.

Android: destino privado `filesDir/cine-transfers`, espacio consultado con StatFs.
No se supone que un document provider SAF permita seek/write/reanudación. Source
SAF utiliza ContentResolver y FD duplicado después del hash; transferencia exige
archivo regular seekable del tamaño seleccionado. Providers no compatibles fallan
limpiamente; no se descarga/copia todo en RAM. Exportar el resultado a un destino
SAF arbitrario sigue NOT IMPLEMENTED. Permisos persistibles existentes siguen
su política; el owner cierra sus duplicados.

Checkpoint en memoria: hash opcional por bloque. Sobrevive socket interrumpido,
pausa y reconexión de sala mientras vive el proceso. Antes de reanudar se releen
los bloques marcados; corrupción los convierte en pendientes. Nueva autorización
no puede cambiar manifest. La recuperación tras muerte del proceso NO está
implementada; archivos huérfanos requieren limpieza local, no son películas listas.

Pause cierra socket, conserva bloques y detiene trabajo. Resume revalida autorización,
manifest y bloques, pide solo los faltantes. Cancel cierra socket, invalida grant
y elimina parcial/directorio de manera best effort; nunca elimina un completado.
Cambio de sala/medio cancela parciales incompatibles. Host desconectado pausa;
política de lease/terminación existente, sin host migration automática.

Completado: todos los chunks + fsync + SHA final exacto + autorización vigente +
commit sin reemplazo + sync directorio. Linux/Android usan `renameat2`
`RENAME_NOREPLACE`, fallan cerrado si no soportado. El nombre final generado es
`media.mp4`; el Player detecta contenido, no se infiere formato por extensión.
No exponer un parcial al Player. El archivo completado atraviesa de nuevo el flujo
existente LocalMedia → Player real → identidad/revisión → clock trusted → Ready.
100% de chunks en VERIFYING no significa Ready ni aceptación automática.

## Recursos y lifecycle

Un worker nativo propietario por emisor/receptor activo, un socket de datos,
una solicitud por chunk. Buffer de aplicación 1 MiB; TLS buffer configurado 64 KiB;
checkpoint hasta 528 KiB más indices hasta 64 KiB. Estos límites
NO son un límite global RSS: kernel, TLS, allocator y Player añaden memoria.
Sin colas ilimitadas ni futures por chunk. Canal de grants capacidad 1. El emisor
limita inicialmente a 8 MiB/s por sala para preservar margen del Player.

Handshake de datos/control máximo 4096 bytes, socket idle 3 s, handshake/autenticación total 5 s, connect 5 s,
12 accepts/min por listener, rendezvous grant máximo 2 s con un solo socket,
15 solicitudes de participantes por sala. WSS mantiene presupuestos existentes
256 conexiones/128 salas, 64 KiB, profundidad12 y rates de sala. Handshake WSS
acotado a 2 s. Estos controles reducen abuso simple; no son protección DDoS pública.

Cancel/Drop interrumpen socket y unen workers; buffers/FDs se liberan por owner.
Los worker threads no ejecutan Flutter isolate, Media3 main Looper ni SyncEngine.
Background Android pausa; no foreground service ni garantía de descarga prolongada.
Reconexión requiere sala válida, oferta vigente y aprobación nueva; no hay resume
silencioso de autorización. Hash final y hash de carga se hacen secuencialmente,
no se omite la validación existente para optimizar prematuramente.

## Límites y próxima fase

Runtime probado y medidas: consultar evidencia actual; build no equivale a prueba
física. Windows/macOS/iOS conservan sus builds de CI pero P2P runtime no se promete.
Browser no soporta este transporte nativo. IPv6, discovery, NAT traversal,
relay fallback y WAN directo siguen NOT IMPLEMENTED.

El [plan WAN](../experiments/13-p2p-media-distribution/WAN-ROADMAP.md) trata NAT,
CGNAT, firewalls, UDP bloqueado, ICE/STUN/TURN y self-hosting como fase independiente.
No hay servidor central de películas, infraestructura pública nueva ni voz.
