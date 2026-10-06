# INFORME — COMPARACIÓN ANDROID ANTIGUO VS MODERNO

MEASURED: cinco corridas Profile de controles, 90 s cada una, más una corrida
Profile separada de recuperación. Android y cross-device cumplen ≤150 ms en
**3/5** cortos. INFERRED: **B. DEVICE CONTRIBUTES**, con mejora clara de latencias
del adapter y evidencia moderada de mejora de drift. No hay cumplimiento
consistente ni prueba de que el hardware sea la única causa.

## Dispositivo antiguo

MEASURED histórico: Samsung SM-J701M, Android 9/API 28, armeabi-v7a, 32 bits.
Modelo/ABI proceden del [smoke físico versionado](results-android-local-regression.json).
La comparación principal utiliza [Profile 1](results-after-profile-1.json),
[Profile 2](results-after-profile-2.json) y [Profile 3](results-after-profile-3.json).
No se repitió el antiguo: solamente el nuevo estaba conectado.

El [largo final V2](results-after-600-debug-v2.json) fue **Debug**, 600,16 s:
Android p95 120 ms, cross-device p95 57 ms, dos hard seeks. Es contexto histórico,
no la población comparable con estos cortos Profile.

## Dispositivo nuevo

MEASURED: Samsung SM-A146U, SoC MediaTek MT6833 según propiedades del dispositivo.
Un único teléfono físico autorizado conectado; todas las operaciones ADB y las
pruebas se dirigieron a él. Wi-Fi validada y conexión al servidor Linux por LAN,
sin túnel ADB. No se guardan serial, cuenta, IP, SSID, URI ni identificadores
personales. [Entorno y condiciones](modern-device-environment.json).

## ABI y Android

MEASURED/TESTED: Android 15/API 35, ABI primaria arm64-v8a, proceso de 64 bits.
APK: libflutter.so y libapp.so exclusivamente ARM64; bridge ELF64/AArch64,
machine 183, coincidente con la salida actual de Gradle. Package instalado:
primaryCpuAbi=arm64-v8a, secondaryCpuAbi=null. El APK contiene además bridges
inactivos de otras ABI del spike; la ejecución comprobada es ARM64.

## Build utilizada

TESTED: código actual **cc7544b**, Flutter **Profile**, Rust Android **release**
para aarch64-linux-android; Flutter 3.47.6/Dart 3.13.5, Media3 1.11.1.
Se recompilaron el bridge ARM64 y el APK con ROOM_MODE/ROOM_AUTORUN. Las seis
corridas reutilizaron ese mismo APK mediante --no-build. El primer intento falló
por una configuración generada que apuntaba a un SDK Flutter temporal eliminado;
flutter pub get regeneró la caché, sin cambiar dependencias versionadas ni código.

MEASURED: mismo host Linux, cine-server/libmpv, fixture sintético de
24.300.725 bytes, transporte LAN/Wi-Fi. Hash coincidente, Ready, rechazo de Play
no autorizado, Play/Pause/seek pausado/seek playing, telemetría y teardown
confirmados en todas las corridas. Flujo estándar: Play 5000; a los 10 s
Pause/Seek 10000/Play; a los 22,5 s seek playing +3000; Pause al terminar 90 s.
Sin faults artificiales. Instrumentación y filtros heredados sin cambios.

IMPLEMENTED: solamente análisis offline y evidencia nuevos. **Cero cambios a
producción, SyncEngine, Media3, thresholds, polling o arquitectura.** Los
source_worktree_dirty=true de los JSON modernos reflejan los archivos de
evidencia/análisis nuevos, no una variante de la implementación Android.

Limitaciones A/B: Profile históricos usaron compensación V1; cc7544b usa V2,
que conserva la medición de pérdida cuando un seek se estabiliza pausado antes
de reanudar. No tenemos identidad binaria de esos APK históricos. Device, OS,
ABI y estado temporal varían juntos; no es un ensayo simultáneo aleatorizado.
La LAN existente se reutiliza, pero la identidad/interferencia Wi-Fi histórica
no se puede verificar independientemente. Por ello no atribuimos el beneficio
exclusivamente al SoC. [Datos y límites machine-readable](modern-device-comparison.json).

## Run 1

MEASURED: 90,070 s, funcional PASS; precisión **FAIL**. Android
p50/p95/p99/max=39/178/227/5000 ms, n=191. Cross-device=27/203/249 ms, n=186.
Hard/soft/restore=0/5/3. [Raw](modern-device-run-1.json),
[timing](modern-device-run-1-timing.json).

## Run 2

