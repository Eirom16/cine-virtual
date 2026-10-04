# Spike C — Resultados y límites

Fecha: 2026-10-04. Base Git c5d2849. Prototipo **pre-v0.1**; no UI final,
no cliente de sala móvil. [Research previo](RESEARCH.md) justifica el gate.
Evidencia: [Android](results-android.json), [Linux UI](results-linux-ui.json),
[intentos diagnósticos](results-android-diagnostics.json) y
[regresión Linux](results-linux-regression.json). No URI, usuario, serial o
SHA-256 en resultados; el digest existe solo en memoria/frontera local del device.

## Clasificación

- Flutter: **PROVISIONAL**. UI/bridge ejecutados Linux y Android emulado.
- Bridge: C ABI manual v1 + dart:ffi suficientemente probado para **este ensayo**;
  no API pública ni validación de producción. FRB/cbindgen siguen alternativas.
- Desktop: libmpv **PROVISIONAL FOR LINUX**, evidencia previa preservada.
- Android: **Media3 provisional**, H264 en emulador API 35/x86_64 y teléfono
  Android 9/API 28 armv7 con audio. Calidad/perfiles/codecs ampliados pendientes; no libmpv móvil instalado/probado.
- iOS: **AVPlayer candidate**, solo investigación. **BLOCKED BY REQUIRED TOOLCHAIN**
  para build/runtime: no macOS/Xcode/SDK Apple/signing/dispositivo aquí.

## Build/runtime propios

| Plataforma | Research | Build | Runtime | Resultado |
| --- | --- | --- | --- | --- |
| Linux | Sí | Rust + Flutter | Sí, UI real y regresión mpv | Bridge probado; vídeo Flutter embebido no integrado |
| Windows | Sí | No ejecutado | No ejecutado | Sin soporte Cine Virtual demostrado |
| macOS | Sí | No ejecutado | No ejecutado | Sin soporte Cine Virtual demostrado |
| Android | Sí | Rust x86_64/arm64/armv7; APK x86_64 y armv7 | Emulador Android 15 y SM-J701M Android 9 | SAF/Player/bridge/lifecycle probados; no soporte universal |
| iOS | Sí | No ejecutado | No ejecutado | BLOCKED BY REQUIRED TOOLCHAIN |

Toolchain: Flutter 3.47.6/Dart 3.13.5, Temurin 21, Android CLI SDK,
compile/target 36, NDK 28.2.13676358, Media3 1.11.1. KVM, AVD Google APIs 35,
2 vCPU/1536 MiB, SwiftShader, sin host audio. ARM **biblioteca compilada**,
no APK ni runtime ARM. El usuario conectó después un SM-J701M/API 28 de 32 bits; se detuvo el AVD y
se compiló/ejecutó APK armeabi-v7a. Evidencia física abajo, separada del emulador.
No instalación Android Studio/VLC/GStreamer/mpv móvil. Dependencias Cargo ya
presentes en lockfile; nuevo crate bridge y pub ffi 2.2.0.

## Corpus y LocalMedia

`normal.mp4` generado por script existente: H264/AAC, 320x180, 30 fps,
30 s, 3308532 bytes. No películas privadas ni descargas.
SAF ACTION_OPEN_DOCUMENT real → permiso → ContentResolver → FD regular duplicado
por Rust → Read/SHA-256 streaming, 1 MiB, check size/mtime/ctime antes/después.
Hash 82.556 ms, 3308532 bytes; oracle independiente
Java MessageDigest: **identity_match=true**. Duración reportada 30000 ms.
Rust reutiliza hash_reader; nativo cierra FD original y worker cierra duplicado.

Buffer acotado no equivale a memoria total de app. PSS antes de hash
219695 KiB; después **hash + load**
232211 KiB, 45 threads;
tras playback 248308 KiB,
45 threads. No atribuir el delta solo al hash;
no benchmark aislado de memoria. Pipes/cloud FD no regulares/seekables rechazados;
proveedor remoto, permiso revocado y cambios durante hash en Android real pendientes.
Cancelación/FD ownership/cambio de archivo cubiertos por tests Rust; no escenario
Android independiente cancelando un hash en curso.

