# Estrategia de pruebas

## Ejecutable hoy

`cargo test --workspace` ejecuta pruebas deterministas de Core, RoomService, codec
y réplica con tiempo numérico inyectado, sin sleeps. Además ejecuta dos pruebas
WebSocket sobre TCP localhost real, con servidor/cliente in-process y timers Tokio.
Estas últimas esperan notificaciones con timeout y no exigen un Player multimedia.
`cargo fmt`, Clippy y build
completan las comprobaciones de Rust. `python3 scripts/check_docs.py` comprueba
links locales, JSON de ejemplos, capítulos y formato básico de documentación.

| Escenario | Cobertura de esta pasada |
| --- | --- |
| A=100000 ms, B=100083 ms | Signo +83, rate de frenado tras tres muestras |
| Cliente retrasado y jitter de posición | Rate aumenta; cambio de signo evita oscilación |
| Drift pequeño/grande y paused | Deadband, hard seek, restauración rate y cooldown |
| Clock con latencia/jitter | Ecuaciones, filtro, muestra inválida y asimetría |
| Pending futura y recepción tardía | Timeline conserva current y proyecta estado al vencer |
| Eventos duplicados/antiguos/fuera de orden | SequenceGate ignora o exige snapshot |
| Desconexión/reconexión | Gate requiere snapshot; estado completo incluye pending |
| Cambio de epoch | Rechazo sin reinicialización explícita |
| Identidad multimedia | Tamaño+digest; metadata no reemplaza hash |
| Capacidades Player | Fake player y tolerancia sin rate |

Transferencia Host, autorización, tokens, readiness global y deduplicación de
requests tienen tests en [rooms/tests](../rooms/tests/service.rs). La réplica y
scheduler se prueban en [client/tests](../client/tests/replica.rs); codec y ejemplos
normativos en [protocol/tests](../protocol/tests/codec.rs).
[server/tests](../server/tests/websocket.rs) comprueba el flujo completo y sender_id
falsificado sobre conexiones reales. La demo de tres procesos está en
[scripts/demo_control.py](../scripts/demo_control.py).
Un test de timeline con Player falso no demuestra precisión de SDK o scheduler.

## RoomService y protocolo implementados

Fixture sin red: reloj falso, store en memoria y sesiones simuladas; assert de
estado y efectos. Crear/unirse, límite, roles, claim inválido, Ready viejo,
selección nueva, Host desconectado reemplazando pending, gracia/expiración,
transferencia solo pausado y rechazo de old authority. Mutación fallida no consume
sequence. Same event_id/payload no duplica efectos; otro payload se rechaza.
Reintentos en nueva conexión no restauran un control antiguo.

Fixtures JSON golden compartidas con cliente futuro: envelope, State con pending,
ACK/ERROR, unknown version/type, claves duplicadas, límites, signed drift y nulls.
Decoder debe rechazar input inválido antes de tocar dominio. El codec serde con
validación explícita ya existe; los ejemplos JSON de PROTOCOL
se leen directamente desde el documento y se validan con ese mismo codec.

## Simulación de red y sistema futura

El Core cubre muestras de reloj con latencia/jitter/asimetría y la réplica cubre
duplicados/gaps/reconexión. Pendiente harness completo de fallos sobre el adapter:
cola virtual con reloj para latencia 0–1000 ms, jitter y asimetría, duplicados,
reordenación, gaps, pérdidas por cierre y reconexión durante pending. WS ordena
frames en una conexión; pérdidas/reordenación se simulan en el nivel de sesión
y aplicación. No modelar datagramas como si WS fuera UDP.

Invariantes: controles solo del Host autenticado; media_revision vigente; cada
mutación única aumenta una vez sequence; snapshot reemplaza backlog; ningún
timer antiguo se ejecuta tras reset; offset/target finitos y posición acotada.
Después de converger, drift permanece en tolerancia del adapter bajo red medida.
Más adelante property tests/fuzz de codec y máquina de estados; no dependencias
añadidas hasta disponer de un comportamiento real que probar.

## Pruebas con multimedia y plataformas

Corpus sintético o redistribuible y con licencia indicada: MP4/H264/AAC, MKV,
framerate variable, vídeos largos, seek cerca del final, corrupción y archivo
cambiado. Sin vídeos privados en repositorio/CI. Medir exactitud/keyframes,
audio/rate, decodificación hardware, pause/resume, URI Android y permisos Apple.
El Core portable se compila/prueba en Windows/Linux/macOS en CI cuando se conecte
el repositorio; Android/iOS necesitan toolchains y dispositivos para adapters.

Primer gate de integración: dos clientes reales con el mismo archivo, Play/Pause/
Seek, ensayo 10 min, métricas p95, desconexión, copia discrepante y Host inválido.
Android/iOS deben ensayarse antes de elegir motor/puente, no después de diseñar
una UI final. Registrar evidencia en experiments sin promocionar el spike a producto.

## Gate aislado de multimedia: Spike B

Antes del recorrido con dos vídeos se ejecutó Core + un adapter in-process Linux,
sin WebSocket: [experimento 01](../experiments/01-player-crossplatform/README.md).
Los 62 tests de Spike A se conservan; dos tests nuevos puros validan límites/mapeo
de errores del adapter, sin SDK. Cuatro tests SDK están ignorados por defecto y
se ejecutan explícitamente con corpus generado y libmpv: controles/rates/EOF y
SyncEngine, errores/destroy, reemplazo de medio y 60 ciclos de lifecycle.

Headless decodifica con null outputs; no demuestra ventana/audio. Se ejecutó
además visible GPU/Xwayland, captura de la ventana sintética, audio aislado y
600 segundos reales de reproducción. Completion/position reportados no equivalen
a frame físico ni calidad perceptual. Los resultados machine-readable conservan
samples y método. No se declaran tests Windows/macOS/Android/iOS ejecutados.

## Vertical slice 1: pruebas de integración real

LocalMedia tiene tests sin SDK: vector SHA-256 conocido, mismo/distinto contenido,
progreso/cancelación, modificación durante lectura, reemplazo de pathname,
metadata inválida y descriptor mínimo. Application valida Ready con Player aún
cargando/error/buffering/seek/duración inválida. La réplica genérica cubre selección
fake/unknown y snapshots periódicos que no resetean rate/posición.

Dos tests SDK adicionales son opt-in: el proxy con owner usa el mismo scheduler
+ SyncEngine sobre vídeo real, y cinco ciclos create/load/destroy verifican
threads/FDs y registran RSS. Ejecutar sin concurrencia para recursos comparables:

```sh
python3 scripts/generate_test_media.py
cargo test -p cine-player-mpv --test real_player -- --ignored --test-threads=1 --nocapture
cargo test -p cine-client --test replica -- --ignored --test-threads=1 --nocapture
python3 scripts/demo_control.py
python3 scripts/demo_real_media.py
python3 scripts/demo_real_media.py --seconds 25 --faults
```

La demo real usa servidor y dos clientes **en procesos distintos**, mpv headless
con decoding real, hash de corpus, copia discrepante, Participant no autorizado,
Play/Pause/Seek pausado y durante reproducción, disconnect/resume/snapshot y
~600 s de tiempo real. La ejecución corta añade seek local pequeño/grande,
pausa local y reloj no confiable (invalida Ready), conservando transitorios/fallos de la meta p95.
No simula un seek lento ni buffering de red; no introduce framework de fallos.
Resultados y metodología en
[vertical slice 1](../experiments/06-real-vertical-slice/RESULTS.md).
