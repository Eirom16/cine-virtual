# Spike B — Resultados Linux

Fecha: 2026-10-04. Evidencia propia: Linux/CachyOS/Arch x86_64, mpv 0.41.0/API 2.5,
FFmpeg n9.0.2, Rust 1.99. Datos completos:
[results-linux.json](results-linux.json). Son runs de una máquina y corpus pequeño,
no benchmarks generalizables ni certificación de precisión visual/móvil.

## Estado y decisión

- IMPLEMENTADO: un adapter in-process, puerto Player sin cambio de firmas,
  load/eventos locales, CLI automático/manual, mediciones y SyncEngine sin red.
- PROBADO: Linux, decoding real H.264/AAC, MP4/MKV/VFR, controles, rates, EOF,
  errores, reemplazo, lifecycle, ventana GPU/Xwayland, audio aislado y 600 s.
- INVESTIGADO: cuatro familias y las cinco plataformas en [RESEARCH](RESEARCH.md).
- PLANEADO: dos vídeos + WebSocket, hashing/probe/Ready reales, móvil y UI embebida.

**Clasificación D: SUFICIENTE PARA CONTINUAR AL VERTICAL SLICE Linux con candidato
PROVISIONAL.** Ningún motor definitivo, ni v0.1 terminado. Móvil/licencias/memoria
siguen siendo gates antes de promover o distribuir.

## Método

Tiempo Instant monotónico real. Load ready espera FILE_LOADED/restart y consultas.
Playing espera pause=false y avance de position. Pause se considera estable tras
cinco observaciones iguales a intervalos ~10 ms: stable_ms incluye ese mínimo
artificial de observación, no representa latencia perceptual de 50 ms.

Seek dispatch se mide separadamente; completion requiere SEEK seguido de restart.
Pausado se verifica estabilidad durante tres observaciones de ~10 ms. Position
es tiempo reportado por SDK, no timestamp del frame presentado. Rate se estima por
Δposition/Δmonotonic entre extremos de ventanas de ~2 s tras 300 ms de asentamiento;
no es una regresión ni una prueba estadística. Samples ~50 ms en automático.
El residual de deltas contra reloj nominal incluye cuantización/timers/correcciones;
no se llama jitter del display ni error de reloj puro.

## Carga, play y pause

| Corpus/modo | Load ready ms | Duración ms | Output/hardware |
|---|---:|---:|---|
| normal / headless | 26.04 | 30000 | null/null, software |
| long-gop / headless | 23.94 | 30000 | null/null, software |
| VFR / headless | 15.32 | 30021 | null/null, software |
| audio-video / visible | 296.16 | 30000 | gpu/pulse, VAAPI reportada |
| long-duration / headless | 43.25 | 620000 | null/null, software |

normal: dispatch play 0.11–0.42 ms; evento playing 1.28–2.30 ms;
primer avance 1.74–3.17 ms. Seek-to-play position parte de 5000/20000/29200 ms.
Pause dispatch 0.02–0.27 ms; estabilidad observada 50.94–53.44 ms incluyendo
la ventana de medición; avance después del comando **0 ms reportado**.
Visible pause 52.61–53.00 ms con el mismo mínimo de observación y 0 ms de avance.
Estos tiempos no miden drenaje del altavoz, pantalla física ni última muestra.

## Seek (pausado, valores reportados)

| Corpus | Target ms | Completion ms | Reported ms | Landing error ms |
|---|---:|---:|---:|---:|
| normal | 5000 | 8.28 | 5000 | 0 |
| normal | 20000 | 11.93 | 20000 | 0 |
| normal | 29200 | 10.09 | 29200 | 0 |
| long-gop | 5000 | 34.79 | 5000 | 0 |
| long-gop | 20000 | 121.76 | 20000 | 0 |
| long-gop | 29200 | 93.53 | 29200 | 0 |
| VFR | 5000 | 6.06 | 5000 | 0 |
| VFR | 20000 | 5.61 | 20000 | 0 |
| VFR | 29221 | 6.65 | 29233 | +12 |
| visible/VAAPI | 5000 | 16.03 | 5000 | 0 |
| visible/VAAPI | 20000 | 25.58 | 20000 | 0 |
| visible/VAAPI | 29200 | 14.41 | 29200 | 0 |

