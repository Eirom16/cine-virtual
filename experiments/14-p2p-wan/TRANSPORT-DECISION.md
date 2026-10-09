# Transport decision — Phase 2

Consultado 2026-10-09. **PROVISIONAL**, previo al spike. Investigación de fuentes
primarias y código publicado, no benchmark comparativo. Registro exacto de
versiones/licencias/commits: [dependency-research.json](dependency-research.json).

## 1. Contexto

HEAD inicial b41f3c03854767fed1456ac3c917c90977c9a45b, master limpio; Actions
37966817288 con 11/11 success. Phase 1 prueba TCP/TLS Linux ↔ Android en LAN.
`TransferIntent::Offer::validate` solo admite IPv4 privada/loopback; Client liga
listener y anuncio a la IP local del socket WSS. No hay WAN, ICE o relay ocultos.
WSS exige `#tls=` y nombre SAN correcto; cine-server genera certificado efímero
para la IP de bind/localhost, no para un dominio público. Reinicio cambia pin.

Objetivo: directo si alcanzable, relay si autorizado; conservar chunks/Partial,
identidad, grants de RoomService, Player, Ready, SAF y Rich Chat. No tres stacks.

## 2. Alternativas y ventajas

A. TCP/TLS actual: LAN ya medida; IPv4 pública/forwarding y IPv6 global pueden
funcionar si listener/firewall alcanzables. Un relay de emparejamiento con DOS
conexiones salientes permite mantener TLS peer-to-peer dentro del túnel. Kernel
resuelve congestión; no SCTP, SDP o runtime nuevo. ICE-TCP es un estándar distinto
(RFC6544), no equivale a probar una lista de direcciones.

B. WebRTC DataChannels fiables/ordenados: ICE host/srflx/relay y checks reales,
DTLS/SCTP; mayor probabilidad de directo con UDP permitido, TURN cuando necesario.
Útil si el coste operativo de retransmisión obliga a maximizar directo. No
requiere voz/cámara. `webrtc` 0.21.0 sobre `rtc` 0.21.0 ofrece Tokio/smol y ring;
API real PeerConnectionBuilder, create_offer, add_ice_candidate y DataChannel
send/writable/try_send. Hay que fragmentar los bloques de 1 MiB en mensajes
acotados y configurar límites de send-buffer (default no los establece).

C. Quinn 0.11.12: streams fiables y TLS1.3, backpressure; UDP y migración de
conexión. `Endpoint`, `Connection::open_bi`, SendStream/RecvStream. No gathering,
ICE, STUN ni TURN integrado: necesita librería/orquestación adicional. Migración
QUIC no preserva por sí sola membresía, grants o Ready ante cambio de red.

D. Híbrido: conservar carrier TCP/TLS directo y añadir túnel relay bajo el mismo
stream. Menor riesgo de migración; dos conexiones salientes, sin cambiar protocolo
de bloques. Otra variante (TCP LAN + WebRTC WAN) duplica carrier/cripto/lifecycle,
justificable solo con evidencia de coste/éxito directo y build móvil.

## 3. Desventajas y límites

TCP no ofrece hole punching general: detrás de CGNAT/symmetric NAT el camino
mínimo será normalmente relay. Port forwarding doméstico no controla CGNAT.
IPv6 global sigue sujeto a firewall, soporte operador y políticas de privacidad.
El túnel suma TLS exterior para admisión y head-of-line de TCP; throughput/CPU
WAN no conocidos. Firewall que solo permite HTTP puede rechazar TLS dedicado
incluso en puerto443. No prometer conectividad universal.