MEASURED: 90,072 s, funcional PASS; ambas metas p95 **PASS**. Android
31/148/159/5000 ms, n=192. Cross-device=20/148/199 ms, n=190.
Hard/soft/restore=0/6/3. [Raw](modern-device-run-2.json),
[timing](modern-device-run-2-timing.json).

## Run 3

MEASURED: 90,071 s, funcional PASS; ambas metas p95 **PASS**. Android
41/148/163/5000 ms, n=191. Cross-device=24/143/191 ms, n=191.
Hard/soft/restore=0/8/5. [Raw](modern-device-run-3.json),
[timing](modern-device-run-3-timing.json).

## Runs adicionales

MEASURED: funcional PASS y teardown confirmado en ambas.

| Run | Wall s | Android p50/p95/p99/max ms | n | Cross p50/p95/p99 ms | n | Hard/soft/restore | Gate |
| --- | ---: | --- | ---: | --- | ---: | --- | --- |
| [4](modern-device-run-4.json) | 90,059 | 33/137/154/5000 | 191 | 25/148/193 | 191 | 0/7/4 | PASS ambos |
| [5](modern-device-run-5.json) | 90,061 | 27/185/230/5000 | 190 | 33/200/255 | 189 | 0/6/2 | FAIL ambos |

Timing completo: [4](modern-device-run-4-timing.json) y
[5](modern-device-run-5-timing.json). Cinco repeticiones funcionalmente completas,
sin seleccionar ni eliminar los dos fallos de precisión.

## Comparación de drift

MEASURED: valores por corrida; no percentil calculado sobre promedios ni mezcla
de poblaciones.

| Métrica ms | Antiguo Profile 1 / 2 / 3 | Moderno Profile 1 / 2 / 3 / 4 / 5 |
| --- | --- | --- |
| Android p50 | 11 / 41 / 26 | 39 / 31 / 41 / 33 / 27 |
| Android p95 | 153 / 178 / 195 | 178 / 148 / 148 / 137 / 185 |
| Android p99 | 203 / 245 / 240 | 227 / 159 / 163 / 154 / 230 |
| Android max | 250 / 251 / 241 | 5000 / 5000 / 5000 / 5000 / 5000 |
| Linux p95 | 63 / 46 / 42 | 67 / 55 / 63 / 60 / 64 |
| Android gate ≤150 | 0/3 | 3/5 |

La mediana descriptiva de los p95 baja de 178 a 148 ms; los dos fallos modernos
178/185 ms impiden declarar precisión consistentemente estable. Los máximos
5000 ms corresponden a preparación antes de Play: posición SDK 5000 frente a
target vigente 0. Permanecen en el agregado primario, como exige el filtro
histórico all_loaded_trusted; no representan cinco segundos de lag reproducido.
No se modifica ese filtro ni se usa el subset estable para aprobar.

## Comparación cross-device

| Métrica ms | Antiguo 1 / 2 / 3 | Moderno 1 / 2 / 3 / 4 / 5 |
| --- | --- | --- |
| p50 | 67 / 26 / 20 | 27 / 20 / 24 / 25 / 33 |
| p95 | 155 / 183 / 209 | 203 / 148 / 143 / 148 / 200 |
| p99 | 202 / 244 / 283 | 249 / 199 / 191 / 193 / 255 |
| max | 283 / 262 / 287 | 251 / 200 / 192 / 197 / 256 |
| Gate ≤150 | 0/3 | 3/5 |

MEASURED: misma proyección de posición libmpv al timestamp servidor de Android,
pares con sequence/playing coincidentes y delta ≤600 ms. No equivale a captura
óptica simultánea de frames. Mediana descriptiva de p95: 183 →148 ms.

## Comparación hard seeks

| Corrección | Antiguo 1 / 2 / 3 | Moderno 1 / 2 / 3 / 4 / 5 |
| --- | --- | --- |
| Hard seek | 1 / 0 / 1 | 0 / 0 / 0 / 0 / 0 |
| Soft rate | 4 / 4 / 4 | 5 / 6 / 8 / 7 / 6 |
| Restore | 2 / 3 / 2 | 3 / 3 / 5 / 4 / 2 |

MEASURED: no storm ni hard seek en las cinco corridas modernas. Las correcciones
soft aumentan; cero hard seeks por sí solo no demuestra convergencia completa.
Los seeks SDK de preparación/controles siguen presentes en raw, sin contarlos
como hard corrections del SyncEngine.

## Comparación Looper

