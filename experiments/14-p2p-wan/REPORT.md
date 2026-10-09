# INFORME — P2P MEDIA DISTRIBUTION PHASE 2
## Internet Connectivity & NAT Traversal

Clasificación final: **BLOCKED para WAN real**, por infraestructura confirmada ausente.
Investigación y spike local completos; no deployment ni migración fundamental.
Estados explícitos: IMPLEMENTED, TESTED, MEASURED, VISUALLY VERIFIED, INFERRED,
PROVISIONAL, NOT IMPLEMENTED, NOT TESTED, BLOCKED.

### 1. Estado inicial

TESTED: master limpio, HEAD b41f3c03854767fed1456ac3c917c90977c9a45b; origin Eirom16/cine-virtual, gh autenticado. Actions inicial37966817288:11/11 success. Usuario confirmó ausencia de WSS público y relay autorizado; no infraestructura externa modificada.

### 2. Auditoría del P2P existente

TESTED: Phase1 Rust TCP/TLS1.3, grant WSS, scope de sala/member/epoch/Host/revisión/manifest, bloques1MiB y SHA, Partial, cancel/resume vivo. Core sigue dueño de autoridad/Ready/SyncEngine; Flutter presenta, Kotlin SAF/Media3 permanece intacto.

### 3. Limitaciones LAN anteriores

IMPLEMENTED v1 permite IPv4 privada/loopback; anuncio depende de IP local WSS. No ICE ni hole punching; snapshot opt-in difunde oferta dentro de sala. Candidatos WAN privados requieren nueva capacidad negociada, no reutilizar ese snapshot.

### 4. Alternativas investigadas

TESTED investigación de documentación oficial, crates publicados/Cargo.toml/API y actividad upstream. A TCP/TLS, B ICE/WebRTC, C Quinn+traversal externo, D híbrido; [decisión](TRANSPORT-DECISION.md), [matriz](CONNECTIVITY-MATRIX.md). Sin benchmarks inventados.

### 5. TCP/TLS

PROVISIONAL recomendado: mantener TCP directo y añadir túnel con dos conexiones salientes. WAN directo necesita endpoint alcanzable/IPv6/forwarding autorizado. Kernel aporta congestión; TCP no promete perforar NAT simétrico/CGNAT.

### 6. WebRTC DataChannels

INFERRED potencial mayor éxito directo mediante ICE UDP; fiables/ordenados sobre DTLS/SCTP. Migración cambia carrier/identidad criptográfica y necesita consentimiento. API webrtc0.21/rtc0.21 inspeccionada; TURN TCP/TLS no demostrado por código seleccionado, ARMv7/buffers/CPU pendientes.

### 7. QUIC

Quinn0.11.12: TLS1.3/streams/migración, UDP. NOT IMPLEMENTED: ICE y relay externos necesarios; QUIC por sí mismo no resuelve CGNAT. No migración ni dependencia añadida.

### 8. ICE

RFC8445 y TCP RFC6544 investigados. NOT IMPLEMENTED: no checklist, nomination ni ICE restart. Probar un endpoint TCP no se etiqueta ICE; gathering/ICE ms nulos en JSON.

### 9. STUN

RFC8489 investigado: descubre mappings/reflexive, no garantiza ruta directa universal. NOT IMPLEMENTED, ningún servidor STUN público usado; no candidato srflx inventado.

### 10. TURN y relays

RFC8656/coturn investigados. TURN corresponde a stack ICE/UDP compatible, no se conecta sin adaptación a TCP carrier existente. IMPLEMENTED relay experimental TCP con TLS exterior de admisión y TLS interior end-to-end; servicio público NOT IMPLEMENTED.

### 11. Matriz comparativa

[Matriz](CONNECTIVITY-MATRIX.md) cubre LAN/WAN, NAT/CGNAT, IPv4/6, UDP bloqueado/firewall, cifrado/auth/MITM, plataformas, CPU/RAM/throughput/dependencias/licencias/runtime/self-hosting/costes. Documentación, inferencia y medición distinguidas; nuevos transportes no benchmarked.

### 12. Dependencias Rust evaluadas

