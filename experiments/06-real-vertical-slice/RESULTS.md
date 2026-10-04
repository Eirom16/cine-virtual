# Resultados — Vertical slice 1 Linux

Ensayo final: 2026-10-04, CachyOS/Arch Linux x86_64. Implementación sobre base
27311ff, cambios de integración en working tree antes del commit. Rust 1.99,
mpv package 1:0.41.0-6, client API 2.5. Solo localhost; no distribución/release.
Libmpv continúa PROVISIONAL FOR LINUX; v0.1 no está terminado.

## Evidencia y método

[results-linux.json](results-linux.json): ejecución final de **600,733 segundos
reales** con servidor y dos ejecutables CLI distintos, cada uno con libmpv
in-process. [results-faults-linux.json](results-faults-linux.json): segunda
modalidad corta de 27,114 s con fallos explícitos. [results-lifecycle-linux.json](results-lifecycle-linux.json):
cuatro tests SDK anteriores y dos nuevos del proxy/owner, con recursos de ciclos.
No se aceleró reloj. Se repitió el recorrido largo después de corregir contexto
de resume y pérdida de confianza del reloj. Datos de diagnósticos anteriores
no sustituyen el ensayo final y no se versionaron como resultados finales.

Corpus existente/reproducible: long-duration.mp4, 620 s, 160×90, 15 fps, GOP150,
H264/AAC, 24.300.725 bytes, generado por scripts/generate_test_media.py. Outputs
null, decoding software real, audio decodificado con salida null. No test visible
ni audio audible de dos clientes; Spike B tiene evidencia aislada, no de este slice.

Cada owner consulta SDK/posición con timestamp de su reloj cada ~5 ms; Application
observa cada 500 ms. Target se proyecta al timestamp de la muestra, no al terminar
el RPC. Percentil nearest rank sobre |actual-target| de todas las muestras
**localmente cargadas**, clock trusted, no seeking/buffering y edad ≤100 ms.
Incluye pausado y preparación previa; no exige que el miembro ya haya declarado
Ready al servidor. Los datos excluidos se conservan con flags: A 2/B 6; incluidos
A 1209/B 1199. No se retocaron thresholds del Core.

Comparación A/B: consultas cada ~0,9 s, timestamps distintos. Se reporta diferencia
raw y proyectada a la muestra de B usando el rate de A; es estimación temporal,
no captura simultánea de frames. Durante pasos bloqueantes de control/fallos
hay huecos entre esas parejas; las muestras internas de 500 ms siguen guardadas.
CPU se deriva de delta /proc/stat utime+stime, CLK_TCK=100, respecto a tiempo real
(100% significa un core). Hubo verificaciones/builds concurrentes; no es benchmark
con máquina aislada. resources_summary se calcula después a partir de datos crudos.

## Flujo funcional comprobado

PASS: creación/join, dos miembros Host/Participant, LocalMedia y hashes finales,
metadata propia, MEDIA_VERIFIED emitido por servidor y MEDIA_READY solo después
de carga estable y clock confiable. Ready con Player vacío fue bloqueado con
MEDIA_NOT_READY. Copia con bytes añadidos produjo MEDIA_MISMATCH y dejó B no Ready;
seleccionar copia correcta permitió continuar. Participant PLAY_REQUEST produjo
NOT_AUTHORIZED sin cambiar sequence. Server/rooms/protocol/Core sin dependencias
nuevas de multimedia/filesystem; vídeo/URI/ruta nunca se envían.

Los mismos deadlines programan Play/Pause/Seek en los dos clientes. Se probó
seek pausado y playing. B desconecta, pausa localmente; A continúa. B recalibra
8 muestras, resume antes del lease, recibe snapshot, revalida handle/digest final
retenidos y declara Ready. No se repiten controles viejos. Cierre final pausado,
misma posición reportada en ambos y tres procesos hijos reapeados.

## Drift y controles durante diez minutos

| Métrica | A | B |
| --- | ---: | ---: |
| p50 absoluto ms | 24 | 13 |
| p95 absoluto ms | 36 | 42 |
| p99 absoluto ms | 65 | 43 |
| máximo absoluto ms | 5000 | 5000 |
| soft corrections/min | 0 | 0 |
| hard corrections/min | 0 | 0 |
| hard seeks de SyncEngine | 0 | 0 |
| deadlines primarios >50 ms tarde | 0 | 0 |
| lateness máximo dispatch primario ms | 2,5 | 3 |

