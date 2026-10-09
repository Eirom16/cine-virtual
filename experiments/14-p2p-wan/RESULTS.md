# Results — P2P WAN foundation

2026-10-09. **BLOCKED: WAN real**, confirmado por usuario: no existe WSS público
ni relay autorizado. **Gate 1 PASS** investigación; gates 2/3/4 WAN **NOT TESTED**.
Spike local y regresión funcional de producto **TESTED**; no WAN SPIKE PASS por
redes simuladas. Gate 5: checks locales pasan, CI final se verifica en el run del
HEAD publicado; rendimiento/estabilidad física LAN necesitan seguimiento.

## Evidencia separada

| Entorno / build | Corpus | Resultado | Medición |
| --- | --- | --- | --- |
| Linux debug → SM-J701M Android9 Phase1 release, misma Wi-Fi | 8KiB +8MiB | SHA PASS en muestra completa | 8MiB 28.921s, 0.277MiB/s |
| Linux loopback WSS/TCP, cliente real/libmpv debug | vídeo sintético propio 24,300,725B | cancel, disconnect, nuevo grant, SHA, load, Ready, nueva revisión PASS | No benchmark Player |
| Dos NAT IPv4 rootless, sin netem, Linux debug | 8MiB, pausa tras1MiB | directo bloqueado; relay+resume SHA PASS | muestra inicial resume7MiB 2.446MiB/s |
| Dos NAT +40ms±10ms/0.5% loss/10Mbit por enlace externo | 8MiB | relay+pause+nuevo grant+resume SHA PASS | resume7MiB 13.914s, 0.503MiB/s |
| Dos NAT + corte TCP crudo tras1MiB, sin netem | 8MiB | error I/O esperado, nuevo grant, SHA PASS | resume7MiB 7.876s, 0.889MiB/s |
| ARMv7 release, SM-J701M, relay en loopback del dispositivo | 8KiB +8MiB | SHA PASS, pausa/nuevo grant/resume | resume7MiB 9.841s, 0.711MiB/s |
| Redes físicas independientes / CGNAT / IPv6 Internet | — | **NOT TESTED / BLOCKED** | ninguna |
| Relay público autorizado | — | **NOT TESTED / BLOCKED** | ninguna |
| ARM64 físico / Windows/macOS/iOS runtime | — | **NOT TESTED** | CI comprueba build, no runtime multimedia |

[LAN](results-lan.json), [producto](results-product-lan-loopback.json),
[NAT inicial](results-simulated-nat.json), [netem](results-simulated-netem.json),
[corte/recovery](results-recovery.json), [ARMv7](results-android-local-relay.json),
[WAN directo](results-wan-direct.json), [relay WAN](results-relay.json).

## Condiciones e interpretación

Datos sintéticos, bloques1MiB, SHA por bloque/final, mismo manifest/Partial,
Rustls/ring y Cargo.lock conservados. Host toolchain en
[checks](results-local-checks.json). ARMv7 release NDK28.2.13676358 API21;
binario completo1,959,108B, **no incremento atribuible al carrier** ni comparación
con builds WebRTC/Quinn. Android9 API28; Android15 ARM64 no disponible.

Rates son bytes útiles NUEVOS divididos por tiempo de intento incluido admisión,
autenticación, checkpoint y SHA; no equivalen al rate instantáneo de transporte
Phase1 (~3MiB/s). Comparación de transportes bajo condiciones iguales **NOT
MEASURED**. Primera muestra NAT precede cierre TLS instrumentado final; netem,
recovery y Android corresponden al carrier final. Recovery/netem coincidieron
con otras verificaciones/compilación en host; diferencias no prueban regresión.

Wi-Fi físico variable: [primera muestra](results-lan-first-sample.json)8MiB
20.410s frente28.921s posterior. Ensayos comparativos posteriores con
[fuente antigua](results-lan-old-source-comparison.json) y
[actual](results-lan-current-comparison.json) completaron8KiB pero fallaron8MiB
con I/O. **Estabilidad y causa del rendimiento bajo no resueltas**; no esconder
fallos ni declarar equivalencia al benchmark histórico. No más corpus grande
para perseguir una cifra. Funcionalidad LAN demostrada en muestra exitosa;
seguimiento controlado de Wi-Fi/timeout es pendiente antes de release general.

## Tiempos y recursos medidos

Netem: intento directo5.007s termina como fallo esperado; admisión exterior
206.4ms, auth interior254.3ms y primer chunk3053.9ms en resume. Recovery hasta
completar13,921.4ms, conserva12.5% (1/8MiB), un reintento explícito autorizado.
Corte crudo: directo5.004s; recovery7,890.7ms hasta SHA completo, mismo12.5%.
No gathering/ICE: campos nulos. Ningún éxito directo WAN medido.

Netem: CPU2.732s, peak RSS getrusage17,016KiB; son **todo el proceso laboratorio**
(fixtures/hash + sender + receiver + relay), no coste marginal de un peer.
Muestreo /proc100ms: máximos observados11FD/4threads/12,704KiB RSS (no pico absoluto).
Corte: CPU2.808s, peak RSS16,924KiB, máximos muestreados11FD/4threads/12,704KiB.
Tras join:5FD/1thread en ambos; no prueba de fuga de largo plazo.
ARMv7 8MiB: CPU4.77s, peak RSS10,964KiB,5FD/1thread al terminar;8KiB0.17s/3736KiB.
Player/Flutter no corrieron dentro de estos procesos, impacto WAN **NOT MEASURED**.

Netem resume:7,352,682B ciphertext interior retransmitido para7,340,032B útiles
nuevos (~0.172% extra). No incluye tickets/TLS exterior/IP/TCP/retransmisiones;
**overhead total de red NOT MEASURED**. Métrica de relay no es evidencia del
contenido en claro: su buffer contiene registros de TLS interior.

## Fallos observados y correcciones

- Resume de producto fallaba: sender escogía autorización revocada antes de que
  WSS entregara nueva. Corrige espera por autorización disponible; consumo TLS
  sigue validando secreto/scope. Test socket real + baseline producto PASS.
- Lectura del config de laboratorio antes de finalizar JSON: espera por parse
  completo con5s. [Fallo preservado](results-lan-config-race-failure.json).
- nft nombres reservados en primer lab: cadenas lab_post/lab_forward/lab_input;
  guarda namespace impide tocar host. [Fallo](results-simulated-nat-first-failure.json).
- Primer pequeño ensayo ARMv7 devolvió TRANSFER_Io, causa no aislada.
  Cierre inner/outer close_notify explícito y ensayos finales PASS; no atribuir
  causalidad exclusiva. [Primera evidencia](results-android-local-relay-first-attempt.json).
- Puertos inválidos en advertise podían perderse en Uri.port: valida autoridad
  explícita, WSS/SAN/persistencia cubiertos por tests.
- [Producto antes de corregir grants](results-product-lan-first-failure.json)
  permanece FAIL, no se reemplaza por evidencia posterior.

## Verificaciones

149 tests Rust PASS,8 opt-in ignored,68 Flutter PASS/analyze, fmt/clippy/build PASS.
8 nuevos tests relay reales TLS,11 transfer,2 config y regresión socket cliente
entre los tests workspace. No Internet obligatorio en CI; no sleeps largos ni
continue-on-error añadidos. Checks docs/CI y matriz final: ver run del HEAD
publicado, no reutilizar run inicial37966817288 como resultado de estos cambios.
No nueva QA visual ni capturas: [alcance](screenshots/README.md).
