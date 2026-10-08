# Player integration & recovery — resultados

**IMPLEMENTED / TESTED / VISUALLY VERIFIED:** vídeo Linux dentro de Flutter,
overlays, resize, fullscreen y salida sin reiniciar sesión. **TESTED:** bug Host
Android reproducido antes del fix; cinco recoveries Playing conservan timeline,
identidad y Ready. No se modifica protocolo, autoridad ni tuning de SyncEngine.

Evidencia crítica: [Player Host Linux 1366×768](screenshots/linux-host-normal-1366.png).
Es vídeo libmpv real compuesto en la ventana Cine Virtual. KWin contó una ventana
Cine Virtual y cero ventanas mpv. No son mockups ni frames de un widget test.

## Entorno y estado inicial

master limpio en cc1116d, origin coincidente; GitHub autenticado y CI inicial
[37538977527](https://github.com/Eirom16/cine-virtual/actions/runs/37538977527)
SUCCESS. Se leyeron README, arquitectura, protocolo, sync, media, testing, ADRs,
CI, UI y experimento 09 antes de modificar. Sus capturas se conservan intactas.

Linux: GTK 3.24.52, GdkWaylandDisplay, KDE KWin 6.7.5, Flutter 3.47.6
fijado en 5fc346839b5d0eef006ed8404392afb4dfae428d, libmpv API 2.5/mpv 0.41.
Un monitor físico 1366×768. Android: Samsung SM-J701M, API 28, armeabi-v7a,
Media3/SAF reales. WS por Wi-Fi LAN, sin adb reverse; adb forward solo para
observar el debug VM. Corpus sintético compartido, duración 620 s; no benchmark
de sincronización de 600 s ni recalibración de p95.

## Auditoría, decisión y arquitectura

La cadena anterior y comparación de Render API, FlTextureGL, GTK, wid X11,
wl_surface y buffers CPU están en [embedding-spike](embedding-spike.md).
ADR-011 en [DECISIONS](../../docs/DECISIONS.md) registra contexto, alternativas,
decisión y consecuencias. libmpv continúa **PROVISIONAL**.

La Render API produce un FBO en un worker EGL propio, compartido desde el
contexto real de Flutter. Raster copia por GPU a la textura consumidora. Una
copia GPU por frame; sin readback, RGBA en Dart ni rebuild Flutter por frame.
No es zero-copy. El owner Rust continúa ejecutando controles del SDK y mantiene
la autoridad existente. C ABI Linux acquire/release presta el handle únicamente
al runner nativo; una lease mantiene vivo el owner durante presentación.

El callback mpv despierta una condición; no ejecuta controles. Worker libera el
render context con EGL actual antes de devolver lease. Detach despierta y une
worker fuera del mutex de notificación. Raster y productor tienen mutex GPU y
glFinish en sus límites. Registrar retiene la textura hasta shutdown del engine;
finalizer libera EGL antes del EGL del engine. No se mezclan GdkGLContext y EGL
del engine por suposición.

La textura/contexto sobreviven Lobby ↔ Player, fullscreen y reconnect. Clear/
reveal y generación de presentación invalidan frames durante cambio de medio.
Resize físico explícito evita que el bootstrap 1×1 determine la resolución.
Contain/letterbox lo conserva mpv; overlays visibles cubren parte de la imagen
sin alterar su proporción, y al ocultarlos se ve la región completa.

## Linux físico

| Escenario | Resultado / evidencia |
| --- | --- |
| Spike aislado antes de integración | TESTED: frame, play 2.267 s, seek 45 → 46.867 s, pause, resize, destroy/recreate. |
| Create/Join → archivo → hash → Ready → Player | TESTED con backend real; vídeo embebido, misma ventana. |
| Play/Pause/Seek Host | TESTED por input real; Space y flechas ±10 s, posición SDK y timeline observadas. |
| Participant | TESTED controles deshabilitados; widgets verifican también que shortcuts no emiten autoridad. |
| Resize | TESTED: 30 cambios, cada 150 ms, 700–1285 × 450–630; error nativo 0. Sin basura/crash observado. |
| Aspect ratio / overlays | VISUALLY VERIFIED: black bars, header/timeline/botones Flutter encima; panel participantes. |
| Fullscreen / exit | TESTED / VISUALLY VERIFIED: F/Esc, chrome restaurado, misma room/member/media/timeline. |
| Lobby ↔ Player | TESTED: diez ciclos, textura/generación conservadas, frames siguen avanzando. |
| Disconnect/reconnect | TESTED: identidad, Ready y vídeo recuperados, error render 0. |
| Cerrar ventana GTK con render activo | TESTED: salida 0, sin hang/crash. |
| Wayland | TESTED solo en KWin y GPU del entorno. No se declara universal. |
| X11 / segundo monitor | NOT TESTED. X11 compatible por diseño, sin wid/reparenting. No segundo monitor disponible. |

Capturas reales a 1366×768 y 1920×1080. La segunda es una ventana Wayland real
sobredimensionada sobre monitor 1366×768; no es reescalado de imagen ni una prueba
en monitor Full HD. Índice y estados exactos en [screenshots](screenshots/README.md).

## Fullscreen y controles Android

**IMPLEMENTED / TESTED / VISUALLY VERIFIED:** immersiveSticky, portrait y
landscape, SafeArea, controles accesibles, participantes en bottom sheet, Back
sale de fullscreen antes de navegar. Sin orientation locking. Ajustes de rotación
del dispositivo restaurados tras QA. Posición pausada ~286.5 s y sesión conservadas.

Se encontró un segundo bug: `edgeToEdge` es ignorado por el engine bajo API 29;
al salir quedaba la barra de estado oculta. Se corrigió restaurando explícitamente
ambos overlays en modo manual. Probado físicamente con la biblioteca Dart final
hot reloaded (1 biblioteca), sin reiniciar sesión; APK correspondiente compilado.
[Captura de salida](screenshots/android-exit-fullscreen.png) muestra la barra
restaurada y vídeo en 4:46. Android 15/16 y sus restricciones de system UI
**NOT TESTED**. No se modifican políticas de cutout ni locks complejos.

La platform view Android tiene key estable: un test reproducía dos creaciones
al ocultar controles; ahora mantiene una creación y un dispose al desmontar.
La prueba valida lifecycle, no frames. La relación con cualquier negro observado
al llegar al EOF queda **INFERRED**, no se atribuyen todos los negros a esta causa.
Navegar físicamente Lobby → Player mostró nuevamente vídeo y posición 286519 ms.

## Android: reproducción del bug y causa

**REPRODUCED / TESTED:** APK Phase 1 instalado, fuente auditada en cc1116d
(el commit del binario instalado no fue atestado independientemente). Host a
32509 ms; Home 5 s; foreground termina con hash completo, Media3 en 0/paused,
target 44194 ms, acción attach y INVALID_STATE. Participant Linux continúa
aproximadamente en 45600 ms; misma room/member/media revision.

Recovery reutilizaba `attach_media`: después de `setMediaItem` local, que vuelve
temporalmente a 0, Host enviaba MEDIA_SELECT_REQUEST. Mientras Playing el servidor
lo rechazaba INVALID_STATE; en estado pausado, seleccionar puede crear revisión
y timeline nuevas. Esta reselección no es una restauración de la selección actual.

**IMPLEMENTED:** intent local revalidate, sin mensaje nuevo en wire. Limpia la
verificación anterior, comprueba descriptor/hash/generación e identidad contra
el medio actual y asocia el descriptor local. Después Ready y reconciliación
existente restauran la timeline. Attach explícito conserva su semántica de nueva
selección. Kotlin sigue haciendo SAF, hash completo y Media3 load; no se evita
validación ni se inventa lastPosition en Flutter. Core/SyncEngine no cambian.

## Cinco recoveries definitivos

Serie nueva con APK del fix, ambos clientes Ready/Playing, lejos de EOF. Misma
room/member en las cinco, revisión media 1. La generación local cambia de 3 a 13
como exige invalidar observaciones; cada foreground completa hash e identidad.
Participant conserva Ready y reproducción. Ninguna posición final vuelve a 0.

| Run | Background | Host antes (ms) | Host final (ms) | Target (ms) | Generación final | Correcciones seek adicionales | Resultado |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | Home 5 s | 99348 | 112183 | 112818 | 5 | 0 | PASS |
| 2 | Home 15 s | 116698 | 139298 | 139922 | 7 | 1 | PASS |
| 3 | Home 5 s | 143939 | 210246 | 210226 | 9 | 1 | PASS |
| 4 | Home 30 s | 212207 | 250439 | 250190 | 11 | 1 | PASS |
| 5 | Home 5 s | 254075 | 269449 | 269449 | 13 | 1 | PASS |

Run 3: primer Ready aún estaba seeking, antes de terminar el play encolado del
SDK. El observador se detuvo demasiado pronto; se conserva esa muestra y una
observación estable posterior Playing/not-seeking. No se atribuye una latencia
de recovery precisa a ese intervalo. El harness se ajustó para esperar también
Playing/not-seeking. Se cuentan correcciones del Core, no todos los seeks internos
Media3. No se afirma «cero seeks» ni se estiman drift/p95 con lecturas secuenciales.

Una serie preliminar llegó al EOF en runs 3–5; se conserva separada y **no cuenta**
para este resultado. Probe opcional de screen lock quedó **BLOCKED** por keyguard
seguro antes de alcanzar foreground. No se automatizó PIN ni se cambió seguridad.
El posterior desbloqueo permitió continuar, pero no convierte ese probe en una
prueba controlada de lock/unlock. Process death **NOT TESTED**, fuera de alcance.

Detalles, muestras iniciales, checks y observaciones:
[android-recovery-results.json](android-recovery-results.json).

## Reconnect y smoke cruzado

**TESTED:** Android Host ↔ Linux Participant por LAN, Ready, Play, Pause y Seek
reales, antes y después del recovery. Android disconnect/reconnect pausado:
misma room/member/media 1/generación 13, hash completo/identidad válida y Ready,
posición restaurada a target 286519 ms; Linux en 286533 ms. Sin error. Reconnect
Linux durante Playing conserva render y vuelve a Ready. No prueba de diez minutos.

## Rendimiento y recursos

**MEASURED**, /proc, build debug, muestras 10 s, % de un núcleo:

| Estado | CPU | Observación |
| --- | --- | --- |
| Paused Host confirmado | 7.2% | SDK y timeline pausados antes de medir. |
| Playing normal | 30.1% | Render por eventos, vídeo sintético; no frames por Dart. |
| Playing fullscreen | 26.1% | Mismo pipeline. No conclusión estadística de diferencia. |

Un primer sample llamado idle era transición a Playing; se conserva excluido de
idle. Diez ciclos Lobby/Player: FD 60 constantes, threads 47–50; serie warm RSS
473832 → 486304 KiB, fluctuando en ciclos ~473324–482728 KiB. Primer calentamiento
con fullscreen/shaders creció ~26 MiB. Sin crecimiento claramente no acotado en
este ensayo; no prueba de ausencia total de fugas ni de cargas 4K. JSON completo:
[linux-results.json](linux-results.json).

## Estados de producto, accesibilidad e input

**IMPLEMENTED:** carga inicial/generación limpia, buffering discreto y reconnect
según snapshots reales. Error EGL/FBO/render/contexto muestra «No se pudo mostrar
el vídeo» y conserva controles visibles; detalle en Developer. Error render
inyectado **TESTED por widget**, no por pérdida real de GPU. Cambio de contexto
detectado: requiere reiniciar app; GPU loss físico **NOT TESTED**.

Play/Pause, tiempos, timeline y fullscreen con tooltip/focus/targets Material.
Scrub mantiene preview local y solo envía un seek al commit. Mouse/tap restauran
controles; no se ocultan durante scrub, modal, panel o error. Desktop tiene
participantes lateral, mobile bottom sheet. Sync usa texto semántico sin drift
permanente. Shortcuts Host Space/Left/Right y F/Esc; Participant sin autoridad.
Text scaling/regresiones responsive **TESTED**; auditoría WCAG completa **NOT TESTED**.
No capturas de buffering sostenido: **NOT VISUALLY VERIFIED** en esta sesión.

## Validación automatizada y CI

**TESTED / PASS local:** cargo fmt --all -- --check; clippy workspace/all-targets/
locked/-D warnings; test workspace --locked; build workspace --locked. Tests SDK
opt-in mpv (4), Client (2) y lease owner reales, además de checks ordinarios.
Test C++ puro con -Wall -Werror: resize inválido/límite/aspecto, generación vieja,
clear/reveal y recreate. No se presenta ese test como validación de frames.

**TESTED / PASS:** flutter analyze y 30 tests Flutter, incluyendo fullscreen/
Escape sin autoridad, scrub un seek, mensaje render error/cleanup y platform view
Android estable. Linux debug y APK armeabi-v7a debug compilados. Tests Flutter
de frontera necesitan CINE_BRIDGE_LIBRARY: una ejecución sin esa variable falló
por ruta SDK; se repitió con la configuración documentada y pasó completa.
check_docs.py, check_ci.py y 10 tests Python correspondientes PASS.

CI de implementación
[37717640777](https://github.com/Eirom16/cine-virtual/actions/runs/37717640777)
SUCCESS, incluyendo matriz multiplataforma. El informe final de la sesión verifica
también el run asociado al commit final de evidencia. CI hosted comprueba builds,
no Wayland/teléfono físico. Windows/macOS runtime **NOT TESTED**; iOS Player
**NOT IMPLEMENTED**, sin cambios de runtime multimedia en esas plataformas.

## Límites y continuación

Linux y Android cumplen el flujo objetivo en este entorno. Renderer sigue
**PROVISIONAL**: una GPU/compositor, software decode existente, copia GPU/glFinish,
sin ensayo de context-loss, HDR, video 4K ni multi-monitor. X11 no probado.
Android validado en API 28; no process death ni nuevo sistema de orientación.

El Player permite planificar la siguiente fase social, pero esta sesión no la
inicia. Antes de ampliar soporte del renderer, repetir smoke en otro compositor/
GPU y X11; probar system UI en Android moderno. No retocar thresholds de sync
para corregir problemas de presentación.