Máximo 5000 ms es real y se conserva: durante preparación de Play a 5 s, mpv ya
hizo seek pausado mientras el timeline actual del servidor sigue en 0 hasta su
deadline. No es drift sostenido durante playing; sí limita una afirmación de
alineación absoluta en todo momento. El render visible podría mostrar ese frame
previo: este ensayo headless no aprueba cuándo se presenta físicamente.

No hicieron falta correcciones automáticas en el ensayo normal. **Sí se aplicaron
en el ensayo con fallos**, indicado abajo; cero en esta tabla no incluye seeks
solicitados por Host, preparación ni reconciliación del snapshot.

Diferencia A/B alineada: p50 1 ms, p95 5 ms, p99 65 ms, máximo 68 ms (659 parejas).
Diferencia raw p95 0 ms; consulta secuencial y frame quantization explican por qué
no equivale a posición simultánea. Diferencia final pausados: 0 ms.

## Dispatch y completion

SDK playing significa pause=false, no frame presentado. First position advance
ocurrió 20–22 ms tras Play; primer incremento observado ≈67 ms (un frame a15fps).
Evento playing llegó ~5–6 ms tras dispatch. Primera Pause: evento +1 ms desde
dispatch, posición estable −6 ms respecto al target; final pause −9 ms.
Estabilidad usa muestras iguales ≥50 ms, no sleep fijo que presuponga completion.

| Acción | Target ms | SDK completion A/B ms desde dispatch | Position A/B ms | Landing error A/B ms |
| --- | ---: | ---: | ---: | ---: |
| Preparación de Play | 5000 | 26 / 32 | 5000 / 5000 | 0 / 0 |
| Seek pausado | 10000 | 64 / 62 | 10000 / 10000 | 0 / 0 |
| Seek playing | 24533 | 41 / 41 | 24533 / 24533 | 0 / 0 |

Primario significa primera llamada correspondiente a la orden (play/pause/seek)
con ese deadline/sequence. Las pausas internas después de seek no cuentan como
otro deadline perdido. Events conserva también transiciones de snapshot/
disconnect/corrección; sequence indica versión de sala y no identifica por sí sola
una operación de SDK. pause_stable histórico no contiene reason: al interpretar
control_metrics usar target y proximidad al dispatch, pues incluye observaciones
locales posteriores con la misma sequence. Sus campos error_ms fuera de contexto
no son drift autoritativo: para drift usar sync_sample. Queda una mejora de
atribución de ese log; no afecta los percentiles ni controles primarios.

## Clock y resume

Clock final A: RTT 0 ms, offset 22 ms, incertidumbre 0 ms, 123 samples.
B: RTT 0 ms, offset 1054 ms, incertidumbre 0 ms, 101 samples desde resume.
Offsets distintos expresan orígenes de proceso, no relojes de sistema erróneos.
RTT/uncertainty 0 reflejan resolución de 1 ms/filtro y no precisión física cero.

B desconectó en server monotonic 126489 ms; pausa local por ~3 s. Resume respuesta
804,361 ms, drift inicial −3840 ms durante recuperación; convergió 63,728 ms
después de la respuesta (≈868,089 ms desde solicitar resume), drift observado −37 ms.
Snapshot y secuencia compartidos comprobados. No se necesitó reload/rehash porque
el handle y el digest calculado siguen vigentes; modificación obliga select otra vez.

## Fallos controlados: resultado distinto del ensayo normal

Seek pequeño local (+120 ms), pausa local, reloj no confiable durante1600 ms,
y seek local a120 s. Pérdida de clock invalida Ready vía MEDIA_NOT_READY reason=user;
Ready se recupera explícitamente con reloj confiable. B convergió en todas las
inyecciones, observaciones a partir de 2 s tras cada comando (resolución de prueba,
no tiempo mínimo exacto); el caso clock se observó en ≈2062 ms.