## Player y surface — emulador

Load→READY SDK: 2700 ms. ExoPlayer/main Looper + SurfaceView
PlatformView; vídeo sintético visible inspeccionado y callback first-frame=true.
Decoder `c2.goldfish.h264.decoder`: emulación; no aceleración de dispositivo físico
probada. Play avanzó 1001 ms en ventana aproximada de 1 s;
pause avance entre muestras estabilizadas: 0 ms.
No se midió latencia perceptual de audio/frame ni calidad de time stretching.

| Seek target ms | Dispatch→READY observado ms | Posición SDK ms | Landing SDK ms |
| --- | --- | --- | --- |
| 5000 | 190.577 | 5000 | 0 |
| 10000 | 220.303 | 10000 | 0 |
| 20000 | 153.219 | 20000 | 0 |

Esta métrica incluye plataforma/polling. READY y posición SDK no certifican frame
presentado exactamente en target ni completion del decoder. onEvents y first-frame
se observan por separado; no sleep fijo usado como prueba de completion.

| Rate solicitado | API reportada | Δposición/Δmonotónico nativo | Ventana ms |
| --- | --- | --- | --- |
| 0.98 | 0.980000 | 0.907738 | 2016 |
| 1.00 | 1.000000 | 0.963990 | 2055 |
| 1.02 | 1.020000 | 1.012395 | 2017 |

Ensayo final **vídeo sin audio**, sin acelerar relojes. Posición y timestamp se
capturan juntos en nativo para evitar sesgo de RPC. Pitch configurado a 1.0 no
prueba calidad sonora. El intento A/V anterior dio ratios 0.598/0.876/0.786:
**falló**, se preserva; también usaba timer Dart y entorno emulado.
Se cambiaron simultáneamente fuente temporal y audio: no aislar causalidad ni
presentarlo como bug de Media3 corregido. Gate A/V requiere dispositivo físico.

## SyncEngine y lifecycle — emulador

Mismo cine-core/SyncEngine/config/protocolo, sin cambios. Drift relativo inyectado
por comando de ensayo, cálculo/decisión Rust; 120 ms→rate 0.98, 0 ms→restore 1,
1000 ms→seek. Se aplican y observan en Media3, no solo ACK del bridge. No prueba
convergencia de timeline de sala móvil ni sincronización entre dos Android.

Home→foreground real: 2 invalidaciones; generation
5, clock_trusted=false, snapshot_required=true.
Resume **no auto-play** ni confianza automática. Android onStop pausa/restaura
rate; Rust cancela deadline/hash y resetea muestras/corrección; respuestas nativas
viejas se descartan por generación. Recuperación requiere clock resync + snapshot
v1 en integración futura. Calibración inicial y snapshot son **fixtures offline**,
no TIME_PING/TIME_PONG móvil ni RoomState de servidor.

Activity exit observó release SDK: true.
La demo force-stops/reaps y retira archivo experimental. Destrucción bridge con
join usa isolate auxiliar; no esperar hashing en UI thread. Registry destroy
retira bajo mutex y join fuera. SDK no cruza ABI ni fuerza Send/Sync.
No validar con esto recreación/proceso muerto, rotation, screen lock, background
playback ni callbacks de todos los proveedores. Teardown completo Application al
cerrar un engine/recrear Activity debe integrarse y probarse antes de producto.

## Bridge y frontera

DTO api_version=1/generation, validación serde/rangos, error codes, muestra,
capabilities playback_rate/content_uri_input y effects. Handles process-local,
create/destroy/call/hash_fd, buffers caller-owned, 64 KiB y 16 instancias.
No structs Rust como ABI. Application Rust posee Sync/Clock/lifecycle; Kotlin
Player/URI/surface; Dart presenta y transporta intents/efectos. No room logic Dart.

