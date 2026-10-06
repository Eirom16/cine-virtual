# Product UI Phase 1 — resultados

Estado: IMPLEMENTED, TESTED y VISUALLY VERIFIED localmente. Este documento
registra únicamente esta fase, no vuelve a medir ni ajustar la sincronización
larga del experimento 07.

## Base verificada

master limpio/sincronizado, HEAD b90fee2; gh autenticado y CI inicial
[37479639668](https://github.com/Eirom16/cine-virtual/actions/runs/37479639668) SUCCESS.
Inspeccionados README, arquitectura/protocolo/sync/media/testing/decisiones/CI,
UI Flutter completa, C ABI y Android. Arquitectura/auditoría en [UI](../../docs/UI.md).

## Implementación

IMPLEMENTED: shell, Home, Create/Join reales, invitación nativa/envuelta,
servidor avanzado, lobby responsive, presencia/Host/Ready reales, hash/progreso
real/cancelación, mismatch humano, Player/controles autoritativos, conexión,
errores, recuperación/lifecycle y Developer saneado. Android usa SAF y Media3;
Linux Client/libmpv existentes con vídeo en ventana nativa aparte.

No cambios a Core, protocolo, SyncEngine, thresholds, predictor o scheduler.
PROVISIONAL: runtimes multimedia anteriores. NOT IMPLEMENTED: embedding Linux,
fullscreen del sistema, light theme, discovery. iOS Player NOT IMPLEMENTED;
Windows/macOS runtime NOT TESTED/deshabilitado en producto.

## Pruebas locales

| Gate local | Resultado |
| --- | --- |
| cargo fmt --all -- --check | PASS |
| cargo clippy --workspace --all-targets --locked -- -D warnings | PASS |
| cargo test --workspace --locked | PASS; 6 SDK opt-in omitidos en este comando |
| cargo build --workspace --locked | PASS |
| python3 scripts/check_docs.py / check_ci.py | PASS |
| unittest scripts/tests | PASS, 10 tests |
| flutter analyze | PASS, sin issues |
| flutter test con CINE_BRIDGE_LIBRARY | PASS, 26 tests (20 de producto + 6 previos) |
| SDK libmpv + réplica, --ignored --test-threads=1 | PASS, 4 + 2 tests reales |
| Linux debug / APK debug armv7 + arm64 | PASS |
| Bridge Android release armv7 / arm64 | PASS |

Los tests SDK deben ejecutarse en serie según TESTING.md. Una primera invocación
paralela falló en el contador compartido de recursos; la invocación documentada
pasó, sin modificar tests ni Player.
Widget fake solo en tests; producto usa bridge/servidor reales. Se añadió test
C ABI de DTO desktop/límites de ownership. Posición: 20 snapshots de 500 ms
no reconstruyen sala en test (no es benchmark físico).

## Smoke físico y screenshots

Corpus sintético, servidor LAN aislado de las pruebas personales. Sin WebSocket
por adb reverse. ADB forward solo para leer observaciones de VM en build debug;
intents se accionan con los formularios/controles reales.

Linux: 1366×768 y 1920×1080 en Xvfb para capturas de Flutter; libmpv real
capturado en display físico con ventana propia identificada por PID.
Android: SM-J701M/API28/armeabi-v7a 720×1280 y SM-A146U/API35/arm64-v8a
1080×2408; portrait y landscape según tabla final. Sin seriales/credenciales.

La captura inicial mostró Home antigua demasiado larga y vídeo portrait
estirado. Se compactó Home móvil y se ajustó geometría nativa con letterbox.
La QA táctil/automatización no permitió acceder fiablemente a los controles
superpuestos a SurfaceView. Navegación y restauración de controles pasan a
regiones Flutter fuera de la superficie; restauración y botones se comprobaron
al tacto. UIAutomator también espera idle mientras cambia la posición: no se
atribuye cada timeout de automatización a un fallo del producto.

Las imágenes son capturas reales del cliente, sin mocks ni retoque. Solo
metadata del corpus sintético. El vídeo pequeño/pixelado es el corpus de
160×90, no un defecto de resolución del Player.

## Limitaciones y deuda

El lobby móvil usa scroll para Ready/participantes. Cancel durante selección
nativa corresponde al picker; cancelación de hash utiliza API real. No cancel
falso de operaciones que el backend no expone. Cancelación de hash expuesta
solo en Android; desktop no expone cancelación de su LocalMedia. Resume manual con Reintentar,
sin bucle de reconexión paralelo en Dart. Startup/recovery anteriores siguen
pendientes; no se promete precisión a partir del indicador «Conexión lista».

Xvfb carece de DRI3 para la ventana GPU de libmpv: se verifica vídeo en display
físico, separado de las capturas del shell. Windows/macOS/iOS se validan como
build en CI; no se atribuye smoke multimedia remoto.


## Recorridos realmente ejecutados

| Escenario | Resultado y alcance |
| --- | --- |
| Linux Host, Android API28 Participant | TESTED: Create/Join, SAF, mismo archivo, Ready, Start y Pause; vídeo Android real. |
| Android API28 Host, APK final | TESTED: Create, SAF, Ready, Play/Pause/Seek táctiles; portrait 720×1280 y landscape 1280×720, proporción correcta. |
| Android API35 Host | TESTED: Home/Create, SAF, hash, Ready, Start, seek táctil, Play/Pause; vídeo con proporción correcta. |
| Linux físico Host + Android API35 + Linux Participant | TESTED: tres clientes Ready, Play, Pause y seek +10 s. Vídeo libmpv físico y Media3; Participant sin controles de autoridad. |
| Android Participant disconnect/retry | TESTED: socket cerrado desde Developer; lobby/sesión conservados; Reintentar recuperó snapshot, posición y Ready. |
| Android Host background/foreground | TESTED: conserva sala, clock/Ready se recuperan. PREEXISTING BUG: revalidación del Host reinicia posición. |
| Invite clipboard | TESTED: clipboard coincide con invitación real del controller; sin publicarla. |
| Estados de error/hash/cancel/autoridad/responsive | TESTED en widgets; no se atribuyen inyecciones fake a smoke físico. |

[Observaciones saneadas](observations.json) contienen únicamente estados y
posiciones después de acciones de UI. Son lecturas secuenciales: diferencias
entre posiciones durante Play no son mediciones de drift ni p95. Pause quedó
estable; seek compartido terminó en 181733 ms en los tres clientes. No se
repitió el ensayo de 600 s ni se recalibraron thresholds.

PREEXISTING BUG confirmado en código anterior: RoomScreen.recover ya llamaba
hashAndAttach; Client::attach_media envía MEDIA_SELECT_REQUEST para Host, incluso
cuando revalida el mismo archivo. Background a 497827 ms volvió a Ready en 0 ms.
No se corrige aquí: cambiar revalidación/selección autoritativa requiere revisar
la frontera Client/media. El resume de socket del Participant conservó 84267 ms.

El endpoint compartido debe ser alcanzable desde los otros dispositivos. Para
el smoke que creó con localhost se usó la invitación nativa y la conexión LAN
en Advanced. No hay traducción automática de localhost ni discovery.

## Galería y revisión visual

| Captura | Resolución / revisión |
| --- | --- |
| [Home desktop](screenshots/linux-home-1366.png) | 1366×768, VISUALLY VERIFIED |
| [Home amplia](screenshots/linux-home-1920.png) | 1920×1080, VISUALLY VERIFIED |
| [Create](screenshots/linux-create-1366.png) | 1366×768, VISUALLY VERIFIED |
| [Join](screenshots/linux-join-physical.png) | 1366×700, VISUALLY VERIFIED |
| [Lobby sin medio](screenshots/linux-lobby-empty-1366.png) | 1366×768, VISUALLY VERIFIED |
| [Lobby Ready](screenshots/linux-lobby-ready-1366.png) | 1366×768, VISUALLY VERIFIED |
| [Player Participant desktop](screenshots/linux-player-1366.png) | 1366×768, VISUALLY VERIFIED |
| [Player Host físico](screenshots/linux-player-physical.png) | 1280×700, VISUALLY VERIFIED |
| [Vídeo libmpv](screenshots/linux-native-video.png) | 960×540, VISUALLY VERIFIED; ventana nativa independiente |
| [Android API28 Home](screenshots/android-home-portrait.png) | 720×1280, VISUALLY VERIFIED |
| [Android API28 Lobby](screenshots/android-lobby-portrait.png) | 720×1280, VISUALLY VERIFIED; Ready requiere scroll |
| [Android API28 Player portrait](screenshots/android-old-player-portrait.png) | 720×1280, VISUALLY VERIFIED; APK final |
| [Android API28 Player landscape](screenshots/android-old-player-landscape.png) | 1280×720, VISUALLY VERIFIED; APK final |
| [Android API35 Home](screenshots/android-modern-home-portrait.png) | 1080×2408, VISUALLY VERIFIED |
| [Android API35 Lobby Ready](screenshots/android-modern-lobby-ready-portrait.png) | 1080×2408, VISUALLY VERIFIED |
| [Android Host Player portrait](screenshots/android-modern-player-portrait.png) | 1080×2408, VISUALLY VERIFIED |
| [Android Host Player landscape](screenshots/android-modern-player-landscape.png) | 2408×1080, VISUALLY VERIFIED |
| [Android Participant portrait](screenshots/android-participant-player-portrait.png) | 1080×2408, VISUALLY VERIFIED |
| [Android Participant landscape](screenshots/android-participant-player-landscape.png) | 2408×1080, VISUALLY VERIFIED; controles deshabilitados |
| [Android desconectado](screenshots/android-reconnect-portrait.png) | 1080×2408, VISUALLY VERIFIED; conserva lobby |

La revisión produjo iteraciones reales: Home compacta para móvil pequeño,
invite oculto, proporción de vídeo, regiones táctiles fuera de SurfaceView y
restauración de controles. Se descartaron capturas tomadas antes de terminar
una transición y la captura de vídeo anterior a corregir el estiramiento.
Sin clipping/overflow en las capturas finales inspeccionadas. Tests adicionales
cubren 360×640 con texto 150% y Player 640×360. No auditoría WCAG exhaustiva,
fullscreen del sistema ni rendimiento GPU/CPU medido.

## CI y trazabilidad

El código de producto termina en fd75d62; documentación/evidencia se publican en
commits separados. La matriz del commit que publica esta evidencia se consulta
en [CI/master](https://github.com/Eirom16/cine-virtual/actions/workflows/ci.yml?query=branch%3Amaster).
Se comprueba con gh run list/watch/view antes del informe final. Las conclusiones
hosted se distinguen del smoke físico; compilar no certifica multimedia.

Comprobación adicional tras la pausa: API28 con ambos bridges actualizados y
APK final pasó Create/SAF/Ready/Play/Pause/Seek. Pause estable en 31949 ms y seek
en 403939 ms. UIAutomator devolvía root nulo en esa sesión; toques por coordenadas
funcionaron y las observaciones reales confirmaron las acciones. No se afirma
validación TalkBack a partir de la automatización. Se restauró la orientación
del dispositivo después de capturar landscape.