Otros seeks y dispatch/stable_ms están en JSON. Completion normal 8–26 ms frente
hasta 122 ms con GOP largo. Un landing 0 en position no prueba frame exacto.
Seek cerca del final terminó en EOF; el harness nunca excedió duración.

## Position y rates

normal: 29 samples en ~1,5 s, 0 backsteps/repeats, residual absoluto de deltas
p95 ~20 ms. Cuantización compatible con 30 fps, no muestreo continuo perfecto.

| Rate solicitado | Rate observado headless normal | Rate visible | Tono aislado Hz |
|---:|---:|---:|---:|
| 0.98 | 0.9716 | 0.9684 | 440.0 |
| 1.00 | 0.9884 | 1.0117 | 440.0 |
| 1.02 | 1.0260 | 1.0228 | 440.0 |
| 0.95 | 0.9481 | 0.9513 | 440.0 |
| 1.05 | 1.0544 | 1.0502 | 440.0 |
| 1.00 restaurado | 1.0034 | 0.9866 | 440.0 |

La velocidad cambia realmente según timeline, aunque una ventana breve produce
error de estimación ~0.01. No hubo retrocesos en las ventanas de rate.
Audio medido en sink Pulse temporal aislado: PCM mono 48 kHz, RMS ~0.088,
cero ventanas silenciosas de 10 ms entre las 159 de cada tramo estable. Pitch
correction activada; la medida por cruces de cero conserva 440 Hz. Excluye cambios
de rate, no mide artefactos perceptuales ni audio speech/music ni dispositivos
móviles. No se hizo escucha humana; null audio no prueba calidad.

## Visible y hardware

Ventana real del SDK con GPU/X11 EGL sobre Xwayland, patrón sintético inspeccionado
mediante captura del ID de la ventana, 320×180. También screenshot-to-file del
vídeo propio cerca del final, sin capturar escritorio/contenido privado. Archivos
PNG quedan en corpus ignorado. Play/Pause/Seek/rate y EOF completaron el run visible.
No se ensayó embedding de surface Flutter ni la ruta Wayland nativa completa.

hwdec=auto produjo hwdec-current=vaapi. Una advertencia de búsqueda CUDA sin
biblioteca no impidió seleccionar VAAPI. Headless usa hwdec=no y decodificación
software real. No se midió consumo GPU ni ahorro energético, ni se garantiza
aceleración para otros codecs/dispositivos. Media de baja resolución.

## SyncEngine + Player real

Sin WebSocket ni FakePlayer. Timeline objetivo monotónico local con lead inducido
120 ms; Core decide rate y adapter lo aplica. normal terminó a +48 ms tras ~9 s;
long-gop +53 ms; VFR +26 ms; visible +72 ms. Variación cerca de deadband genera
varios cambios rate/restauración: thresholds siguen provisionales, no se retocaron
para maquillar una medición. Rate final restaurado a 1.0.

Con lead de 700 ms Core decidió hard seek. normal: target 14238, completion 10.78 ms,
reported 14233 (−5 ms), drift final +24 ms; visible +82 ms. La diferencia entre
landing de un seek en reproducción y timeline al terminar incluye ejecución/avance.
La prueba aislada demuestra dispatch real de ambas estrategias y convergencia
aproximada; no garantiza sincronía entre dos pantallas.

## Prueba prolongada real

600175.18 ms transcurridos, medio de 620 s, 1194 muestras a ~500 ms.
Drift absoluto relativo a timeline monotónico: p95 **74 ms**, máximo **165 ms**,
0 backsteps/repeats, residual p95 de deltas 38 ms. Doce cambios de rate y ningún
hard seek (1.2 cambios/minuto), sin errores. No se aceleró reloj ni se usó seek
para fingir duración. Se restauró 1.0 al terminar.

