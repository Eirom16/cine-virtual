# Diagnóstico de precisión Android

Estado: instrumentación experimental, antes de UI de producto. Core y protocolo
v1 conservan su comportamiento y sus thresholds. Evidencia física y conclusiones
se registran en RESULTS.md al completar el diagnóstico.

## Reproducción

```sh
python3 scripts/demo_cross_platform.py --seconds 90 --diagnostic --output-dir experiments/08-android-sync-precision --label before-debug
python3 scripts/demo_cross_platform.py --seconds 90 --diagnostic --build-mode profile --output-dir experiments/08-android-sync-precision --label before-profile
python3 scripts/analyze_android_timing.py experiments/08-android-sync-precision/results-before-debug.json --output experiments/08-android-sync-precision/timing-before-debug.json
```

Android físico autorizado, corpus sintético, SAF, LAN existente, sin túnel ni
cambio de firewall. --port permite reutilizar el puerto autorizado de la LAN.
--no-build exige APK del build-mode declarado; Rust Android se compila release
como en el experimento 07. --faults-only crea una corrida separada con bias
sintético +120/+700/+1000 ms en las observaciones durante 10 s cada uno; sus
métricas nunca cuentan como precisión normal.

## Pipeline y dominios temporales

RoomService programa timeline → servidor WebSocket → Client::start/ws.next
captura recepción Rust → Replica::install/prepare_pending/deadline_local_ms →
execute_due → apply_timeline/correct_drift → SyncEngine::observe →
MobilePlayer::action (cola de 32) → nativeDrive (JNI, registry) → drive.run
(main Looper) → applyEffect → Media3 callbacks → state() → nativeDrive →
MobilePlayer::sample → Replica::correct_drift.

Rust usa Instant relativo a Instance.started compartido con Client. Kotlin usa
SystemClock.elapsedRealtime/elapsedRealtimeNanos (incluye suspensión). No son
el mismo origen ni la misma semántica de suspensión. Cada drive calibra de nuevo:
K0 antes de nativeClock, R leído dentro de Rust, K1 al volver; R corresponde al
intervalo [K0,K1]. Mapeo diagnóstico R + (Ksource - midpoint(K0,K1)), incertidumbre
≤(K1-K0)/2 + 1 ms de cuantización. Nunca restar ticks absolutos Kotlin y Rust.
El registro conserva también el timestamp que el comportamiento original asigna
en Rust después de JNI; comparar ambos permite medir sesgo sin arreglarlo a ciegas.

No hay un runnable por comando: los effects se ejecutan dentro del runnable de
polling ya posteado. post, due=post+20, execution y recepción del effect se
registran separadamente. poll_lateness=execution-due; effect_queue se mide en
Rust desde enqueued hasta drain e incluye la espera por el siguiente poll.
post no es posterior a enqueue necesariamente; no sumar ambas latencias.

## Correlación y completion

NativeEffect añade operation_id local creciente, sequence, generation,
media_revision, action y reason. Callback de seek conserva su propia operación,
no la última operación rate/play; callbacks no causados por un comando pueden
carecer de operación. No se registran tokens, URIs, serial, SHA o direcciones.

La instrumentación inicial conserva la completion antigua: cualquier onEvents
con READY termina seeking. seek_ready_candidate conserva qué eventos lo causan,
position_discontinuity y playback_state_changed permiten comprobar la señal.
first_stable_position exige dos observaciones READY/no buffering/no seeking con
avance compatible con playing/rate (tolerancia local 35 ms); es diagnóstico de
posición SDK, no certificado de frame/audio presentado ni sleep de completion.

El sink Rust está acotado a 256 registros, comunica diagnostics_dropped;
Kotlin publica comandos/callbacks y una muestra de coste cada 25 polls. CPU de
proceso y por thread, coste acumulado source/JNI/driver y memoria se conservan
para distinguir presión de tooling de latencia propia del adapter. Instrumentar
introduce coste; debe compararse con sus medidas y build-mode, sin ocultarlo.

## Política de métricas

Se conservan raw muestras/eventos sanitizados y los mismos filtros del experimento
07. El análisis añade scheduler, cola, JNI, Looper, llamada Media3, completion y
observaciones. Clasificaciones de hard seek son INFERRED a partir de tiempos
MEASURED, con criterio explícito; OTHER nunca se elimina. PASS funcional y gate
p95 ≤150 ms son distintos. Resultados ausentes son UNTESTED.

## Contrato Media3 consultado

[Eventos oficiales](https://developer.android.com/media/media3/exoplayer/listening-to-player-events):
seekTo provoca discontinuity por seek y puede cambiar playback state inmediatamente;
no garantiza que siempre haya cambio de estado. onEvents agrupa callbacks, READY
significa capacidad de reproducir desde posición actual, isPlaying combina
READY/playWhenReady/no suppression. Un onEvents genérico no es un ACK de seek.
[Player.Listener](https://developer.android.com/reference/androidx/media3/common/Player.Listener)
y [fuente 1.11.1](https://github.com/androidx/media/blob/1.11.1/libraries/exoplayer/src/main/java/androidx/media3/exoplayer/ExoPlayerImpl.java)
distinguen discontinuidad inmediata y actualización interna. playbackParameters
requiere observar el valor aplicado; buffering y timeline changes se registran.
El contrato no promete latencia, frame exacto ni progreso durante buffering.
Las cifras del dispositivo son comportamiento MEASURED, separado del contrato.