1000 State roundtrips: Linux p50 0.128 ms,
p95 0.246, max 0.980;
Android emulado p50 0.072, p95 0.678,
max 49.869. Debug Flutter, Rust release en Android;
no comparación de producción/benchmarks extremos.

| Runtime | Snapshots Hz | Muestras | Roundtrip p95 ms |
| --- | --- | --- | --- |
| Linux | 2 | 4 | 0.610 |
| Linux | 10 | 20 | 0.688 |
| Android | 2 | 4 | 5.590 |
| Android | 10 | 20 | 5.967 |

Snapshots Rust→Dart vuelven en la llamada; no stream push ni medición
unidireccional independiente de callback. 4/20 muestras, 2 s por frecuencia:
ensayo funcional limitado. 60 Hz no ensayado; UI conserva 2 Hz.

## Fallos y correcciones

- Kotlin Debug.getPss retorna Long: corregido tipo de campos.
- Heap Gradle template 8 GiB excesivo para host 7.6 GiB: limitado a 2 GiB/
  2 workers; build antes de emulador, no atribución de OOM no demostrado.
- SAF Recent vacío: harness abre roots/Downloads, sin inyectar URI.
- Flutter/logcat segmentó JSON: fragmentos numerados/base64, reensamble y chequeo
  de privacidad del contenido, no omisión de métricas fallidas.
- Rate A/V fallido conservado; final vídeo-only con muestreo temporal nativo.
- Espera fija de 3 s no aseguró resume: espera condición real con timeout 15 s.
  Intento intermedio preservado en diagnostics; no falso pass de lifecycle.
- Respuesta sample pendiente tras cambio generation: descartada; test Dart.
- Callback oracle después de onDestroy: guard alive/cancel cooperativo.
- Buffer FFI se libera también si falla abrir biblioteca.

## Regresión y verificaciones

82 tests workspace aprobados; 6 SDK tests opt-in ejecutados aparte y aprobados
(4 adapter + 2 client owner). 4 Flutter tests, analyze sin issues, fmt/clippy
-D warnings/build. Rust Android x86_64/arm64/armv7 y APK debug x86_64/armv7 compilados.
Docs/local links/JSON y git diff --check verificados en cierre.
Demo FakePlayer original aprobada. Slice Linux corto real 25.305 s: p95 drift
A98/B110 ms, p95 par66 ms, deadline lateness>50ms cero; resume B865.896 ms +
70.815 ms convergencia tras respuesta. Host con carga de build concurrente;
no sustituye los JSON previos de 600 s ni se repitieron 10 min completos.

## Riesgos y siguiente paso

Flutter **provisional**, Media3 **provisional Android**, AVPlayer solo candidato;
licencia pendiente sin LICENSE/release. ABI manual unsafe confía en caller;
no recovery de panic/poison, no codegen ni push streams. SDK/URI/network Application
requieren ownership del engine completo, cancellation y adversarial lifecycle.
GPL/LGPL/linking/tiendas: TECHNICAL RISK, LEGAL REVIEW REQUIRED; build mpv/FFmpeg
GPL Linux no se copió a móvil. Plugins/codecs/patentes se revisan por build concreto.

Recomendación: ampliar físico (A/V/formatos/SAF/lock/recreation) e integrar la
Application de red Rust v1 al bridge con suspensión/recovery. En paralelo disponer
de Mac/Xcode para prototipo AVPlayer/security-scoped recursos. No rehacer sync en
Dart ni promover motor universal/UI final antes de esos gates.

## Validación física adicional — Android 9

[results-android-device.json](results-android-device.json): **PASS**, Samsung
SM-J701M genérico, Android 9/API 28, armeabi-v7a (32 bits), sin serial.
Flutter + C ABI + mismo Rust Core + Media3, **audio habilitado**. No emulador
activo. No contenido privado: solo fixture sintético, seleccionando vía SAF real.
Aplicación de prueba queda instalada y detenida; archivo de prueba retirado.
Para uso manual recompilar sin SPIKE_AUTORUN; no es una release de producto.