B ejecutó una corrección rate, una restore y dos hard seeks del SyncEngine real;
clock recovery usa además snapshot/reconciliación. p95 B **5000 ms**, p99/máximo
**91307 ms**, mientras A p95 31 ms. Meta p95 ≤150 **NO pasa** incluyendo fallos y
sus transitorios. No se cambiaron thresholds para esconderlo. Todos los datos
crudos y los gaps/exclusiones se mantienen. El p95 normal 36/42 ms pasa solo en
el entorno/corpus/red medidos, no como garantía de producto.

Buffering real/seek deliberadamente lento/red degradada no fueron inyectados.
No se creó framework de fault injection; fault es un comando diagnóstico local.

## RSS, threads, FDs y CPU

| RSS KiB | A | B |
| --- | ---: | ---: |
| Antes de crear mpv | 6396 | 6412 |
| Después de crear | 54844 | 55012 |
| Después de primera carga | 73260 | 73208 |
| Tras seleccionar copia correcta (B) | — | 85336 |
| Inicio de reproducción medida | 73748 | 85492 |
| Después de ~10 min / máximo | 74392 | 85684 |
| Después de destroy SDK | 72688 | 83944 |

CPU medio A≈7,53%, B≈7,60% de un core; threads21–22 y FDs15 constantes durante
reproducción. Después de destroy: 8 threads/11 FDs frente a6/10 antes de crear;
el checkpoint aún contiene el owner antes de su join, runtime y handle LocalMedia.
Después los procesos terminaron y fueron reapeados; no se lanzó mpv externo.
No afirmar que ese checkpoint ya haya destruido toda Application.

Cinco ciclos del owner, SDK en el mismo proceso: RSS33704→78472 KiB, threads2→2,
FDs4→4. SDK previo60 ciclos: RSS34248→119384 KiB al terminar, checkpoints internos
160084/160092/164100 KiB (incluyen biblioteca aún cargada). Threads2→2 y FDs4→4.
El RSS retenido existe; no demuestra crecimiento ilimitado ni liberación total.
Mantener diagnóstico pendiente sin convertir esta integración en proyecto de memoria.

Hash streaming: A911,139 ms/B909,609 ms para24,3 MB en build debug. Buffer1 MiB
más estado SHA-256, sin archivo completo en RAM; no se aisló RSS del hash del
resto del runtime. Comparación antes/después y callback permiten detectar cambios
(o cancelar); tests incluyen mutación/reemplazo y vector SHA-256 conocido.

## Verificaciones, correcciones y límites

73 tests workspace pasan, 6 SDK ignorados por defecto ejecutados aparte y verdes.
Incluye todos los anteriores (64 basales) y9 nuevos puros. fmt, Clippy -D warnings,
build, documentación, demo FakePlayer, SDK4+2 y demos de tres procesos reales.
Se corrigieron acceso directo a FakePlayer, seeks/reset de rate en cada snapshot,
uso de posición sin timestamp conjunto, atribución de resume/disconnect y Ready
anunciado con clock no confiable. Una tolerancia local35 ms evita seeks redundantes
al reconciliar; thresholds50/250 ms,3 samples,2 s cooldown/±2% del Core no cambian.

Riesgos: C/RPC bloqueantes pueden demorar el runtime aunque SDK tenga owner;
timeout no preempta C. SDK no aislado contra crashes. TOCTOU/filemutable y falta
watcher después de Ready; cache efímera no prueba inmutabilidad. Sin rollback
transaccional de selección rechazada, UI/cancel interactivo/backoff automático,
WAN, HD/4K/hardware/visible-audio de dos clientes, licencia o packaging aprobados.

| Plataforma | Research existente | Build de este slice | Runtime de este slice | Resultado |
| --- | --- | --- | --- | --- |
| Linux | Sí, Spike B | Sí | Sí | Provisional validado headless localhost |
| Windows | Sí, Spike B | No ejecutado | No ejecutado | Sin validar |
| macOS | Sí, Spike B | No ejecutado | No ejecutado | Sin validar |
| Android | Sí, Spike B | No ejecutado | No ejecutado | Gate pendiente |
| iOS | Sí, Spike B | No ejecutado | No ejecutado | Gate pendiente |

Siguiente gate: dispositivos Android/iOS, URI/recursos locales, lifecycle/thread,
render/rate/seek y build/licencias antes de promover motor o bridge. Mantener
regresión del slice Linux; no UI/voz/P2P/providers ni declaración de v0.1 terminado.