| p95 ms | Antiguo 1 / 2 / 3 | Moderno 1 / 2 / 3 / 4 / 5 |
| --- | --- | --- |
| Scheduler lateness | 2 / 2 / 1,5 | 2 / 2,5 / 4 / 2,5 / 2,5 |
| Effect queue | 22 / 71 / 115 | 20 / 21 / 20 / 20 / 74 |
| JNI roundtrip | 1 / 2 / 2 | 2 / 2 / 2 / 2 / 2 |
| Looper lateness, trace periódico | 32 / 40 / 45 | 4 / 1 / 4 / 5 / 1 |
| Looper lateness, por efecto | 2 / 85 / 157 | 1 / 7 / 1 / 1 / 70 |
| Post→execution, por efecto | 22 / 105 / 177 | 21 / 27 / 21 / 21 / 90 |
| Dispatch inicial del control | 67 / 25 / 32 | 27 / 22,5 / 26,5 / 23,5 / 29 |

MEASURED: mejora marcada del trace periódico, pero el moderno tampoco elimina
todos los retrasos: Run 5 tiene 70 ms por efecto aunque su trace periódico marque
1 ms p95. Cada efecto referencia el runnable del poll; varios pueden compartir
runnable. Post→execution incluye el periodo deliberado de 20 ms. Effect queue
se mide enqueue→drain en Rust; no sumar dos latencias que se solapan.
JNI y scheduler no muestran una mejora comparable a Looper/seek.

## Comparación Media3

| p95 ms | Antiguo 1 / 2 / 3 | Moderno 1 / 2 / 3 / 4 / 5 |
| --- | --- | --- |
| Seek→READY | 438 / 366 / 451 | 267 / 212 / 225 / 224 / 315 |
| Seek→posición estable | 530 / 425 / 477 | 291 / 243 / 292 / 260 / 347 |
| Seek pérdida de avance medida | 422 / 370 / 470 | 269 / 213 / 226 / 224 / 317 |
| Llamada SDK | 23 / 15 / 20 | 12 / 13 / 12 / 10 / 13 |

MEASURED: Media3 1.11.1 y la misma heurística SDK de READY/dos observaciones
compatibles. Estadísticas heredadas sobre todos los seeks, incluyendo
preparación, controles y correcciones; número/tipo de operación puede variar.
READY y posición estable son señales SDK, no garantías de presentación audiovisual.
INFERRED: el conjunto dispositivo/OS/ABI contribuye fuertemente a estas latencias.

## Comparación Play startup

MEASURED_INTERVAL: tiempo desde la **primera llamada Media3 Play** hasta el
primer avance SDK está acotado por la última observación sin avance y la primera
con avance. El trace existente toma posición cada 25 polls; no tiene resolución
suficiente para afirmar el instante exacto. El analyzer corta antes de otro
seek/pause y usa timestamps Kotlin de fuente. No se confunde isPlaying con avance.

| Run | Antiguo: intervalo ms | Moderno: intervalo ms |
| --- | --- | --- |
| 1 | 7–350 | 22–554 |
| 2 | 6–140 | 2–576 |
| 3 | 7–266 | 2–90 |
| 4 | — | 9–297 |
| 5 | — | 63–611 |

INFERRED, asumiendo rate=1 sin rate/seek intermedio: elapsed menos avance de
posición en la primera muestra =135/119/116 ms antiguos frente a
209/139/77/123/194 ms modernos. Es pérdida efectiva de avance, no un timestamp
exacto de comienzo ni latencia perceptual. Los modernos que fallan (1/5)
coinciden con pérdidas mayores (209/194); hipótesis de arranque pendiente de
diagnóstico, sin corrección aplicada. Play→isPlaying p95 de todas las operaciones
es 15/13/9 ms antiguo y 9/6/7/9/3 ms moderno: ese callback no mide lo mismo.
UNTESTED: primer frame/audio físico e instante exacto del primer avance.

Las conversiones al deadline Rust usan el bracket de la misma operación;
elapsedRealtime y Rust Instant no se restan como si compartieran origen.
Se preserva bracket, incertidumbre de cuantización y bounds en JSON.

## Comparación CPU

| % de un core | Antiguo 1 / 2 / 3 | Moderno 1 / 2 / 3 / 4 / 5 |
| --- | --- | --- |
| Sesión completa, estadística heredada | 76,44 / 70,32 / 76,28 | 82,85 / 85,66 / 86,25 / 76,11 / 76,12 |
| Ventana de controles | 90,18 / 81,26 / 89,09 | 84,51 / 87,14 / 87,60 / 76,66 / 76,47 |