Load SDK READY: 1232 ms; duración 30000 ms. SHA-256
72.069 ms para 3308532 bytes, oracle Java match=true.
Play avance 907 ms/ventana aproximada 1 s; pausa avance
0 ms. Frame real renderizado, decoder
`OMX.Exynos.avc.dec`. Es decoder vendor Exynos; no se consultó flag hardware
(API 28) ni se probó exhaustivamente aceleración/formats/audio perceptual.

| Target ms | Dispatch→READY SDK ms | Posición SDK ms | Error SDK ms |
| --- | --- | --- | --- |
| 5000 | 189.692 | 5000 | 0 |
| 10000 | 171.439 | 10000 | 0 |
| 20000 | 181.701 | 20000 | 0 |

No certifica frame exacto. La tolerancia Linux de drift no es un límite de
latencia seek móvil. Dispatch y SDK READY siguen diferenciados.

| Solicitado | Reportado | Ratio medido | Ventana nativa ms |
| --- | --- | --- | --- |
| 0.98 | 0.980000 | 0.973032 | 5006 |
| 1.00 | 1.000000 | 0.994010 | 5008 |
| 1.02 | 1.020000 | 1.014580 | 5007 |

Tras detectar sesgo en ventanas de arranque de 2 s, se preservó el intento físico
inicial en diagnostics y se ensayaron ventanas de **5 s tras 500 ms de warmup**
después de observar playing/avance. Warmup no se usa para certificar seek.
Posición/timestamp juntos en native; sin reloj acelerado. Esta prueba describe
rate estacionario corto, no borra startup ni prueba audio perceptual o estabilidad
prolongada. Audio habilitado no significa pitch/calidad auditiva aprobados.

Core 120 ms→rate 0.98, 0 ms→restore 1 y 1000 ms→hard seek: aplicados/observados en
SDK. Hard seek target 13796 ms y posición observada
13802 ms, jugando (avance durante RPC posible).
Ningún threshold, SyncEngine o protocolo cambiado.

Home desde **playing**→resume: 2 invalidaciones,
generation 5, clock_trusted=false y snapshot_required=true;
SDK **playing=false**, rate **1.0** después de volver. Sin reanudar automáticamente.
Clock/snapshot anteriores son fixtures offline; resync/RoomState de servidor móvil
no integrados. Surface mantiene pantalla encendida mientras visible para evitar
que el timeout de pantalla altere el ensayo; no cambia settings globales.
No rotación/lock/recreación/proceso muerto probados en este cierre.

PSS antes hash 184231 KiB; hash+load
183806 KiB; después reproducción
167254 KiB.
Threads 48→47.
PSS/GC varían entre lanzamientos; no atribuir delta al hash ni afirmar cero leak.
Release Player al salir observado=true; force-stop propio y archivo retirado.
No se borra logcat global ni se sobrescribe un fixture preexistente en teléfono.

1000 C ABI State roundtrips: p50 0.154 ms,
p95 0.300, max 3.843.

| Snapshots Hz | Muestras | Roundtrip p95 ms |
| --- | --- | --- |
| 2 | 4 | 5.859 |
| 10 | 20 | 5.011 |

Observaciones RPC/polling, no latencia unidireccional de evento push. Es un ensayo
corto debug, no benchmark generalizado. URI inválida y guard de acceso destroyed
rechazados; release real observado aparte. Error tests no equivalen a probar todas
las fallas del SDK. Android físico no valida Android de todas las versiones/ABIs.

Riesgos adicionales: open/query SAF son IPC síncronos en el adapter nativo para
este proveedor local; producción debe aislar proveedores lentos/IO del main.
Hash y Player abren recursos por separado; proveedores/versiones mutables
requieren verificación hash→load/Ready. Teardown/recreación completa del engine
Application, revocación y providers remotos pendientes. iOS sigue bloqueado por
Mac/Xcode. Próximo paso: unir la Application de red Rust existente al bridge y
probar Android+Linux, clock real/snapshot y suspensión, antes de UI final.