WebRTC cambia fundamentalmente carrier. Fingerprint DTLS por WSS debe sustituir
la prueba de identidad del carrier, conservando grant/manifest. Riesgo de
colas/threading, API reciente y validación ARMv7 pendiente. IMPORTANTE: código
publicado `webrtc` 0.21.0 `transport/turn_relayer.rs::gather` fija
TransportProtocol::UDP; aceptar esquema turns en parser no demuestra TURN/TLS.
`webrtc-ice` 0.17.2 `agent_gather.rs` rechaza casos TURN TCP/TLS con TODO explícito.
No elegir estas rutas asumiendo fallback para UDP bloqueado. `str0m` 0.24.1
realiza checks pero deja NIC/gathering y sockets TURN a la aplicación. Default
aws-lc-rs/examples; alternativa rust-crypto requiere validación independiente.
`rice-proto` 0.4.3 es Sans-I/O ICE/TCP/TURN candidato; README deja RFC7675 pendiente.

Quinn puede conservar TLS y manifest, pero añadir traversal seguro es otra
implementación; no se justifica migrar solo por modernidad. Iroh1.3.0 integra
QUIC/traversal/relay y permite configuración custom sin servidores públicos;
introduce identidad/routing/lookup propios, MSRV1.91 vs workspace1.89, DNS Android
con JNI y más superficie. Requiere autorización de arquitectura antes de migrar.

## 4. Riesgos

Un relay propio implica mantener admisión, cuotas y operación. Limitar spike a
una pareja local, sin daemon público ni destinos arbitrarios; no promoverlo sin
prueba adversarial/pública. Endpoints pueden servir para escaneo desde el cliente:
validación/tipos/TTL/generation y consentimiento antes de conectar. Server nunca
hace fetch/connect a IPs candidatas. IP privada no identifica topología WAN.
La oferta v1 hoy distribuye dirección/certificado a todos los opt-in de sala:
NO reutilizar ese snapshot para candidatos WAN sensibles. Nueva capacidad y
entrega privada Host↔receptor son requisito previo a producto WAN.

## 5. Compatibilidad

No dependencia nueva, lock ni mínimos Rust cambiados en el camino recomendado.
El C de ring/NDK ya existe. Core cero dependencias se conserva. Builds ARMv7,
ARM64, Android x64, Windows/macOS/Linux/iOS continúan bloqueantes. Nuevas libs:
compatibilidad documental/plausible NO equivale a build ni runtime probado.
Libdatachannel soporta Android/iOS/POSIX/Windows según upstream, C++17/CMake,
usrsctp y crypto/ICE nativo, wrapper0.16.1 MPL2.0; revisar obligaciones por archivo.
Tamaño binario incremental de candidatos NOT MEASURED; no números inferidos.

## 6. Seguridad

Interior: TLS1.3 Host↔receptor con pin WSS y SAN cine-transfer.local, grant secreto
solo dentro de ese TLS, un uso/TTL/revocación y scope actual. Exterior: TLS1.3
peer↔relay con pin separado y SAN de relay; tickets independientes por rol,
scope/manifest, CSPRNG, comparación constante, TTL, un uso y cuotas.
Pareja/generation y allocation vinculada por WSS son requisitos de producto
posterior, no señalización implementada en el laboratorio.
Relay posee SOLO clave exterior: observa ciphertext interior, tiempos/tamaños/IPs,
puede cortar/reordenar/alterar bytes (TLS interior detecta manipulación), no leer
medio o grant interior. TLS peer→relay→peer sin TLS interior sería insuficiente.
No tickets/0-RTT/resumption. No claves, IPs completas ni manifests en evidencia.

## 7. Coste operativo

Para F bytes útiles retransmitidos, relay recibe ~F y emite ~F más overhead;
un proveedor puede cobrar salida, ambos sentidos o tráfico agregado: comprobar
contrato antes de contratar. 10 archivos de 2GiB suponen ~20GiB de salida y
~40GiB aggregate, sin overhead (aritmética, no precio medido). Cuotas de bytes
cifrados por pareja, TTL, idle, rate, global connections/egress y admisión privada.
Sin almacenamiento/caché; memoria de buffers y kernel solamente. No servicio
tercero default, puerto público ni contratación autorizados en esta sesión.

## 8. Decisión recomendada

