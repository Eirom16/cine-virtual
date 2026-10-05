# Vertical slice 2 — Linux ↔ Android

Recorrido de ingeniería previo a v0.1. Flutter **PROVISIONAL**, libmpv
**PROVISIONAL FOR LINUX**, Media3 **PROVISIONAL FOR ANDROID**. iOS no forma parte
de este trabajo; requiere macOS/Xcode. Evidencia ejecutada en [RESULTS](RESULTS.md).

## Arquitectura e implementación

`Client<P: ApplicationPlayer + Send>` reutiliza el runtime de red probado por la
CLI: codec v1, sesión, identidad, réplica, clock T1–T4, scheduler y SyncEngine.
Solo el proxy necesita Send; Core/Player y los SDKs no reciben ese requisito.
`attach_media` recibe un descriptor portable producido por el dispositivo, sin
filesystem en rooms/server. Se mantiene `select` desktop y FakePlayer.

Android: Flutter envía intents por el C ABI existente. Un owner Rust con runtime
Tokio current-thread ejecuta el mismo Client con MobilePlayer, proxy de muestras y
efectos de SDK. Kotlin/main Looper posee Media3, SAF y superficie. Un driver nativo
cada 20 ms observa SDK y drena una cola de 32 efectos mediante un shim JNI pequeño;
**no calcula timelines, clock, autoridad o correcciones**. UI a 2 Hz; no segundo
cliente WebSocket Dart, segundo RoomState Flutter o segundo SyncEngine Kotlin.

Polling es deliberado: 32 intents/efectos, 16 engines máximos, respuesta ABI ≤64 KiB.
SDK dispatch, scheduler Rust y READY SDK se registran separadamente. JNI no cambia
la elección C ABI + dart:ffi para la API Application y no expone structs Core.

## Ownership y lifecycle

Activity registra el handle Rust al crear Flutter. Destroy detiene driver,
libera SDK/FD/URI y cancela/une el owner Rust en un worker; Dart dispose async es
una segunda vía idempotente. Registry remueve el handle antes de joins, fuera del
mutex. Owner cancela incluso connect en curso y cierra socket/tasks antes de salir.
Hash usa worker cancelable sobre FD regular duplicado; no URI convertido a path.
Solo se liberan permisos persistentes adquiridos por esta sesión.

Suspend invalida generation, SDK samples/efectos, confianza de reloj y deadlines;
pausa/restaura rate e informa MEDIA_NOT_READY conservando el socket. Foreground
obtiene ocho muestras nuevas, snapshot real, rehash/reload y Ready antes de
convergencia. Ready espera de forma acotada hasta 5 s por una observación SDK
fresca/usable; se cancela por generation/destroy y no reintenta mismatch, clock
o permisos. Document Picker también provoca lifecycle: no comenzar hash antes
de volver a foreground y terminar la recuperación. Disconnect es otra operación:
cierre WS, lease, resume con mismo miembro/token rotado y snapshot, sin replay.

## Build y comandos

Requisitos: Linux con libmpv/ffmpeg, toolchains de [Spike C](../02-rust-ui-bridge/README.md),
Android físico autorizado por ADB, desbloqueado y en la misma LAN accesible de la
laptop. Sin adb reverse, túnel ni emulador. Un solo endpoint ADB.

```sh
cargo build --workspace
python3 scripts/build_mobile_bridge.py --abi armeabi-v7a
python3 scripts/demo_cross_platform.py --seconds 30
python3 scripts/demo_cross_platform.py --mismatch-only --no-build
# Sesión ~600 s reales; genera/reutiliza corpus, build/install y SAF físico:
python3 scripts/demo_cross_platform.py
# Puerto explícito si la política existente de la LAN lo requiere:
python3 scripts/demo_cross_platform.py --port 1729 --no-build
```

La demo compila `ROOM_MODE=true`/`ROOM_AUTORUN=true`. `--no-build` exige ese APK,
no el autorun local de Spike C. Para UI manual compilar ROOM_MODE=true sin
ROOM_AUTORUN y ejecutar la APK. Server field usa ws:// + IPv4/puerto manual;
invitación JSON privada contiene room_id/room_epoch/invite_token. No usar esa
cadena experimental como URL pública ni publicarla en métricas.

```sh
cine-server --bind 0.0.0.0:8765 --allow-lan
cine-client --server ws://127.0.0.1:8765 --name Linux --player mpv
# Cliente CLI remoto experimental: --allow-lan true
```

Sin flag, servidor/cliente conservan loopback. LAN emite warning, conserva 64 KiB,
colas/miembros/rate limits y no distribuye media. **ws sin cifrado solo para LAN
controlada**; WSS y hardening siguen siendo gates de producción. El harness no
modifica firewall/qdisc ni obtiene root. UFW/reachability son prerrequisitos.

## Corpus y métricas

`long-duration.mp4` sintético H264/AAC 620 s, generado fuera de Git. La demo pone
una copia temporal reservada en Downloads, la elige mediante SAF real y la retira
al terminar; no sobrescribe un fixture preexistente. Hash SHA-256 completo 1 MiB:
Linux LocalMedia, Android el mismo lector Rust sobre FD. Título/ruta/URI/digest y
credenciales nunca se exportan a evidencia.

Resultados se guardan en results-lan.json (largo), results-smoke.json (corto) y
results-background.json. Fallos/ausencia de hardware no se convierten en PASS.
Drift usa timestamp de muestra, target servidor y descarta clock no confiable,
buffering/seek activo, media no cargada o muestra >100 ms; conserva excluidos.
Diferencia cruza muestras cercanas del mismo sequence/modo y proyecta Linux al
tiempo servidor de Android usando su rate; no compara llamadas secuenciales como
si fueran frames simultáneos. La meta p95 ≤150 ms es experimental, no garantía.

Regresiones: demo_control, demo_real_media --seconds 25, demo_mobile_ui Linux y
Android local. Build no equivale a runtime. Corpus, archivos personales y raw
logs no se versionan; resultados solo campos seleccionados.

Mismatch físico usa otra copia con un byte añadido: Android calcula el hash,
abre el clip real y obtiene MEDIA_MISMATCH sin entrar Ready. El selector se opera
por UI SAF, incluyendo scroll de Downloads; botones Flutter se buscan por
semantics/content-description además de text. No se inyecta un content URI.

Las estadísticas primarias conservan muestras durante preparación/recuperación
con SDK cargado y clock trusted, aunque Ready de sala aún sea falso. Se publica
además el subconjunto room_ready. Convergencia ≤150 ms requiere tres muestras y
se observa hasta 20 s; si no sucede se registra null, no se detiene ni se inventa
una convergencia. Missed deadline aquí significa dispatch >50 ms tarde. READY
tras seek es completion del SDK, no prueba de frame renderizado. Una smoke puede
durar más que --seconds para completar las cuatro acciones obligatorias; la
sesión larga sigue usando tiempo real y registra su duración observada.
