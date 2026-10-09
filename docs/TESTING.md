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

## Spike C: UI/ABI y gate móvil

cine-ui-bridge prueba DTO/version/rangos, errores, handles destruidos, capabilities,
observaciones al SyncEngine original, cancelación/FD y lifecycle que borra reloj,
Player sample y deadlines. LocalMedia añade lector genérico y read failure; Core,
codec y RoomService conservan su cobertura. No mocks Android presentados como runtime.

La pantalla Flutter tiene tests de frontera/lifecycle, destroy async y widget.
Requieren build local del C ABI y CINE_BRIDGE_LIBRARY apuntando al .so de target/debug.
Runtime SDK Android se ensaya con build NDK/APK y emulador/dispositivo, SAF, hashing por
FD, Media3/surface, controles y background/resume. Datos/versiones y comandos en
[experimento 02](../experiments/02-rust-ui-bridge/README.md).
Tests host y build no sustituyen runtime; iOS sin toolchain está bloqueado.

```sh
cargo test -p cine-ui-bridge -p cine-local-media
python3 scripts/demo_mobile_ui.py --platform linux
python3 scripts/build_mobile_bridge.py --abi x86_64
python3 scripts/demo_mobile_ui.py --platform android
python3 scripts/demo_control.py
python3 scripts/demo_real_media.py --seconds 25
```

La prueba Linux corta conserva evidencia de 10 min del slice anterior; no sustituye
su ensayo prolongado. No se modificó profundamente el runtime Player/red Linux.

## Vertical slice 2 — Application compartida y dispositivo físico

Tests nuevos usan Client<MobilePlayer> y el **WebSocket real**, con vistas SDK
falsas únicamente en tests host. Cubren Ready sin load, autoridad Participant
sin cambio de secuencia, invalidación suspend, clock/snapshot foreground, resume,
claim mismatch, cola acotada/generation y cancelación/join durante connect.
Frontera C ABI y Flutter añaden DTO UUID inválido, prohibición de fixtures en modo
sala, generaciones viejas y acceso después de destroy. No se presentan fixtures
SDK como runtime Android. La política del Core no cambia.

```sh
cargo test --workspace
python3 scripts/demo_cross_platform.py --seconds 30
python3 scripts/demo_cross_platform.py
```

La segunda demo requiere Android físico y Wi-Fi/LAN, sin adb reverse; ~600 s de
reloj real y evidencia en [experimento 07](../experiments/07-linux-android-room/README.md).
Ausencia de ADB, conectividad o fallo de SDK produce bloqueo/fallo, no métricas
inventadas. UI/ABI Linux y Player Android aislado se conservan con demo_mobile_ui;
FakePlayer y dos libmpv con demo_control/demo_real_media. Android build/install
no equivale a runtime cruzado; verificar resultados físicos antes de promover gates.

## CI de portabilidad

[CI](CI.md) automatiza checks base y builds por plataforma; distingue build,
linking y runtime. SDK headless Linux es opt-in y no valida display/audio físico.
No teléfono físico ni pruebas de diez minutos en cada push. Apple device/simulator
son compilaciones unsigned, con Player explícitamente no implementado.

## Precisión Android

[Experimento 08](../experiments/08-android-sync-precision/README.md) conserva
raw métricas sanitizadas, corridas debug/profile antes/después, faults separados
y ensayo largo condicionado a convergencia repetida sin storm. Tests numéricos
cubren pérdida de avance durante seek, proyección/edad de target, paused exacto,
media duration, generation/operation invalidation y estimación sin sleep fijo.
Réplica verifica que samples pending/stale no corrigen y que samples frescos
vuelven a activar corrección. Una observación pausada estable no invalida la
medida posterior de pérdida de avance al reanudar Play. Tests Python verifican
offset entre dominios, preservación de OTHER y rechazo de callbacks de completion
invalidados por comandos opuestos, conservando la evidencia. El diagnóstico no aprueba frame/audio perceptual ni todos
los dispositivos; functional PASS y p95 gate se publican separadamente.


## Product UI Phase 2: render y recovery

[Experimento 10](../experiments/10-player-integration/README.md) conserva spike,
backend, screenshots y resultados separados de Phase 1. Los frames no se validan
con widget tests. El test nativo puro comprueba dimensiones inválidas, límite
4096, clear/reveal, generation vieja y recreate. El test SDK de lease comprueba
que el owner sobrevive al drop de su referencia inicial hasta liberar la lease.

```sh
c++ -std=c++14 -Wall -Werror experiments/02-rust-ui-bridge/app/linux/runner/presentation_state_test.cc -o /tmp/cine-presentation-test
/tmp/cine-presentation-test
cargo test -p cine-ui-bridge video_lease_retains_owner_until_render_detach --locked -- --ignored --test-threads=1
```