MEASURED: ticks CPU del proceso/tiempo monotónico, porcentaje de un core. La
estadística completa abarca ~120 s en el antiguo y ~95 s en el moderno: incluye
preparación de distinta duración. La ventana comparable toma el primer resource
sample después de Play inicial hasta el último antes de Pause final, ~85–90 s,
sin interpolación. No supone cores de igual potencia, frecuencia o microarquitectura.
No hay reducción uniforme de CPU ni prueba de saturación como causa exclusiva.
Temperatura de batería 30–32 °C al empezar los cortos, thermal status 0; no es
una medición de temperatura/frecuencia CPU. Polling y logging no se modificaron.

## Foreground/resume

MEASURED separado: [raw recovery](modern-device-recovery.json),
[timing](modern-device-recovery-timing.json),
[background](modern-device-recovery-background.json). Profile, 90,192 s,
funcional PASS, identidad de miembro preservada y teardown confirmado.
Android p50/p95/p99/max=93/9394/13392/13437 ms (n=182); cross-device
55/214/243 ms (n=154); hard/soft/restore=0/8/4. Gate **FAIL**. Sin mezclar con
los cinco cortos controls-only. Incluso el subset descriptivo room-ready da
p95 217 ms; tampoco aprueba y no sustituye el agregado primario.

| Wait del harness ms | Antiguo: 600 s Debug V2 | Moderno: 90 s Profile |
| --- | ---: | ---: |
| Foreground → marcador RECOVERED | 1908,18 | 1356,02 |
| Convergencia medida después del wait Ready, foreground | 0,03 | 7383,31 |
| Reconnect → connected + Ready | 13195,23 | 12229,87 |
| Convergencia medida después del wait Ready, reconnect | 1559,13 | 5179,17 |
| Mismo miembro tras resume | Sí | Sí |

UNTESTED: comparación de recuperación con modos/duraciones idénticos; esta
tabla es contexto. La desconexión es WebSocket mediante el control experimental,
no pérdida de radio Wi-Fi. Los tiempos incluyen waits/automatización UI del
harness y no son latencia pura de transporte.

LIMITACIÓN ENCONTRADA, sin fix: convergence cuenta tres muestras válidas ≤150 ms
en toda la ventana, **no exige consecutivas**. El índice foreground comienza
antes de HOME, no al volver a Ready. Sus waits no prueban tres muestras estables
consecutivas después de Ready; el valor histórico casi cero tampoco representa
recuperación instantánea. Debe diagnosticarse antes de convertirlo en garantía.
Los transitorios de recuperación room-not-ready se conservan en métricas primarias.

## Conclusión A/B

INFERRED: **B. DEVICE CONTRIBUTES**. Disminuyen latencias de seek y del Looper;
la mediana de p95 mejora y hay tres pases modernos frente a cero históricos.
Los cortos 1/5 fallan y la recuperación separada falla ampliamente. No corresponde
A: cambiar de teléfono no consigue ≤150 ms consistentemente. Tampoco describimos
las latencias del adapter como similares al antiguo. Evidencia moderada para
beneficio de drift, clara para latencias; límites V1/V2, OS/ABI y muestra pequeña
impiden una conclusión causal exclusiva o una probabilidad de éxito futura.

## ¿El teléfono antiguo está limitando las métricas?

INFERRED: contribuye a colas/completion más lentos, pero **no explica por sí solo
los fallos de precisión**. El moderno aún alcanza p95 178/185 ms y recuperación
muy fuera de meta. No se atribuye automáticamente todo a la edad del teléfono.

## ¿Recomiendas seguir optimizando Android antes de empezar la UI?

INFERRED: sí. Primero estabilizar y medir arranque real de Play y recuperación;
no aprobar UI de producto por tres corridas favorables ni por cero hard seeks.
No se ha optimizado Android en esta tarea ni cambiado thresholds para aprobar.

## Próximo paso

Repetir el antiguo con **el mismo cc7544b/V2 Profile** y alternar dispositivos,
controlando arranque frío/caliente, para eliminar la diferencia histórica de
versión y acotar el beneficio. Después instrumentar, si se autoriza como siguiente
tarea, el primer avance real de posición de Play con mayor resolución y la
convergencia consecutiva posterior a Ready. No cambiar algoritmo a ciegas.

IMPLEMENTED: [analyzer reproducible](analyze_modern_device.py) y evidencia nueva.
Reproducción: ejecutar python3 experiments/08-android-sync-precision/analyze_modern_device.py;
regenera comparación y timing modernos a partir de raw, sin tocar históricos.
TESTED: agregados Android/cross-device de los ocho cortos y la recuperación
reproducidos desde muestras; verificación de ABI/hash/Ready/controles/teardown.
Verificaciones finales se registran en modern-device-verification.json. Se
conservan todos los intentos y los fallos; no hay cambios de producción.