CPU orientativa 3.59% de un core, calculada desde ticks /proc con HZ=100 confirmado.
RSS inicial/final 67104/68380 KiB; threads 15/15 y FDs 8/8 durante reproducción.
El run empezó antes de corregir el selector de audio visible; el modo null/null
y el algoritmo de reproducción/sync no cambiaron por esa corrección. La medición
se conserva tal como ocurrió. Nueva CLI pide CINE_SPIKE_CLOCK_TICKS para CPU,
y sin frecuencia explícita reporta no medido.

## Errores y recursos

Tests SDK ejecutados: archivo inexistente (FileNotFound), vacío (EmptyMedia),
corrupto/no-media (LoadFailed), seek >duración (SeekOutOfRange), segundo seek
pending (SeekPending), rate cero/NaN (UnsupportedRate por límites del adapter),
antes de load (NotLoaded), después de destroy (Destroyed y mapping BackendFailure).
El engine soportó todos los rates válidos ensayados; **no se provocó un fallo de
capacidad rate del SDK**. Error de audio durante playback se mapea a BackendFailure,
separado de LoadFailed. destroy dos veces y poll tras destroy son seguros.

60 ciclos create/load/play/destroy devolvieron threads/FDs a baseline, sin FD del
archivo retenido. Hay RSS retenido importante/variable; valores y checkpoints
reales en JSON. Un loader dinámico, dependencias y allocators pueden retener
memoria, pero eso no demuestra que no exista fuga. En la última suite, RSS antes/después fue 33816/118432 KiB y los checkpoints
con la biblioteca aún cargada fueron 155904/156384/163060 KiB. Falta análisis allocator/leak
instrumentado; el test solo exige recursos de ownership y no impone RSS cero.
No se registran callbacks: no pueden ejecutarse tras destroy; poll posterior se
rechaza antes del SDK. Todos los procesos y ventanas del harness terminaron;
sinks temporales eliminados. No se observan procesos mpv/vlc usados como adapter.

## Problemas encontrados y corregidos

1. ao=auto no es un driver válido: se elimina esa opción para audio real y se usa
   selección del SDK; null solo cuando se solicita headless silencioso.
2. EOF con error de audio se etiquetaba como load: ahora distingue fase Loading
   de playback para conservar taxonomía.
3. Reemplazo puede entregar END_FILE del medio previo: stop/idle/drenaje antes de
   iniciar la nueva generación; test específico.
4. PlayerError no implementaba std::error::Error; se añaden Display/Error sin
   cambiar trait ni dependencias. El primer build expuso esa necesidad.
5. Captura de root X11 sobre Wayland resultó negra; usar el ID de nuestra ventana
   obtuvo contenido real, sin ampliar captura al escritorio.

6. Un assert SDK de drift final instantáneo ≤70 ms falló en una repetición. Se
   cambió a mediana del último segundo con tolerancia deadband + un frame del
   corpus (84 ms a 30 fps), manteniendo las decisiones y thresholds Core. Es un
   check de convergencia reportada, no una garantía de 70 ms ni precisión visual.

## Estado por plataforma

| Plataforma | Research | Build Cine Virtual adapter | Runtime | Resultado |
|---|---|---|---|---|
| Linux | Sí | Sí | Sí | corpus headless, GPU/Xwayland, Pulse, VAAPI |
| Windows | Sí | No ejecutado | No ejecutado | pendiente |
| macOS | Sí | No ejecutado | No ejecutado | pendiente |
| Android | Sí | No ejecutado | No ejecutado | URI/lifecycle/build pendientes |
| iOS | Sí | No ejecutado | No ejecutado | SDK/build/store/lifecycle pendientes |

## Verificación y siguiente gate

Los 62 tests existentes deben mantenerse verdes. Se añaden dos tests puros y
cuatro tests SDK opt-in; fmt/Clippy/build/docs/diff, suite SDK y demo real de
Spike A se ejecutan antes del commit, ver informe final. No se cambia el protocolo
ni RoomService y no se conecta WebSocket al Player real aquí.

Siguiente tarea: vertical slice Linux con dos vídeos idénticos, pipeline
load/probe/SHA-256/Ready y scheduler/replica desacoplados del FakePlayer; medir
scheduled controls y snapshot/resume sin promover motor ni UI. Gates posteriores:
Android/iOS reales, surface embebida, build/licencias, audio y memoria instrumentada.
