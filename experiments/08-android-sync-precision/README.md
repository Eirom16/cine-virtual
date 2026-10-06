# Diagnóstico de precisión Android

Estado: instrumentación experimental, antes de UI de producto. Core y protocolo
v1 conservan su comportamiento y sus thresholds. Evidencia física y conclusiones
se registran en [RESULTS](RESULTS.md) al completar el diagnóstico.

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
El registro conserva también el timestamp que el comportamiento actual asigna
en Rust después de JNI; comparar ambos permite medir sesgo sin arreglarlo a ciegas.

No hay un runnable por comando: los effects se ejecutan dentro del runnable de
polling ya posteado. post, due=post+20, execution y recepción del effect se
registran separadamente. poll_lateness=execution-due; effect_queue se mide en
Rust desde enqueued hasta drain e incluye la espera por el siguiente poll.
post no es posterior a enqueue necesariamente; no sumar ambas latencias.

| Punto pedido | Campo/evento | Dominio y significado |
| --- | --- | --- |
| T0 | authoritative_received.at_ms | Rust; recepción antes de instalar evento |
| T1 | deadline_local_ms | Rust; deadline servidor convertido con offset estimado |
| T2 | wake_at_ms | Rust; execute_due del scheduler |
| T3 | enqueued_ms | Rust; MobilePlayer crea effect |
| T4 | rust_received_ms / effect_received.sdk_ms | Rust/JNI entrega y Kotlin recibe; bracket documentado |
| T5 | drive_post_ms, drive_due_ms | Kotlin; runnable del poll, puede preceder T3 |
| T6 | drive_execution_ms | Kotlin; runnable comienza |
| T7 | media3_call.sdk_ms | Kotlin; antes de llamar SDK |
| T8 | callbacks correlacionadas | Kotlin; discontinuity/state/isPlaying/rate |
| T9 | seek_ready_candidate / playing_completed / rate_completed | Kotlin; semántica SDK, con límites de correlación |
| T10 | first_stable_position | Kotlin; seek estable en dos muestras; Play/Pause conservan callback y muestras raw |
| T11 | stable_observation_delivered.rust_delivered_ms | Rust; entrega de observación SDK por JNI |

La latencia de completion de Play/Pause es cambio de isPlaying, no comienzo/fin
perceptual de audio; un comando que ya estaba satisfecho puede no emitir callback.
No se inventa una completion ausente. El analyzer distingue callback, READY,
pérdida de avance y estabilidad, y preserva counts para saber qué se midió.

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

## Corrección local implementada

MobilePlayer::drain proyecta un seek playing desde target_at_ms hasta el drain y
suma la mediana de las últimas tres pérdidas de avance medidas. Kotlin añade
solo el breve tiempo desde el retorno JNI hasta dispatch; no interpreta estado
autoritativo ni cambia SyncEngine. Paused y preparación conservan posición exacta.
seek_loss_measured playing usa elapsed desde dispatch menos avance SDK hasta
primera posición que avanza; paused usa READY. Se correlaciona por operation_id y
generation, se descartan resultados antiguos y se borra modelo al reset.
No se aplica un tiempo constante ni un sleep de completion. La incertidumbre de
esta predicción sigue siendo una limitación medida, especialmente en arranque.

Lifecycle: REQUESTED al recibir effect; DISPATCHED antes de seekTo; IN_PROGRESS
en discontinuity de seek; SETTLED en onEvents/READY; IDLE tras dos muestras READY
con evolución compatible (playing exige avance positivo). SETTLED no garantiza
frame presentado. La señal READY actual no se cambió como nueva exclusión del
Core: en los ensayos anteriores las muestras que provocaron hard seek ya eran
frescas, READY y fuera de pending; excluirlas indefinidamente escondería lag real.

El análisis network/runtime convierte recepción Rust a servidor con el offset
calculado en el mismo control, obtenido de deadline_server - deadline_local.
Incluye colas runtime/transporte y incertidumbre NTP; no es latencia de cable
pura ni resta relojes absolutos. observation_return usa los dos timestamps en
dominio Rust tras calibración, con incertidumbre de bracket + cuantización.

Las primeras variantes profile posteriores se compilaron antes del getter de
streak y de la distinción callback/completion playing; no cambia su política.
Profile 3, debug y ensayo largo incluyen esa instrumentación final. Source base
es el commit anterior a la corrección, ejecutada con worktree modificado y
versionada al cerrar. Los archivos históricos del experimento 07 no se cambian.

--controls-only limita el diagnóstico corto a controles. El subset de últimos
30 s sirve exclusivamente para comprobar estabilidad/convergencia, nunca para
reemplazar el p95 primario; todos los FAIL cortos permanecen. --faults-only espera
ahora tres muestras <=50 ms antes de cada bias, después del seek playing, y espera
cada fault_end. El primer fault de +120 coincidió con lag previo y NO confirmó
soft rate; se conserva y se repite con baseline estable.

## Versiones y límites de la evidencia

before-debug/profile contienen instrumentación sin compensación. after-profile-1,
2, 3 y after-debug contienen la compensación local original (V1). El primer
after-600-debug mostró además un defecto de medición: un snapshot playing podía
quedar estable mientras el SDK aún estaba pausado y dejar de medir al reanudar.
after-recovery-v2 y after-600-debug-v2 mantienen la medición de pérdida hasta el
primer avance, independientemente del marcador de posición estable. Se conservan
ambas variantes y sus resultados; no se mezclan como una única implementación.

Los comandos medidos pertenecen a un worktree modificado sobre source_base_commit;
los commits de cierre versionan ese código y evidencia. --no-build reutiliza el
APK del modo indicado; las corridas V2 debug reutilizan su APK compilado después
de cambiar observeStable. El fault profile separado reutiliza V1 y no valida V2.

El analyzer calcula primera callback y completion play/pause/rate en el mismo
dominio Kotlin. Un isPlaying callback puede conservar una operación vieja cuando
un comando no produce transición: se rechazan completions duplicadas o posteriores
a un comando opuesto, con evento y razón preservados en rejected_completion_callbacks.
Los logs iniciales siguen siendo evidencia; esas latencias no son ACKs fiables.
Los grupos CPU usan contadores de threads vivos y son un límite inferior: los
threads que terminan desaparecen. No se atribuye todo main a Flutter ni el tiempo
de pared del driver a CPU, y no se comparan sus sumas como un perfil exhaustivo.

La cifra legacy dispatch mantiene la fórmula del experimento 07 para comparación;
su ancla antes de JNI puede añadir el pequeño coste de JNI dos veces. El análisis
de scheduler/queue/SDK usa timestamps separados y calibración; no se presenta la
cifra legacy como latencia de seek ni como medida exacta de frame/audio.
