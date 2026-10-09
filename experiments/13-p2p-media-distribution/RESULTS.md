# Resultados — P2P Media Distribution Phase 1

La clasificación distingue implementación, ejecución y medición. Las pruebas
físicas usan red LAN real y los Players existentes; no mocks de descarga ni de Ready.
La implementación de código queda en 668a75c y sus cinco commits anteriores.
Los reportes del primer ensayo contienen source_base_commit: identifica la base
antes de ajustes locales, no un claim de que esos archivos estaban limpios en Git.

## Transporte

| Ensayo | Tamaño | Resultado | Tiempo receptor | Velocidad efectiva |
| --- | --- | --- | --- | --- |
| Linux → Android | 8 KiB | TESTED PASS | 0.123 s | Overhead domina; no benchmark |
| Linux → Android | 32 MiB | TESTED / MEASURED PASS | 10.478 s | 3.05 MiB/s |
| Linux → Android | 128 MiB | TESTED / MEASURED PASS | 40.148 s | 3.19 MiB/s |
| Android → Linux | 32 MiB | TESTED / MEASURED PASS | 10.357 s | 3.09 MiB/s |

Fuente: results-performance.json y results-android-linux-transport.json.
Misma Wi-Fi: Linux 192.168.1.26 y Android 192.168.1.4, /24 al realizar el spike.
TCP1730 entre pares; WSS1729 para producto. Android físico SM-J701M/API28/armv7;
Linux bin debug, Android carrier/Rust release, Flutter debug. Emisor limitado
8 MiB/s; no comparar con QUIC/WebRTC, ni interpretar como límite de hardware.
No relay, adb reverse, STUN/TURN ni servidor de contenido.

Linux transporte 128 MiB: CPU proceso 10.36 s y pico RSS 7260 KiB, 1 thread,
6 FDs observados. RAM de transporte no incluye Player ni Flutter. Android CPU/PSS
no se midieron en el shell spike; el ensayo de producto los registra aparte.
El primer intento hardlink falló en el filesystem Android; el resultado FAIL se
conserva. Se sustituyó por renameat2 RENAME_NOREPLACE y se repitió con PASS.

## Producto y recuperación

Linux → Android 256 MiB: TESTED PASS, results-product-linux-android.json.
Pausa a 10 MiB verificados (3.9%), no a 100%; conserva el contador. Resume con
nuevo grant. Wi-Fi deshabilitado después de 11 MiB observados: se detuvo a 17 MiB
por bytes en vuelo/latencia de observación. Wi-Fi reactivado, sala reanudada,
autorización nueva y solo bloques faltantes. SHA exacto, commit seguro, hashing
LocalMedia existente, Media3 cargado, identidad coincidente y Ready explícito.
Ambos Players ejecutaron Play/Pause/Seek, fullscreen Android, chat/reacción durante
transferencia y background/foreground con revalidación. Descarga con pausas y
verificación: 101.182 s; no usar como throughput continuo.

Linux RSS/PSS Android observados: antes 78048/224882 KiB; pausa 79260/233503 KiB;
después del smoke 80028/202012 KiB. Linux: 22 threads / 17 FDs. Android CPU de proceso: 65.61% de un core en intervalos de transferencia y 79.30% durante SHA (no porcentaje total del dispositivo); detalle en results-resources.json. Android CPU/threads/FDs
están en samples; métricas de proceso completo debug, no costo aislado del carrier.
Los resultados no demuestran estabilidad de larga duración ni scalability multiusuario.

Android → Linux: informe físico independiente results-product-android-linux.json,
archivo 128 MiB seleccionado realmente por SAF. Host UI consent/approval, conexión
saliente mediante FD duplicado; Linux pausa antes de completar, nueva autorización,
SHA final, libmpv y Ready. No sustituir esta prueba por el spike inverso de 32 MiB.
PASS en el flujo inverso. La clasificación definitiva de cada escenario está en el campo status de su JSON.

