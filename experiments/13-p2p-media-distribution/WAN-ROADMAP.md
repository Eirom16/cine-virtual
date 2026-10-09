# P2P MEDIA DISTRIBUTION — Phase 2: Internet Connectivity

Estado: PROVISIONAL / NOT IMPLEMENTED. Phase 1 solo conecta una dirección IPv4
privada alcanzable. No se desplegó STUN/TURN, relay ni infraestructura pública.

| Camino | Phase 1 | Necesidad futura |
| --- | --- | --- |
| LAN directo cifrado | Implementación TCP/TLS; pruebas en RESULTS | Wi-Fi sin aislamiento y firewall permitido |
| WAN directo | NOT IMPLEMENTED / NOT TESTED | Candidatos públicos, política de exposición, redes reales |
| NAT traversal | NOT IMPLEMENTED | ICE/candidate checks; NAT y CGNAT reales |
| STUN | NOT IMPLEMENTED | Descubrir mapping; no transporta ni garantiza contenido |
| TURN/relay | NOT IMPLEMENTED | Fallback autenticado, quotas y coste operativo |
| UDP bloqueado | NOT TESTED | Fallback TURN TCP/TLS o relay específico |

NAT traduce direcciones/puertos; mappings y filtros varían. CGNAT añade traducción
fuera del control del usuario. Conocer IP pública o abrir un listener no garantiza
alcanzabilidad. STUN no crea por sí mismo una ruta; ICE intercambia candidatos y
comprueba pares; TURN puede retransmitir si no hay directo. La documentación
primaria está en [ICE RFC8445](https://www.rfc-editor.org/rfc/rfc8445.html),
[TURN RFC8656](https://www.rfc-editor.org/rfc/rfc8656.html) y
[WebRTC transport RFC8835](https://www.rfc-editor.org/rfc/rfc8835.html).

## Spike comparativo independiente

1. Mantener manifest/identidad/autorización/almacenamiento y sustituir solo carrier.
2. Evaluar WebRTC DataChannels (ICE/DTLS/SCTP, messages fragmentados con bounds)
   frente a Quinn con candidate orchestration/traversal externo. QUIC no incorpora
   ICE/STUN/TURN automáticamente. TLS actual admite una conexión TCP alcanzable;
   integrar ICE-TCP exige biblioteca y checks nuevos, no cambiar solo una URL.
3. Medir dos redes domésticas, móvil CGNAT, NAT doble, firewall UDP bloqueado,
   IPv6 y roaming. Comparar builds/dispositivos/tamaños equivalentes y Player activo.
4. Seleccionar a partir de tasa de éxito directo, throughput, RAM/CPU, latencia,
   tiempo de recovery, mantenimiento y consumo de batería, no estética del protocolo.

## Coordinación, seguridad y fallback

Reutilizar sala WSS para candidatos y checks con capability adicional negociada.
Autenticar signaling y fingerprint; grants actuales mantienen binding a membresía,
epoch, media y consentimiento. Candidatos deben caducar/revocarse; no registrar IPs
privadas. Bloquear uso del servidor como proxy/SSRF hacia endpoints arbitrarios.
Controlar intentos, tamaño y rate por miembro/room/origen. Nunca convertir invite
ni credencial TURN en permiso genérico de archivo.

Si relay es necesario, ofrecer opción visible y self-hosting. Cuotas globales/
por usuario, tiempo, ancho de banda y transferencias; limitar relay allocations.
Mantener cifrado peer-to-peer encima del carrier donde corresponda. Definir
coste/retención y consentimiento de datos; no asumir servidores gratuitos permanentes.
Probar expiración/revocación, relay caído, fallo de credenciales, reconnect y
fallback sin mezclar bloques/media revisions. No desplegar infraestructura hasta
revisión explícita de diseño y autorización.

## Criterio de salida

Pruebas reproducibles entre redes y operadores distintos; matriz de NAT/directo/
relay/UDP bloqueado; autenticación negativa; CPU/PSS/batería y prioridad del Player;
Linux/Android y builds/runtime adicionales. Una prueba por IP privada nunca satisface
ese criterio. Voice y distribución swarm siguen fuera de esta fase.