[Snapshot](dependency-research.json): webrtc/rtc0.21, webrtc-ice/stun/turn0.17.2, str0m0.24.1, Quinn0.11.12, Rustls0.23.45, datachannel0.16.1, iroh1.3, rice-proto0.4.3. Fuentes/licencias/MSRV/actividad/API examinados. Cargo.lock actual no actualizado; ice crate homónimo CTF descartado.

### 13. Compatibilidad multiplataforma

IMPLEMENTED sin dependencia nueva ni cambio de mínimos. Tokio/control intactos, workers blocking acotados. CI final debe validar Linux/Windows/macOS Intel+ARM/Android tres ABI/iOS dos targets. Runtime multimedia fuera de Linux/Android previo NOT TESTED.

### 14. Compatibilidad Android ARMv7/ARM64

TESTED físico SM-J701M Android9 API28 ARMv7, release carrier nuevo, loopback8KiB/8MiB SHA. ARM64 Android15 físico NOT TESTED; build no equivale runtime. No abandono ARMv7.

### 15. Transporte recomendado

PROVISIONAL A+D: TCP/TLS existente más stream relay aditivo. Reduce riesgo inmediato y permite CGNAT mediante salientes si firewall admite relay; preferencia directo depende de alcance real. WebRTC puede revisarse si coste de relay justifica migración.

### 16. Decisión arquitectónica