Lifecycle Linux real: results-lifecycle.json PASS: cancelar mientras espera elimina
parcial; disconnect receptor detiene socket; Cancel funciona offline y elimina
parcial; ROOM_RESUME sin Ready inventado; disconnect Host revoca y conserva datos;
Host resume/nuevo grant completa SHA; carga libmpv/Ready; nueva selección invalida
manifest/partial y Ready antiguo. WSS/TLS/procesos reales, loopback; no evidencia Android.

## Multimedia bajo carga

El ensayo inverso incorpora playback_transfer_load (PASS, 32 MiB en 8.821 s): ambos Players reproducen
su película válida mientras otro corpus propio 32 MiB atraviesa el carrier TLS
independiente. Se observan posiciones, playing, CPU/RSS/FDs y velocidad. Es carga
de ingeniería autenticada con credencial del spike; no un segundo offer del mismo
RoomService. El producto solo ofrece su revisión multimedia vigente. No afirmar
que soporta distribuir otro medio dentro de una sala sin cambiar su selección.
Drift absoluto de snapshots Android durante carga: mediana 140.0 ms, máximo 198 ms; muestreo asíncrono500ms, no timing de frames ni comparación simultánea de posiciones. Detalle en results-playback-impact.json.
No se repitió el ensayo de sincronización de 600 s ni se modificaron thresholds.

## Regresiones y evidencia

137 tests Rust PASS, 8 SDK opt-in ignorados ya existentes; 68 tests Flutter PASS;
10 tests Python CI/timing PASS. fmt, clippy -D warnings, workspace build locked,
Flutter analyze, docs y CI structure PASS locales. Los tests de transporte usan
TLS real para pause/new-grant/resume, pin erróneo, credential errónea, header/payload
malicioso y trickle absoluto; pure tests cubren scopes, expiry, dedupe, corruption,
checkpoints y SHA. Capabilities/WSS/legacy WS ejercitados con sockets reales.
La verificación GitHub Actions del último HEAD se informa al cerrar la sesión;
ningún PASS se hereda del run inicial para el código nuevo. El primer run nuevo detectó el modo no bloqueante heredado en Windows/macOS; reproducción y corrección documentadas en [CI-DIAGNOSIS](CI-DIAGNOSIS.md).

Screenshots reales y evaluación en VISUAL-QA.md. Los fallos iniciales de harness
(pattern lock, selector/pause después de overlay timeout) no se reclasifican como
PASS. El ensayo Linux→Android128MiB pausó durante SHA, por eso no constituye la
prueba de retomar bloques faltantes: la de256MiB sí lo hace.

## Límites

| Función | Estado |
| --- | --- |
| Directo LAN Linux ↔ Android | IMPLEMENTED / TESTED |
| Consentimiento, chunks, SHA, resume, Ready/Player | IMPLEMENTED / TESTED |
| Host Linux UI / Host Android UI / receptor Android UI | VISUALLY VERIFIED |
| Receptor Linux CLI/libmpv | TESTED; UI receptora Linux no capturada físicamente |
| WAN directo / NAT traversal / STUN/TURN/relay | NOT IMPLEMENTED / NOT TESTED |
| Process-death resume / background prolongado | NOT IMPLEMENTED |
| Android export SAF / providers no seek | Export NOT IMPLEMENTED; rechazo implementado, providers diversos NOT TESTED |
| Windows/macOS/iOS producto P2P | NOT TESTED; iOS Player NOT IMPLEMENTED |
| Multiusuario throughput / batería / RTT aislado carrier | NOT MEASURED |
| Falta real de espacio / disco defectuoso / symlink atacante | Tests de límites; física destructiva NOT TESTED |

PROVISIONAL para el MVP LAN en las condiciones ensayadas. Sin discovery automático,
pins WSS locales manuales, cuotas conservadoras, un grant por sala, almacenamiento
privado y sin catálogo/cleanup de completados. Phase2 debe medir conectividad real
entre redes y operadores antes de anunciar soporte Internet.
