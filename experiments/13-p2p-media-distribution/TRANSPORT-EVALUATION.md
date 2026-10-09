# Evaluación de transporte — Phase 1

Fecha de consulta: 2026-10-09. La decisión del spike es PROVISIONAL.
Las características de los estándares son documentación; rendimiento, CPU y
soporte runtime requieren mediciones propias. No se han benchmarkeado Quinn ni
WebRTC en este proyecto.

| Criterio | QUIC / Quinn | WebRTC DataChannels | TCP / TLS 1.3 / Rustls |
| --- | --- | --- | --- |
| Fiabilidad y binarios | Streams fiables; datagramas opcionales | SCTP fiable/ordenado configurable | Stream fiable; framing de aplicación |
| Cifrado/autenticación | TLS 1.3; certificados y política de confianza | DTLS; fingerprint autenticado por señalización | TLS 1.3; certificado anclado, credencial efímera dentro del canal |
| Linux/Android/Rust | Rust compartido; validar UDP/crypto/NDK | Rust o SDK nativo; ICE/DTLS/SCTP y timers adicionales | Rust compartido; ring/NDK requiere compilador C |
| iOS/Windows/macOS | Candidato; build/runtime por validar | Candidato; diferencias de SDK/lifecycle | Candidato; API portable; runtime por validar |
| Multiplexación | Streams independientes; evita bloqueo entre streams por pérdida | SCTP multistream; límites de mensajes/buffers | Una conexión; bloqueo de stream por pérdida |
| NAT | QUIC no proporciona ICE/STUN/TURN automáticamente | ICE + STUN/TURN integrado en el diseño | Sin traversal integrado; ICE-TCP/relay requieren otra implementación |
| UDP bloqueado | Requiere fallback | ICE-TCP/TURN TCP/TLS según implementación | TCP directo puede funcionar; no supera CGNAT por sí solo |
| Latencia | Handshake integrado; 0-RTT no necesario ni deseable para grants | Negociación ICE/DTLS/SCTP | TCP + TLS; más handshakes, poco relevante para descarga completa LAN |
| Throughput | Control de congestión/flow control en userspace | Congestión SCTP; fragmentación y bufferedAmount | Congestión kernel TCP y backpressure request/response |
| Resume | Aplicación debe conservar/verificar bloques | Aplicación debe conservar/verificar bloques | Aplicación debe conservar/verificar bloques |
| CPU/memoria | No medido; buffers/ventanas y crypto configurables | No medido; stack y fragmentación a limitar | Medido en LAN: 3.05–3.19 MiB/s; un worker, buffer 1 MiB + TLS + checkpoint |
| Mantenimiento | Excelente candidato para multiplexación/WAN; UDP e integración ICE por resolver | Mayor superficie de implementación ahora; voz futura no decide archivos | Menor alcance funcional para un único archivo/peer; no requiere SCTP/SDP |
| Voz futura | No comparte automáticamente stack WebRTC | Puede compartir PeerConnection; congestión conjunta exige medir | Transporte independiente; preservar prioridad del Player/voz |

Elegido para el spike: **TCP + TLS 1.3**, con Rustls/ring y certificados efímeros
rcgen. No se usa verificador que acepte cualquier certificado, TLS 1.2, 0-RTT ni
resumption TLS. El receptor valida el certificado específico entregado por un
canal de confianza y el nombre fijo cine-transfer.local. El emisor exige un
bearer aleatorio independiente por conexión, ligado a manifest/room/epoch/member/
media/autoridad. La integración de producto entrega ese material por WSS
autenticado y lo revoca con la membresía. ADB en el spike es un canal privado de
configuración, no discovery ni señalización de producto.

Quinn no se rechaza por rendimiento: no se ha medido. Sus streams adicionales y
migración no son necesarios para un único archivo LAN. WebRTC no se rechaza por
calidad: su ventaja real es ICE y el ecosistema browser; esta fase no necesita
browser ni la implementación de voz. El protocolo de bloques queda independiente
del transporte para reconsiderar QUIC/WebRTC en Phase 2 con pruebas entre redes.
HTTP range, torrents/swarm y relay central de contenido se excluyen del alcance.

## Fuentes primarias

- [Quinn README](https://github.com/quinn-rs/quinn/blob/main/README.md).
- [Quinn certificados](https://quinn-rs.github.io/quinn/quinn/certificate.html).
- [QUIC RFC 9000](https://www.rfc-editor.org/rfc/rfc9000.html).
- [WebRTC DataChannels RFC 8831](https://www.rfc-editor.org/rfc/rfc8831.html).
- [Transports WebRTC RFC 8835](https://www.rfc-editor.org/rfc/rfc8835.html).
- [Rustls y proveedores crypto](https://docs.rs/rustls/latest/rustls/).
- [rcgen](https://docs.rs/rcgen/latest/rcgen/).
- [ICE RFC 8445](https://www.rfc-editor.org/rfc/rfc8445.html).
- [TURN RFC 8656](https://www.rfc-editor.org/rfc/rfc8656.html).
- [Android SAF](https://developer.android.com/training/data-storage/shared/documents-files).
- [ContentResolver](https://developer.android.com/reference/android/content/ContentResolver).