[ADR-015](../../docs/DECISIONS.md#adr-015--fundamento-wan-aditivo-y-relay-tcp-end-to-end-provisional). Provisional; investigación no autoriza reemplazar carrier ni desplegar público. Solo spike reversible y preparación operativa.

### 17. Cambios de transporte implementados

IMPLEMENTED receive_on/observed y sender genérico sobre Carrier; wrappers TCP conservados. Relay finite serve_pair, tickets separados, TLS nested, cuotas/plazos, cierre graceful. Bin cine-wan-spike y scripts reproducibles sin servicio público.

### 18. Abstracción del carrier

IMPLEMENTED pequeña Read+Write/configure/close; TCP y stream TLS exterior. Sin lógica de Ready, media identity, autoridad o ruta en la interfaz; mismo código chunks/Partial/crypto/backpressure.

### 19. Servidor de señalización

IMPLEMENTED startup --advertise separado de --bind, carga identidad persistente DER/PKCS8, validación WSS. RoomService sigue coordinador sin bytes de película. Asignaciones privadas relay/candidatos/generation NOT IMPLEMENTED.

### 20. WSS y certificados

TESTED pin exacto/SAN/TLS1.3 preservados, identidad provisionada rechaza key mismatch. [Self-hosting](SELF-HOSTING.md) usa TLS nativo y passthrough TCP443; persistir cert estabiliza pin, no vuelve desarrollo automáticamente confiable. PKI sin pin requiere decisión posterior.

### 21. Candidate gathering

NOT IMPLEMENTED. No nuevas interfaces expuestas ni candidatos host/srflx/relay wire. Lab obtiene direcciones privadas provisionadas explícitamente; no es gathering ni capacidad producto.

### 22. Candidate exchange

NOT IMPLEMENTED WAN. Roadmap exige entrega privada después de aceptación, scope/grant/generation/TTL/cancel/size/count y negociación explícita; antiguos v1 mantienen LAN. Sin protocolo v1 breaking.

### 23. Connectivity checks

TESTED intento TCP directo acotado5s a NAT de laboratorio, firewall rechaza y counters confirman. No ICE; flujo lab luego autoriza relay y auth interior. Checks entre redes reales NOT TESTED.

### 24. Direct LAN

TESTED físico Linux→Android9:8KiB/8MiB SHA en muestra exitosa; [LAN](results-lan.json). Cortes posteriores tanto fuente vieja como nueva y rates bajos documentados, estabilidad pendiente. Producto libmpv/Ready probado por separado en loopback.

### 25. Direct WAN

NOT TESTED/BLOCKED. No WSS público ni endpoint P2P autorizado alcanzable desde redes independientes. No loopback/LAN/VPN/netns cuentan como Gate2.

### 26. NAT traversal

TESTED solo simulación: dos NAT masquerade y firewall stateful en namespaces rootless; salientes al relay funcionan, directo falla. No TCP hole punching implementado ni promesa de directo universal.

### 27. CGNAT

INFERRED: relay saliente puede superar falta de entrada por CGNAT si salida permitida. Physical CGNAT NOT TESTED. Router doméstico no controla NAT del operador; no forwarding mágico.

### 28. IPv6

TESTED relay pequeño ::1 y pin interior/exterior. IPv6 global/WAN/gathering/firewall operador NOT TESTED. Oferta producto v1 sigue IPv4 privada.

### 29. Relay fallback

TESTED secuencia laboratorio directo bloqueado→relay→pausa/corte→tickets/grant nuevos→bloques restantes→SHA. NOT IMPLEMENTED fallback automático en aplicación; no daemon público ni allocation RoomService.

### 30. Seguridad del relay

TESTED dos tickets CSPRNG de un uso, secretos constantes/scopes/expiración/revocación,2admisiones, acotación16KiB y cuotas ciphertext. Rechaza wrong pin/secret/epoch/replay/oversize/quota. Seguridad servicio público/DDoS/global rate NOT TESTED/IMPLEMENTED.

### 31. Cifrado extremo a extremo

IMPLEMENTED TLS interior Host↔receiver con pin independiente dentro de TLS exterior peers↔relay. Relay solo posee clave exterior, recibe ciphertext; tiempos/tamaños/IPs visibles y puede negar servicio. No sustituir interior por doble TLS hop.

### 32. Autenticación de peers

TESTED misma identidad efímera cine-transfer.local y pin por señalización confiable, credential antes de chunks; relay impostor Host rechazado cero bytes verificados. Trust server WSS vigente.

### 33. Autorización de transferencias

IMPLEMENTED Credential room/epoch/receiver/transfer/media/authority/manifest vincula descarga. Host/media incluidos en identidad/manifest y autoridad; sistema de grants no duplicado en Flutter. Lab relay provisioning manual privado, no autoridad producto nueva.

### 34. Credenciales y revocación

TESTED nuevo grant/tickets tras corte/pausa; consume una vez, deadline monotónico, compare constante/revocation. Corrige sender que escogía grant revocado antes de nueva entrega WSS; regression socket y producto PASS.

### 35. Route selection

IMPLEMENTED solo evidencia lab route RELAYED y baseline DIRECT_LAN. NOT IMPLEMENTED route state machine/selector producto WAN ni clasificación de topología por IP; diagnósticos JSON honestos, no direct label falso.

### 36. Timeout/retries

IMPLEMENTED TCP connect5s, inner handshake5s/idle3s, outer admission absolute10s/client y≤5s/server, rendezvous10s, default relay60s/idle3s. Finite límites y cancel tests; plazos iniciales PROVISIONAL, no tuning universal de Internet. Un retry explícito con grant nuevo.

### 37. Cambio de red

NOT IMPLEMENTED recuperación móvil automática. Socket antiguo debe cerrarse, conservar Partial, obtener grant/asignación nuevos; NetworkCallback/redetect detrás de consentimiento. No ICE restart ficticio. Wi-Fi↔datos físicos NOT TESTED.

### 38. Reanudación

TESTED checkpoint vivo:1MiB de8 conservado12.5%, transmite7 restantes, SHA final. Baseline WSS Hostdisconnect/resume nuevo grant/libmpv Ready. Process-death resume y automatic fallback NOT IMPLEMENTED.

### 39. Manifest/chunks/SHA

IMPLEMENTED existente sin duplicación:manifest versionado, bloques1MiB SHA256 individual/final, scope/fingerprint/missing revalidate/commit atómico. Corpus sintético temporal8KiB/8MiB y medio propio24,300,725B; no archivo ajeno en Git.

### 40. Backpressure

IMPLEMENTED streams blocking en workers, buffers TLS64KiB/relay16KiB, kernelTCP congestión y timeout. No colas ilimitadas ni protocolo propio congestion. Thread/FD métricas en resultados, no Flutter isolate ni Media3 Looper usado para binarios.

### 41. Integración con RoomService

Conservada y TESTED baseline producto; mínima corrección espera grant Client. NOT IMPLEMENTED relay allocation y entrega privada WAN; bytes nunca por JSON WSS ni almacenamiento servidor.

### 42. Integración Flutter/Rust

Conservada, TESTED Flutter analyze/68 tests. No cambio de autoridad, FFI o panel para relay aún. Spike Rust aislado; no afirmar integración WAN mediante fakes.

### 43. Android lifecycle

Política background pause conservada; no foreground service/permisos nuevos. Nuevo carrier probado local release ARMv7; foreground/SAF/Media3 sobre WAN NOT TESTED, no cambios a propiedad FD de Kotlin.

### 44. UI Host

UI existente ofrecer/aceptar/cancelar permanece. NOT IMPLEMENTED oferta WAN privada/relay consent. Sin rediseño ni cambios Rich Chat.

### 45. UI Participant

UI existente solicitar/progreso/pause/resume/retry permanece. NOT IMPLEMENTED consentimiento móvil y selección relay. Descargas físicas solo Wi-Fi y carrier loopback, no datos móviles.

### 46. Connection status UX

NOT IMPLEMENTED direct/relay statuses producto; roadmap conserva mensajes simples y errores útiles separados. Bin marca RELAYED real; no muestra ICE o P2P directo inexistente.

### 47. Seguridad y privacidad

[Security](SECURITY.md) evidencia/amenazas/límites; no secretos, URI SAF, cert/privatekey/IP personal en resultados. IP metadata solo autorizados requiere evolución v1 privada; no difusión WAN añadida. Capturas privadas ausentes.

### 48. Self-hosting

IMPLEMENTED documentación y templates, NOT DEPLOYED: [SELF-HOSTING](SELF-HOSTING.md), native WSS certpersistente/systemd/HAProxy TCPpassthrough. Relaypolicy es PROPOSED, no daemon runnable; requisitos mínimos e integración pendientes explícitos.

### 49. Costes operativos

INFERRED aritmética:10 archivos2GiB≈20GiB salida/40GiB agregado+overhead. Contrato puede cobrar ambos; no precio comercial inventado. Tickets/cuotas porpair y global/outbound/budget antes de público; ningún servicio contratado.

### 50. LAN regression

TESTED funcional8KiB/8MiB con Android antiguo, cancel/resume/SHA/Ready/libmpv cliente real loopback. Estabilidad física posterior/velocidad histórica no confirmadas; resultados FAIL retenidos. No repetir ensayo600s sin regresión temporal.

### 51. WAN test environment

BLOCKED: Linux y Android solo misma Wi-Fi, no WSS/relay público. Namespaces/netem aislados como simulación, sin cambio firewall host/router. Prerrequisito próximo dominio/host autorizado y TLS reachability en dos salidas independientes.

### 52. WAN direct real test

NOT TESTED: [JSON](results-wan-direct.json). Gate2 no PASS. Medir candidatos/route/auth/bytes/SHA con corpus pequeño una vez recursos autorizados.

### 53. Relay real test

NOT TESTED: [JSON](results-relay.json). Gate3 no PASS; falta servicio público revisado/autorizado, quotas y señales privadas. NestedTLS local no equivale prueba relay entre casas.

### 54. Network emulation

TESTED netns/veth/nft masquerade/counters; netem40ms±10ms/0.5%loss/10Mbit egress NAT. [JSON](results-simulated-netem.json). No reordering extra probado ni NAT simétrico/CGNAT físico simulado como hecho real.

### 55. Disconnect/recovery

TESTED [corte](results-recovery.json): socket TCP crudo corta tras1MiB, error I/O y reauth explícita;7MiB restantes/hash. Recovery hasta complete7.891s, no automatismo producto. Control-plane recovery separado en baseline libmpv.

### 56. Throughput

MEASURED resume netem0.503MiB/s, ARMv7 loopback0.698MiB/s y LAN física0.277MiB/s muestra. Entornos/compilación/hostload distintos, no ranking ni regresión demostrada; [RESULTS](RESULTS.md) limita interpretación/overhead.

### 57. CPU

MEASURED lab todosroles+fixture+hash:netem2.732s CPU, corte2.808s; ARMv7 8MiB4.70s. No CPU incremental WebRTC/QUIC ni durante Player WAN, NOT MEASURED.

### 58. Memory/resources

MEASURED netem peakgetrusage17,016KiB; muestreo100ms max11FD/4threads/12,704KiB RSS. ARMv7 peak10,844KiB; al finalizar5FD/1thread. No leaktest largo ni coste marginal3rolesaislado inferido.

### 59. Impacto en Player

TESTED pipeline libmpv/SHA/Ready por baseline control real; SyncEngine/Player no editados. Chat/reactions/fullscreen regresión Flutter cubierta donde existentes. Media3 y reproducción sincronizada durante WAN real NOT TESTED.

### 60. Tests Rust

TESTED150 PASS/8opt-in ignored;fmt/clippy -Dwarnings/test/build --locked. Transfer11 tests, relay9 (Unix;8 Windows), config2 y real socket grant race. Sin red pública obligatoria ni cambio de locks.

### 61. Tests Flutter

TESTED analyze y68 tests existentes. No nuevas UI WAN, por tanto tests de estados WAN/mobileconsent NOT IMPLEMENTED; no fake presentado como integración física.

### 62. Security tests

TESTED pinouter/inner, impostor, credential scope/replay/revoke/expire/oversize/quota/cancelhandshake y suitesPhase1. Candidatos/generation/networkchange/allocationWrongRelay WSS producto NOT IMPLEMENTED/TESTED. Auditoría DDoS/público pendiente.

### 63. Linux ↔ Android physical tests

TESTED Linux→Android9 direct LAN8KiB/8MiB y nuevo relay local ARMv7. No nueva Android→Linux WAN, no ARM64 físico ni Internet. Evidencia anterior Phase1 no reetiquetada.

### 64. Windows/macOS/iOS

CI crossbuild verifica binario/adapter/lib compile; no runtime multimedia ni sockets WAN físicos. No eliminación target ni libsC++ nuevas. Resultado final exacto por run del HEAD, no asumir 11/11 inicial.

### 65. Screenshots

NOT VISUALLY VERIFIED nuevas vistas, ninguna UI cambiada ni nuevas capturas. [screenshots](screenshots/README.md) delimita alcance; imágenesPhase1 no prueban Phase2.

### 66. Visual QA

NOT TESTED nueva UI WAN. Baseline real libmpv/load/Ready es evidencia funcional automatizada, no QA visual Android entre casas.

### 67. Bugs encontrados

TESTED reproducibles: grantrevocado race bloqueaba resume, configJSON parcialmente escrito, nft reservednames y Uri badport. CI macOS ARM64 además mostró EINVAL en poll_config; [diagnóstico](results-ci-macos-diagnosis.json). Primer ARMv7 pequeñoIo y LAN variable no causaaislada; resultados iniciales FAIL conservados.

### 68. Bugs corregidos

IMPLEMENTED espera authavailable con consume obligado, bounded configparse, nft cadenas seguras y namespaceguard, advertise valida puerto, TLS close_notify explicit inner/outer. Relay fija timeout UNA VEZ antes de aceptación, eliminando reconfiguración tras cierre observada en macOS ARM64. Revisión final restaura revalidación antes del connect TCP para parciales grandes. Unit/integration/product finales PASS; no atribuir Io Android solo al cierre.

### 69. Limitaciones

BLOCKED infraestructura WAN. Allocation/privatesignaling/candidatos/route UX/networkchange/automaticfallback no implementados; relay finite lab no público. Grants actuales10min limitan ficheros largos sobre WAN lenta; revisar renovación sin reutilización. LAN física intermitente requiere diagnóstico. [Timeout CI social puntual](results-ci-social-timeout.json) no reproducido localmente; causa pendiente, no afirmamos corregirlo por una reejecución verde.

### 70. Regresiones

150 Rust/68 Flutter PASS y baseline producto funcional; Core/SyncEngine/Player/RichChat intactos. LAN física completó antes pero cortes viejos+nuevos posteriores; no declarar estabilidad universal ni mismos3MiB/s.

### 71. GitHub Actions

TESTED: commit de código bb6ec172eab523b6107cef5f27e6495a84a8f7c0, [run38001263654](https://github.com/Eirom16/cine-virtual/actions/runs/38001263654), **11/11 success** después de reejecutar Linux. [Recibo machine-readable](results-ci-code-final.json). Timeout puntual del test WebSocket social existente: cinco repeticiones locales sin cambios PASS, reejecución CI PASS; causa NOT DETERMINED. Una descarga Ubuntu lenta se interrumpió y relanzó, no error de código. No SyncEngine/test/timeouts modificados por ese episodio. El commit posterior solo añade este cierre documental; su CI exacto se entrega en respuesta final.

Inicial37966817288 11/11 PASS solo b41. Run37997804755 detectó fallos al join del relay tras SHA completo en Windows/macOS ARM64; [diagnóstico preservado](results-ci-first-failure.json). Corrige clasificación del fin de sesión/reset, sin omitir tests. Cambios se commit/push y matriz final se espera/diagnostica; run exacto del HEAD y estado final se entregan en respuesta de cierre. No continue-on-error, secrets, releases o deployment modificados.

### 72. ADR/documentación

IMPLEMENTED ADR015 provisional y docs architecture/P2P/security/protocol/UI/testing/CI. Experimento14 independiente con research, decision, matrix, security, spike/results/roadmap/selfhost y JSON diferenciados; experimento13 preservado.

### 73. Commits

TESTED: base b41f3c0; commits170f9fa (investigación/carrier/spike), fb5002a (prevalidación/grants/Ready ensayo), 8e9dceb (teardown), 9bbbfb9 (diagnóstico seguro), 6994121 (polling previo a ACK), bb6ec17 (evidencia final de código). Todos incorporados en master por commits normales/fast-forward. El cierre posterior solo actualiza documentación/recibos. Hash final se entrega en cierre y git log; no forcepush/rebase/borradohistoria. Docs no inventan su propio hash autoreferente.

### 74. Estado Git final

TESTED antes de este cierre documental: master limpio y sincronizado con origin/master en bb6ec17, ahead/behind0/0; CI11/11. El HEAD posterior de documentación se verifica nuevamente y comunica en cierre. Estado final exacto se comunica en cierre; modificaciones externas no sobrescritas. No certificados/grants/media privada tracked.

### 75. ¿Funciona P2P entre casas diferentes?

**NOT TESTED / BLOCKED.** Esta fase no demuestra todavía dos casas. Construye fundamento local real con TLS end-to-end y recuperación; no WAN TRANSPORT/PRODUCT/SPIKE PASS por simulador.

### 76. ¿Cuándo se necesita relay?

INFERRED cuando no existe ruta entrante directa válida:CGNAT, NAT restrictivo, firewall/UDPbloqueado, IPv6 noalcanzable. Relay saliente TLS funciona solo si salida/hostpolítica lo permite; puerto443 no garantía contra proxyinspección.

### 77. ¿Qué falta para uso general por Internet?

Autorizar host/dominio/cert WSS y relay separado, servicio relay endurecido con allocation privada/globalquotas, capability WAN/routes/scopes/generation/consent/errors; luego corpus pequeño redesindependientes/CGNAT/IPv6 y Player/Ready. No despliegue automático.

### 78. Recomendación de siguiente fase

Primero decidir host/dominio operativo manteniendo pinTLS nativo/passthrough. Completar allocation/privatesignaling y daemon relay sujeto a cuotas antes de publicar. Si prioridad es reducir egress/direct UDP, revisar autorización spike WebRTC compatible ARMv7 con TURN TCP/TLS real; no construir tres stacks.

### 79. Resumen para otro arquitecto

**BLOCKED WAN real; investigación PASS, spike local IMPLEMENTED/TESTED.** TCP/TLS/chunks/Partial autoridad conservados; túnel con tickets separados y inner peerTLS validado NAT/netem/ARMv7loopback. No ICE ni WANproducto escondidos; el siguiente gate requiere infraestructura autorizada y señalización privada. Ver decisión/resultados/selfhosting para reproducir sin secretos.