**A+D aditiva: mantener TCP/TLS y demostrar un túnel relay seguro local**, antes
de integrar producto. No ICE/STUN ficticio. Directo WAN por endpoint explícito
alcanzable/IPv6 es posibilidad documentada, no implementada en señalización v1.
NAT traversal UDP/WebRTC queda alternativa si las medidas justifican su migración,
y requiere autorización explícita antes de reemplazar carrier. Conexión directa
no garantizada; relay debe ser opt-in/configurado y con recursos finitos.

## 9. Componentes conservados

RoomService/membership/autoridad, WSS/pin, manifest1/chunks/SHA/Partial/grants,
workers/cancel/resume vivo, Core/SyncEngine, LocalMedia/Ready, Flutter/SAF/Player.

## 10. Cambios limitados y plan de integración

Spike: pequeña interfaz Read+Write+timeouts para TCP o TLS exterior; wrappers
TCP existentes preservados. Relay local empareja sockets SALIENTES autorizados,
transporta ciphertext interior con presupuesto y plazo. Sin servicio público.
Producto posterior: nueva capability negociada, candidatos privados tipados
(no ICE), endpoint público separado de bind, grant nuevo antes de cambiar ruta,
generation/expiración/revocación, diagnósticos sin IPs, consentimiento de relay/
datos móviles. WSS público debe preservar pins/SAN o migrar a PKI tras revisión;
reverse proxy TLS que termina WSS requiere hacer explícita la frontera trusted
secure signaling en server (hoy Server.secure solo vía serve_tls).

## 11. Qué debe demostrar el spike

LAN baseline; TLS interior sobre ambos sockets outbound hacia relay; hash final,
pausa/cancel/reanudar grant nuevo y bloques faltantes; pin exterior/interior
erróneo/MITM/replay/quotas/revocación/timeout. Métricas separadas de loopback,
LAN y WAN. Dos redes reales solo tras disponer de WSS seguro y relay autorizado.
Sin esos recursos: gates WAN BLOCKED/NOT TESTED, aunque pase el laboratorio.

## Fuentes primarias

- [ICE RFC8445](https://www.rfc-editor.org/rfc/rfc8445.html), [STUN RFC8489](https://www.rfc-editor.org/rfc/rfc8489.html), [TURN RFC8656](https://www.rfc-editor.org/rfc/rfc8656.html), [ICE-TCP RFC6544](https://www.rfc-editor.org/rfc/rfc6544.html).
- [DataChannels RFC8831](https://www.rfc-editor.org/rfc/rfc8831.html), [QUIC RFC9000](https://www.rfc-editor.org/rfc/rfc9000.html), [CGN RFC6888](https://www.rfc-editor.org/rfc/rfc6888.html).
- [webrtc fuente0.21.0](https://docs.rs/crate/webrtc/0.21.0/source/), [rtc0.21.0](https://docs.rs/crate/rtc/0.21.0/source/), [ice0.17.2](https://docs.rs/crate/webrtc-ice/0.17.2/source/), [stun](https://docs.rs/crate/stun/0.17.2/source/), [turn](https://docs.rs/crate/turn/0.17.2/source/).
- [str0m0.24.1](https://docs.rs/crate/str0m/0.24.1/source/), [rice-proto0.4.3](https://docs.rs/crate/rice-proto/0.4.3/source/), [Quinn0.11.12](https://docs.rs/crate/quinn/0.11.12/source/).
- [datachannel0.16.1](https://docs.rs/crate/datachannel/0.16.1/source/), [libdatachannel](https://libdatachannel.org/), [Iroh1.3.0](https://docs.rs/crate/iroh/1.3.0/source/), [Rustls](https://docs.rs/rustls/0.23.45/rustls/).
- [coturn configuración](https://github.com/coturn/coturn/blob/master/examples/etc/turnserver.conf), [Android network state](https://developer.android.com/develop/connectivity/network-ops/reading-network-state).
