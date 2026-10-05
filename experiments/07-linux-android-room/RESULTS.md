# Resultados — Linux ↔ Android

**Recorrido funcional ejecutado; gate experimental de precisión NO aprobado.**
Sesión real de **600,08 s**, Linux/libmpv + Samsung SM-J701M Android 9/API 28
armv7, Flutter/Rust/Media3, WebSocket por Wi-Fi/LAN. Sin emulador, adb reverse,
reloj acelerado ni transporte de vídeo por servidor. Fuente: cambios del slice
sobre b792f37; [datos completos](results-lan.json).

## Resultado funcional

Crear/join, archivo local Linux y SAF Android, SHA-256 completo, identidad igual,
Ready real, Participant NOT_AUTHORIZED sin incremento de secuencia, Play/Pause,
Seek pausado y playing, background/foreground, desconexión WS y resume con mismo
miembro: ejecutados. Server/rooms/Core no incorporan SDK, URI, paths o bytes.
El instrumento marca PASS al terminar el recorrido, **no** al cumplir 150 ms.
Métricas y parámetros SyncEngine no se ajustaron para forzar esa meta.

## Diez minutos

| Métrica absoluta | Linux | Android | Diferencia alineada |
| --- | ---: | ---: | ---: |
| p50, ms | 3 | 33 | 29 |
| p95, ms | 36 | 514 | 462 |
| p99, ms | 47 | 7230 | 541 |
| máximo, ms | 5000 | 12742 | 600 |
| muestras | 1306 | 1126 | 1099 |

Primarias incluyen preparación/recovery con SDK loaded y clock trusted; se
excluyen seeking/buffering, clock no confiable y muestras >100 ms. No se eliminan
outliers. El máximo Linux de 5000 ms corresponde al seek previo al Play futuro:
posición preparada aún distinta del timeline pausado anterior.

Se detectó que el campo original room_ready reflejaba el último snapshot durante
disconnect. Se corrigió instrumentación, sin cambiar SyncEngine. Datos originales
se conservan; el análisis adicional efectivo excluye snapshot_required: Android
p50 33 / p95 **474** / p99 545 / max 5000 ms, 1102 muestras. Tampoco cumple 150 ms.

Proyección A/B usa timestamps servidor cercanos, sequence y modo compatibles,
rate Linux y delta ≤600 ms. Es posición reportada/proyectada, no simultaneidad
óptica de frames. La cifra p95 462 ms no es garantía pública.

| Corrección/recurso temporal | Linux | Android |
| --- | ---: | ---: |
| soft rate | 0 | 7 |
| restore | 0 | 4 |
| hard seeks | 0 | 75 |
| soft/min | 0 | 0,70 |
| hard/min | 0 | 7,50 |
| scheduler/SDK dispatch p95, ms | 2 | 432 |
| dispatch >50 ms | 0 | 2 |

Los seis controles poseen deadline y sequence compartidos. Android scheduler,
dispatch SDK, playing_changed y seek_READY están separados. La cifra dispatch
por sequence conserva el último efecto de ese comando, pudiendo incluir Play
posterior a seek completion; ver raw eventos para el primer dispatch.

Seek SDK READY: 81 observaciones, 59–433 ms; seek pausado target 10000 llegó a
10000 en 86 ms. Seek playing target 36469 llegó a 36480 (+11 ms) en 393 ms.
READY/evento no prueba frame presentado. La latencia de completion introduce
atraso contra un timeline que sigue avanzando; se observaron hard seeks repetidos.
Es un mecanismo plausible respaldado por eventos, no diagnóstico exclusivo:
UI debug, main Looper, audio/decoder y transporte de observaciones requieren perfil.

## Clock, hashing y recovery

Android RTT del filtro 2–70 ms; incertidumbre 2–41,5 ms; offsets monotónicos
11340–11374,5 ms (orígenes diferentes, no diferencia de hora civil). Última muestra:
RTT 8 ms, incertidumbre 9,5 ms, 73 muestras desde el último reset. Se conservan
jitter y counters en raw sync. Suspensión invalida clock/generation; PONG de una
generación anterior se descarta.

Corpus H264/AAC, 160×90, 15 fps, 620 s, **24300725 bytes**. SHA-256 streaming
1 MiB: Linux **834,38 ms**, Android **486,89 ms**. FD SAF regular duplicado, checks
antes/después, cancelación; MIME/codecs Android opcionales. No digest/URI/path
personal/tokens en resultados. [Mismatch físico](results-mismatch.json): clip con
un byte añadido abre en Media3, hash completo y MEDIA_MISMATCH; Participant no
Ready, sala preservada.