El test WebSocket Host revalidation conserva selección/timeline pausada en 45 s
con SDK temporalmente en 0, bloquea identidad incorrecta/Ready y confirma que
selección explícita sigue creando revisión nueva. Los tests previos cubren
generation vieja y suspend/resume. Flutter cubre fullscreen/Escape Participant
sin autoridad y scrub con un único seek al commit, además de todos los anteriores. Otro test prueba un único platform view Android
al ocultar/mostrar controles y un único dispose al desmontar.

```sh
cd experiments/02-rust-ui-bridge/app
CINE_BRIDGE_LIBRARY="$PWD/../../../target/debug/libcine_ui_bridge.so" flutter test
flutter analyze
```

QA físico: Create/Join/SAF/hash/Ready, controles reales Linux ↔ Android, resize,
fullscreen/exit, Lobby/Player, reconnect y cinco foreground recoveries. Guardar
solo corpus sintético y telemetría saneada; nunca tokens, URI privadas o hashes.
La pérdida real del contexto GPU y multi-monitor requieren evidencia adicional.

## Social Phase 1

Pruebas nuevas de RoomService/codec/Client/WebSocket: identidad ligada a binding,
spoof en envelope y payload, Unicode/control/longitud, scope/epoch, dedup tras
ACK y socket cerrado/resume, ordering, buffers por count/bytes/escape JSON,
cuotas por miembro conservadas tras resume, allowlist y ausencia de replay de
reacciones. Dos clientes FakePlayer reales por TCP conservan Ready/Play/Pause/Seek.
Clientes v1 sin capability no reciben social. Ráfagas y retención prolongada usan
reloj inyectado, sin spam externo ni sleeps en RoomService.

`app/test/social_test.dart`: cerrado/abierto, autores/sistema, Enter/Shift+Enter,
error/draft, unread, scroll/indicador, picker accesible, cleanup/cap de animaciones,
foco que bloquea playback, ausencia de rebuild de estructura y redacción.
Viewports: 1366×768, 1920×1080, 390×844, 844×390, 568×320, incluyendo teclado.
Los widget tests no prueban frames ni dispositivos físicos. Evidencia física,
limitaciones y comandos en [experimento 11](../experiments/11-social-chat-reactions/README.md).

La regresión de teclado comprueba también cursor y Ctrl+A/Ctrl+V: proteger Player
no debe deshabilitar los atajos de edición. `DefaultTextEditingShortcuts` local
resuelve edición antes de los shortcuts del Player; Enter confirma, F sin
modificadores se entrega al editor y Esc puede cerrar el panel.

Opt-in adicional: `cargo test -p cine-server --test social_real_player --locked -- --ignored`
usa dos libmpv reales con el corpus sintético. No sustituye la QA visual física.

## Rich Social Phase 2

[Experimento 12](../experiments/12-rich-social/README.md) distingue transporte/render
fixture de búsqueda real (REAL PROVIDER SEARCH NOT TESTED). Tests CI no usan APIs
externas ni credenciales. Rust cubre descriptor/URL/metadata, scope/identidad, reply,
toggle/aggregation/cuotas/dedup, snapshots y budget mixto con reacción máxima.
WebSocket real dos Client/FakePlayer cubre GIF → reply → reacción → replay sin
duplicados ni cambios a playback; legacy social_v1 recibe fallback compatible.
Opt-in social_real_player ahora repite Rich Chat con dos decoders libmpv reales.

Flutter gif_provider_test cubre mapping search/trending/ID/pagination, empty,
missing asset/malformed JSON, HTTP fixture 429/offline/oversize, timeout, debounce,
stale/dispose y cache LRU/clear. rich_social_test cubre picker/selección explícita,
blocked/text fallback, reply/cancel/eviction, copy, reacción/toggle, hora/GIF,
fallo/layout fijo, reduced motion, cinco viewports/teclado y SurfaceView estable.
Capturas widget se pueden generar con CINE_SOCIAL_CAPTURE y CINE_SOCIAL_FONT_DIR;
no representan runtime multimedia. Pruebas físicas y medidas están separadas.

## P2P Media Distribution Phase 1

`cargo test -p cine-transfer -p cine-client -p cine-rooms -p cine-protocol -p cine-server --locked`
cubre manifest, bounds, corrupción, dedup, checkpoint, SHA, pin/peer auth, replay,
expiry, room/epoch/media/autoridad, lifecycle y compatibilidad WS/WSS. Tests TLS
reales localhost y WSS validan bytes fuera del canal de sala.
Flutter transfer_test usa gateway fixture para consentimiento/espacio/progreso/
acciones/verificación/Ready y layouts 360/1280; no sustituye dispositivos.
[Experimento 13](../experiments/13-p2p-media-distribution/README.md) conserva
pruebas físicas, mediciones, QA visual, fallos y límites. P2P_QA opt-in solo facilita
entrada/observaciones sobre backend real; no acepta descarga automáticamente.
