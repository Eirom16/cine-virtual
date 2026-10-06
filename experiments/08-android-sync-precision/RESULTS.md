# INFORME — ESTABILIZACIÓN DE SINCRONIZACIÓN ANDROID

## 1. Estado inicial

MEASURED: master, beeec76, origin/master, árbol limpio. Se comprobaron status/branch/log/remote y gh auth/run list; último CI inicial 37395948312 PASS. Acceso GitHub/ADB requirió salir del sandbox de red; no se cambió CI ni el firewall.

## 2. Datos históricos reproducidos

MEASURED histórico, leído desde JSON: Linux p50/p95/p99 3/36/47 ms; Android 33/514/7230 ms; cross-device 29/462/541 ms. Android 75 hard seeks, 7 rate y 4 restore; dispatch p95 432 ms. Se leyeron smoke, background, fallos de Ready/teardown, SDK/bridge y datos del experimento 02. Originales intactos. [Resumen](results-before.json).

## 3. Pipeline temporal analizado

IMPLEMENTED/TESTED: rooms/src/service.rs::RoomService::execute/apply → server/src/lib.rs::connection/send → client/src/runtime.rs::Client::start/ws.next → replica.rs::install/deadline_local_ms/execute_due/correct_drift → SyncEngine::observe → network.rs::MobilePlayer::action/drain → android.rs::nativeDrive → MainActivity.kt::drive.run/applyEffect → Media3 callbacks/state → nativeDrive/MobilePlayer::sample → Replica. Scheduler y protocolo siguen compartidos con Linux.

## 4. Instrumentación añadida

IMPLEMENTED: recepción autoritativa, deadline calculado/wake, enqueue/drain/JNI, post/due/execution del driver, llamada/retorno SDK, callbacks, READY, primera posición estable y entrega de observación. Correlación operation_id/sequence/generation/media_revision/action/reason; sink acotado y dropped explícito. [Especificación temporal](README.md). No tokens, dirección privada, URI, serial o SHA completo en evidencia.

## 5. Scheduler

MEASURED: p95 antes debug 2,5 ms/profile 2 ms; después debug 3,5 ms. No explica los cientos de ms. Ensayo final V2 p95 2 ms. Deadline, autoridad y clock conservados; instrumentación no cambia scheduling.

## 6. Bridge

MEASURED: cola effects p95 debug antes 32 ms/después 127 ms; profile antes 45 ms/después primera corrida 22 ms. Se preservan máximos y todas las corridas. Ensayo final V2 cola p95 121 ms/máximo 150 ms. No se reconstruyó el bridge ni se introdujeron streams.

## 7. JNI

MEASURED: roundtrip p95 ~1 ms, parse/registry <1 ms p95. Rust Instant y Kotlin elapsedRealtime tienen origen y suspensión distintos; lectura nativeClock bracketed en cada poll, midpoint e incertidumbre de bracket/2 + 1 ms. No se restan clocks absolutos.

## 8. Main Looper

MEASURED: poll_lateness p95 debug antes 134/después 130 ms; profile antes 9 ms. Máximo debug corto 515 ms; ensayo final V2 p95 51 ms/máximo 478 ms. Legacy dispatch p95 final 273 ms (histórico 432; primer largo V1 419,5), aún fuera de 50 ms. No se considera deadline plenamente resuelto. No hay runnable por comando: commands se despachan dentro del poll ya posteado; post→due contiene los 20 ms deliberados, execution−due mide retraso. ContentResolver IPC existe durante selección/load, no cada poll; hash es worker. No se atribuye todo el bloqueo a una función sin perfil de stack.

## 9. Media3

MEASURED adicional: primera llamada SDK del tipo del control, reloj calibrado, p95 152,5 ms frente a legacy 273 ms (n=6). Legacy selecciona último effect por sequence y puede incluir el Play posterior al seek. El miss inicial de Play sigue abierto; no se atribuyen esos 273 ms íntegros al Looper.