Foreground recovery: **2059,95 ms** hasta clock/snapshot/reload/hash/Ready. Tres
muestras dentro de 150 ms ya estaban disponibles al iniciar la observación
post-ready (0,03 ms de espera, **no** latencia total de convergencia).
Disconnect WS separado: mismo member_id tras resume, **13310,06 ms** hasta Ready,
**16557,98 ms** adicionales hasta reunir tres muestras dentro de 150 ms.
[Recovery](results-background.json) y [smoke](results-smoke.json) conservan valores.
La regresión final corta de 88,05 s pasó el recorrido y teardown, pero no reunió
tres muestras ≤150 ms en 20 s después de foreground/resume: null, no PASS de
precisión. Foreground hasta Ready: 2208,02 ms; resume WS hasta Ready: 13280,69 ms.

## Recursos y teardown

Linux RSS 73196→73820 KiB, máximo 73820; 21 threads, 15 FDs; CPU ~8,51% de un
core. Android **PSS**, no RSS: 194843→161334 KiB, máximo 194843; 40–51 threads,
110–175 FDs; CPU ~90,84% de un core. Medidas orientativas de proceso completo,
Flutter debug incluido; no atribuir todo al bridge/decoder ni inferir release.

Tras destroy Rust/SDK: observación Android PSS 161752 KiB, 49 threads, 160 FDs;
luego force-stop y procesos locales reaped. Destroy no significa que Flutter/VM
libere inmediatamente todos sus recursos. Variaciones de FDs/threads no prueban
crecimiento no acotado ni su ausencia. Se observaron release markers en smoke,
mismatch y sesión larga; no prueba exhaustiva de recreación/lifecycle.

SDK Linux opt-in: seis tests aprobados; cinco ciclos owner y sesenta ciclos
libmpv mantuvieron FDs/threads cerca del baseline y retuvieron RSS. Datos en
[resultados SDK](results-sdk-regressions.json); sigue pendiente análisis del allocator.

## Regresiones y límites

FakePlayer/control, Linux real corto, UI Flutter Linux y Android Player aislado
se verifican con los scripts anteriores. Evidencia en results-*-regression.json.
Tests host de MobilePlayer usan WebSocket real y SDK fixture, no un Android falso.
Build NDK/APK e instalación no se confunden con runtime físico.
Verificaciones ejecutadas: [registro](results-verification.json), 90 tests Rust
ordinarios, seis SDK opt-in y cinco Flutter; fmt/clippy/build/docs aprobados.

Historial de fallos: puerto bloqueado inicialmente por UFW; subredes distintas y
Wi-Fi apagado; SAF devolvía resultado antes de resumed; selector ocultaba fixture
por scroll y Flutter botones exponían content-description; generaciones/hash y
muestras SDK viejas. [Diagnósticos](results-diagnostics.json) conserva el historial.
El usuario corrigió LAN/firewall; harness no modificó firewall/qdisc ni obtuvo root.

La regresión final encontró una carrera de Ready tras foreground: la observación
SDK aún no era fresca/cargada y Rust rechazaba correctamente MEDIA_NOT_READY.
Application ahora espera hasta 5 s, reintentando solo ese error cada 20 ms, con
cancelación por generation/destroy; mismatch/clock/autoridad no se reintentan.
No relaja las condiciones Ready ni los 100 ms de edad máxima SDK. El fallo queda
en [ready race](results-ready-race.json) y el recorrido posterior en
[smoke final](results-smoke.json). Los datos de 600 s previos se conservan intactos.
El harness Android local también se corrigió para scroll SAF y doble Back de
teardown; su fallo previo se conserva en results-android-local-failed.json.

No se ejecutó degradación controlada de red, rotación/recreación exhaustiva,
release/perfil optimizado, múltiples modelos ni iOS. iOS sigue requiriendo Mac/Xcode.

## Conclusión y siguiente gate

La estrategia híbrida y Application compartida controlan Players reales por LAN.
**La precisión móvil sigue pendiente: p95 Android 514 ms y 75 hard seeks impiden
considerar satisfecho el objetivo de sincronización.** Flutter, libmpv y Media3
continúan provisionales; v0.1 no está terminada.

Siguiente tarea: perfilar latencia/cola/main Looper y completion durante seek/rate,
conservar protocolo/Core, probar una política Application de convergencia que
respete timestamps y completion, y repetir Linux↔Android 600 s antes de ampliar UX.
No subir el threshold ni excluir datos solamente para aprobar la cifra.
