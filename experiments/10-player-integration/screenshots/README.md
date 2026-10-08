# Capturas reales — Phase 2

Corpus sintético, sesiones reales; sin mockups ni imágenes generadas. Dimensiones
en píxeles de la captura, no logical pixels. En Linux la decoración GTK cuenta
en modo ventana. Wayland probado en KWin 6.7.5; un monitor físico 1366×768.

| Captura | Tamaño | Estado / alcance |
| --- | --- | --- |
| [Linux Host normal](linux-host-normal-1366.png) | 1366×768 | Prueba principal: vídeo libmpv dentro de Cine Virtual, controles Host Flutter activos, paused ~4:35. |
| [Linux Host fullscreen](linux-host-fullscreen.png) | 1366×768 | Sin decoración, controles/timeline Flutter sobre vídeo; misma timeline. |
| [Linux normal 1920](linux-normal-1920.png) | 1920×1080 | Participant y controles visibles. Ventana Wayland sobredimensionada real, monitor físico 1366×768; no imagen escalada ni segundo monitor. |
| [Linux fullscreen](linux-fullscreen-1366.png) | 1366×768 | Participant, controles visibles, sin chrome nativo. |
| [Linux normal 1366](linux-normal-1366.png) | 1366×768 | Player Participant final con controles visibles tras integración de fade; posición 4:46. |
| [Linux final controles ocultos](linux-final-controls-hidden.png) | 1366×768 | Mismo frame y proporción, controles ocultos tras tap; biblioteca Dart final. |
| [Linux controles ocultos](linux-controls-hidden.png) | 1366×700 | Vídeo integrado en modo ventana, sin controles encima. |
| [Linux participantes](linux-participants.png) | 1366×700 | Panel lateral Flutter y vídeo real; puede reducir región de vídeo. |
| [Linux primer embedding](linux-first-embedded.png) | 1366×700 | Primer frame integrado durante la implementación; evidencia intermedia. |
| [Spike recreate](linux-spike-recreated.png) | 1366×768 | Spike aislado tras destroy/recreate; no sesión de producto. |
| [Spike resize](linux-spike-resize.png) | 900×600 | Captura intermedia de investigación de resize, anterior al envío explícito de tamaño físico; no QA final de aspect ratio. |
| [Android portrait](android-portrait.png) | 720×1280 | Host tras cinco recoveries, Media3 visible ~4:46 y controles. |
| [Android fullscreen portrait](android-fullscreen-portrait.png) | 720×1280 | Immersive, sin barra de estado; captura contiene transición de repintado de texto de timeline. Para lectura estable usar landscape/salida. |
| [Android fullscreen landscape](android-fullscreen-landscape.png) | 1280×720 | Vídeo contenido, controles y fullscreen reales, posición conservada. |
| [Android participantes](android-participants.png) | 1280×720 | Bottom sheet Flutter encima de la SurfaceView, sin chat. |
| [Android salida fullscreen](android-exit-fullscreen.png) | 720×1280 | Fix final probado por hot reload: barra de estado restaurada en API 28, vídeo y timeline conservados. |

El fichero sintético tiene resolución baja, por eso se pixela al ampliar. El
renderer mantiene proporción; overlays visibles pueden cubrir parte de la imagen.
No captura sostenida de buffering, pérdida GPU ni segundo compositor. Las capturas
no demuestran por sí solas Play/Pause/Seek; esas observaciones están en los JSON
y [RESULTS](../RESULTS.md).