API CONTRACT: [eventos oficiales](https://developer.android.com/media/media3/exoplayer/listening-to-player-events) distinguen discontinuidad directa de seek, posibles cambios de estado, READY e isPlaying; no prometen tiempo de completion. MEASURED: seek→READY p95 antes debug 411/profile 467 ms; llamadas SDK p95 15/16 ms. Ensayo final V2 READY p95 470 ms, loss 406 ms, estable 517 ms; llamada SDK 15 ms. Mismo Media3 1.11.1 y SeekParameters.EXACT; no se cambió de motor.

## 10. Position observations

MEASURED: muestras que dispararon seeks eran frescas; sesgo source→timestamp asignado JNI p95 ~1 ms, máximo 4 ms en comparaciones cortas. No es causa principal. Source position/time se capturan juntos y se documenta transformación. Persiste la asignación legacy tras JNI en la vista Application: deuda pequeña medida, no maquillada como solución.

## 11. Seek lifecycle

IMPLEMENTED: IDLE → REQUESTED → DISPATCHED → IN_PROGRESS (discontinuity) → SETTLED (onEvents/READY) → IDLE (dos observaciones compatibles; playing exige avance positivo). Completion es heurística de posición/estado SDK, no frame ni audio perceptual. V2 continúa midiendo pérdida de avance aunque el snapshot ya se haya estabilizado pausado; dos operaciones snapshot de la recuperación corta emitieron first_advance, donde V1 omitía la medida. No sleep fijo. SEEKING ya bloqueaba decisiones; no se añadió una exclusión amplia para esconder lag real.

## 12. Causa de los hard seeks

MEASURED + INFERRED causal: target quedaba anclado a una muestra anterior; Media3 interrumpía avance durante seek y timeline servidor continuaba. Al completar volvía a quedar retrasado, provocando siguiente corrección. Muestras frescas y READY corroboran lag persistente, no solo pending. A/B profile con mismos controles: 23 hard seeks antes, 1/0/1 después.

## 13. Clasificación de hard seeks

INFERRED sobre MEASURED: antes debug 23 PLAYER_COMPLETION_DELAY y 15 OTHER; profile 8 y 15. No se relabelan todos como legítimos. Completions play/pause duplicadas o invalidadas por comando opuesto se rechazan sin borrar el evento. Cada decisión conserva drift, streak cuando instrumentado, estado/pending/buffering, edad, último completion/tiempo transcurrido y command latency. Ensayo final V2: PLAYER_COMPLETION_DELAY=1, SNAPSHOT_RECOVERY=1; LEGITIMATE/TRANSIENT_AFTER_SEEK/STALE_SAMPLE/SCHEDULER_DELAY/OTHER=0. Son clasificaciones heurísticas, no prueba independiente de cada causa. Categorías sin evidencia permanecen cero. [Analyzer](../../scripts/analyze_android_timing.py) y timing-*.json conservan criterios y OTHER.

## 14. Debug vs profile/release

MEASURED: debug y profile ejecutados en el mismo Android 9 ARMv7. Rust Android release en ambos, como antes. Debug Looper/CPU peores; profile tampoco resolvía seek storm antes del fix. p95 cortos después: debug 171 ms; profile 153/178/195 ms, todos FAIL. UNTESTED: release Flutter físico, no necesario para demostrar la diferencia principal ni usado para ocultar bugs.

## 15. CPU

MEASURED: histórico debug 90,84% de un core; después debug corto 90,63%. Profile antes 63,97%; después 76,44/70,32% en primeras corridas. CPU de proceso y grupos de threads conservada. En el primer largo debug: main ≥36,32%, Media3 ≥19,50%, raster ≥9,28%, codec/audio ≥9,71%, Rust owner ≥3,78% de un core; límites inferiores por threads terminados, no atribución exhaustiva. CPU proceso final V2 78,55%, sin afirmar mejora CPU causal por el fix; driver wall time final 4,51%, JNI 2,54%, source 1,17%. Driver wall time corto ~2,8–4,3%, JNI ~1,7–2,6%, source ~0,4–0,6%; wall time no equivale a CPU. INFERRED: presión debug contribuye a Looper; no se prueba atribución exclusiva Flutter/Media3/logging. Hash termina antes de playback sostenido.

## 16. Polling

IMPLEMENTED: conservado 20 ms SDK, 500 ms UI, 50 ms publicación Application. MEASURED: su coste no explica por sí solo completion 400 ms; mismo polling con seeks drásticamente reducidos. Delay de cola se incorpora a proyección al dispatch. No infraestructura de streams ni optimización general de CPU.

## 17. Hipótesis descartadas

MEASURED limitado a ensayos: scheduler y JNI no son causa principal; muestras stale y hard seek mientras otro estaba pending no explican el storm capturado; profile por sí solo no lo arregla. Snapshots periódicos no muestran seeks redundantes sistemáticos. UNTESTED: no se descartan todos los casos en otros formatos/SDK/hardware, ni un callback READY prematuro que no ocurrió aquí.

## 18. Causa raíz

INFERRED con intervención A/B: posición objetivo no representaba el instante en que Media3 podía continuar avanzando tras seek. Latencia de cola y tiempo sin avance generaban error nuevo en cada corrección. Debug añade misses de Looper y quedan transitorios de arranque; teléfono antiguo no se usa como explicación automática.

## 19. Corrección implementada

IMPLEMENTED/TESTED: contexto target_at_ms/playing; MobilePlayer proyecta target hasta drain y suma pérdida de avance medida, mediana de últimas tres operaciones. Kotlin añade delta breve hasta dispatch. Paused/prepare conservan target exacto y duration limita posición. Playing mide elapsed menos avance real hasta primera muestra que avanza; paused READY. Modelo se borra al invalidar generation; callbacks ajenos se descartan. Sin tiempo constante ni thresholds Android enormes.

## 20. Cambios en Core

IMPLEMENTED: solo getter diagnostic_streak, sin mutación. Algoritmo, deadband 50 ms, hard seek 250 ms, tres muestras, cooldown 2 s y rate ±2% intactos. Replica añade contexto y telemetría. Ningún cambio a protocolo v1, autoridad, clock, scheduler o política libmpv.

## 21. Cambios Android

IMPLEMENTED: correlación/clock bracket, costes JNI/driver/source, Looper post/execution, callbacks/lifecycle/stability, reporte de pérdida de avance y proyección breve al SDK. Modelo Rust local de MobilePlayer; Kotlin no interpreta RoomState ni decide drift. CPU por grupos sin nombres privados. Fault injection solo con diagnóstico explícito.

## 22. Tests añadidos

TESTED: target playing con edad/cola y seek lento no deja lag fijo; paused exacto/duration; mediana sin delay inventado; completion de otra operación/generation rechazado; pending/stale no corrigen y fresh vuelve a corregir; cola/sink acotados; análisis de relojes distintos y OTHER. Tests con tiempo numérico, sin sleeps largos nuevos. Siete tests Rust nuevos y cuatro Python; suite Rust 97 PASS, seis SDK opt-in PASS serializados. Un test comprueba que una observación pausada estable no invalida la medida posterior de seek al reanudar Play.

IMPLEMENTED/TESTED: el analyzer distingue dispatch inicial del control de effects al completar seek; un test determinista con clocks diferentes evita confundir el Play posterior con el seek programado. Suite Python final: diez tests.

## 23. Resultado diagnóstico corto antes

MEASURED: profile controles 90,06 s: Android p50/p95/p99 226/573/627 ms, 23 hard seeks, 3 rate/2 restore, cross p95 548 ms. Debug 112,56 s incluía además recovery: p95 10594 ms, 38 hard seeks; no comparar ese p95 directamente con controls-only posterior.

## 24. Resultado diagnóstico corto después

MEASURED: profile 90 s ×3: Android p95 153/178/195 ms; hard seeks 1/0/1; cross p95 155/183/209 ms. Debug 90 s: p95 171 ms, un hard seek, cross p95 181 ms. Todos FAIL primarios. Últimos 30 s p95 14/41/34/13 ms, subset descriptivo de convergencia, no sustituto del gate. Los resultados fallidos permanecen. Fault test profile separado con baseline estable confirmó rate 0,98, restore 1,0 y hard seek con biases +700/+1000; el primer intento +120 no confirmó soft rate y se conserva.

## 25. Prueba de 600 segundos

MEASURED: 600.16 s reales, debug, Android físico API 28/ARMv7, audio habilitado, LAN sin túnel. Hash/Ready, Play/Pause/Seek, background/foreground y disconnect/resume, teardown observado=True. Resultado funcional PASS; gate separado. Primer ensayo V1 separado: Android p95 72 ms, cross 91 ms, tres hard seeks; su raw se conserva. V2 repite tras corregir la medición de snapshot pausado.

## 26. Métricas Linux

MEASURED: p50 17 ms; p95 **50 ms**; p99 51 ms; máximo 5000 ms; n=1306.

## 27. Métricas Android

MEASURED: p50 23 ms; p95 **120 ms**; p99 6268 ms; máximo 11761 ms; n=1193 en agregado, 1194 en raw por carrera de publicación. [Auditoría](results-aggregate-audit.json) reproduce todos los percentiles/máximo; valores originales preservados.

## 28. Métricas cross-device

MEASURED: p50 18.0 ms; p95 **57.0 ms**; p99 186.0 ms; máximo 466.0 ms; n=1165. Posición SDK proyectada al reloj servidor; no frames ópticos simultáneos.

## 29. Hard seeks antes/después

MEASURED: histórico 75/600 s; después 2/600 s. Todos los seeks SDK y decisiones Core quedan en raw; preparación/reconciliation no se ocultan.

## 30. Background/foreground

MEASURED: {"foreground_ms": 1908.1794929988973, "foreground_post_ready_convergence_ms": 0.025296001695096493, "foreground_initial_drift_ms": -33}. El wait post-Ready puede retornar inmediatamente porque ya había tres muestras válidas: no es tiempo total de recuperación.

## 31. Disconnect/resume

MEASURED: {"network_resume_ms": 13195.230364999588, "network_post_ready_convergence_ms": 1559.1320709972933, "same_member_after_resume": true}. Resume incluye reconexión y nueva Ready; post-Ready convergence es un intervalo separado, con tres muestras <=150 ms.

## 32. Regresiones

TESTED: FakePlayer demo, Linux real dos libmpv, Flutter Linux smoke y seis SDK tests opt-in. Android local smoke se documenta con su propio JSON. Un test de FDs falló al ejecutarlo en paralelo con otro Player; serializado pasó 4/4, sin cambiar test ni SDK. Rust fmt/clippy/tests/build, docs/check_ci, diez Python y Flutter analyze/seis tests pasan; resultados-verification conserva alcance. Sandbox bloqueó sockets y un rerun Flutter omitió CINE_BRIDGE_LIBRARY: repetidos con entorno documentado pasaron; intentos preservados en results-verification. Android local smoke PASS (SAF/hash/frame/Play/Pause/Seek/rate/lifecycle/destroy; landing error 0 ms en tres seeks, bridge p95 0,236 ms). No se cambia CI.

## 33. GitHub Actions

TESTED: matriz completa del código/análisis fde7c7b PASS, 11 jobs verdes, [run 37417007455](https://github.com/Eirom16/cine-virtual/actions/runs/37417007455). Base, Linux con SDK headless, Windows, macOS ARM64/x64, Android x86_64/arm64-v8a/armeabi-v7a e iOS device/simulator. [Evidencia machine-readable](results-ci.json). Runtime físico es evidencia separada; iOS Player NOT IMPLEMENTED. Los dos runs intermedios se cancelaron por pushes posteriores, no por fallo de tests. El registro final es un commit documental sin cambio de implementación y también se observa su CI.

## 34. Bugs encontrados

MEASURED/IMPLEMENTED: target atrasado tras seek; correlación global de callbacks insuficiente; Kotlin rustNow fuera de alcance durante instrumentación, corregido; telemetría CPU inicial demasiado grande para logcat, compactada; controls-only marcaba background executed, corregido con valor original conservado; primer bias120 canceló lag previo, repeat con baseline estable. Snapshot podía estabilizar posición pausada antes de Play y finalizar prematuramente la medición de pérdida de avance: el primer ensayo largo lo mostró; se separó esa medición del marcador estable y se repitió recovery y ensayo largo. No se borran intentos fallidos.

La recomputación independiente encontró una carrera del harness entre cálculo y copia de muestras: after-debug y ambos largos guardaron una muestra Android terminal adicional. Todos los percentiles/máximos y agregados de pair_samples se reproducen. IMPLEMENTED: congelar sample_snapshot para cálculo y evidencia; originales intactos y auditoría machine-readable.

## 35. Riesgos restantes

INFERRED/UNTESTED: variabilidad del predictor, cold Play/AudioTrack, callback READY genérico y position masking, frame/audio real, corpus de mayor carga, otros dispositivos, rotación/process death, providers lentos y WAN. Los FAIL cortos siguen abiertos aunque el ensayo largo llegue a aprobar. En el largo final Android p99 6268 ms y máximo 11761 ms; aún hay recuperación muy fuera de meta aunque el p95 pase. No nueva UI ni features.

## 36. Deuda técnica

IMPLEMENTED experimental: ABI manual/polling conservados, modelo de seek median-of-three con muestras paused/playing de latencias distintas, timestamp legacy JNI con sesgo pequeño medido, perfiles de stack finos y ownership/recreación completos pendientes. Se añade evidencia al ADR-006 sin cambiar su decisión. No nuevo ADR: arregla implementación de capacidad ya permitida por ADR-006/007.

## 37. Evaluación p95 <=150 ms

MEASURED: ensayo largo Android PASS, cross PASS. Cortos: todos FAIL. No aprobación universal ni modificación de filtros/thresholds para forzar meta.

## 38. Estado Media3

IMPLEMENTED/TESTED experimental; **PROVISIONAL FOR ANDROID**. No se abandona ni se promueve a motor definitivo. libmpv sigue provisional Linux; iOS fuera de alcance.

## 39. Commits

IMPLEMENTED: 81f5cff `diag: instrument android playback timing`; 155299a `fix: compensate android seek completion loss`; ddeb7e7 `docs: record android sync precision diagnosis`; 324d4d8 `fix: freeze diagnostic metric sample boundary`; fde7c7b `diag: distinguish initial scheduled control dispatch`. Registro final documental `docs: record hosted android precision validation`. Los últimos cambios al harness/analyzer no alteran Android probado; source del CI completo está identificado en results-ci.json.

## 40. Estado Git final

TESTED: master con upstream origin/master, implementación y evidencia publicadas, árbol limpio al verificar fde7c7b y tras los pushes previos. El registro documental final referencia ese punto comprobado; su identidad aparece en git log. No force push, ramas borradas, secrets, releases o cambios de visibilidad. El HEAD documental y su nuevo run se comprueban otra vez antes de cerrar la sesión.

## 41. Próximo paso recomendado

INFERRED: diagnosticar first actual advancement de Play/AudioTrack y mejorar incertidumbre del predictor con evidencia; repetir cortos hasta ≤150 ms consistentemente y ampliar corpus/dispositivo. Mantener UI de producto detenida. No rehacer Core/bridge/protocolo sin consulta.

## 42. Resumen para otro arquitecto

Core global y protocolo preservados; el problema reproducible es dispatch/seek semánticamente asíncrono contra timeline en avance. Instrumentación correlacionada y A/B justifican compensación local, con reducción fuerte del storm. Separar completion READY, posición estable y frame real; clocks calibrados, raw conservados, OTHER visible y gates cortos fallidos explícitos. Ver resultados antes/después y límites antes de adoptar stack/UI.
