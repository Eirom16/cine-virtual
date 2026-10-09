# Capturas y revisión visual — P2P Phase 1

VISUALLY VERIFIED: capturas del app real, no render de widget fixture. Android
720×1280 físico y Linux1280×700. Los widget tests además ejercitan360×640 y
1280×720. Linux Host usa bootstrap QA y source fixture seleccionada mediante la
acción real; el portal GTK/Wayland no fue automatizable con XTest. Esa captura
no demuestra el chooser nativo Linux. Receptor Linux físico usa CLI/libmpv.

| Captura | Observación |
| --- | --- |
| linux-host-available | Medio verificado, disponibilidad y Lobby/chat conservados |
| linux-share-consent | Derecho de distribución explícito; ofrecer deshabilitado sin checkbox |
| linux-host-request | Solicitud por participante, autorización/rechazo visible |
| linux-host-progress | Bytes reales por peer sin reorganizar chat |
| android-offer | Alternativa recibir/copia propia; no descarga automática |
| android-consent | Tamaño,64MiB de margen, datos y almacenamiento privado explícitos |
| android-progress | Porcentaje proporcional; track contrastado y ETA indicada como estimada |
| android-paused | Datos conservados, Resume/Cancel, sin velocidad/ETA falsa |
| android-connection-interrupted | Pérdida real de Wi-Fi, parcial conservado y estado coherente |
| android-verifying | 100% de bloques, SHA en curso, Ready deshabilitado |
| android-completed / android-loaded | Transfer completo separado del hash/load del pipeline existente |
| android-ready | Identidad/media validada antes de Ready |
| android-player / android-fullscreen | Media3 real, mismo Player y fullscreen |
| android-host-saf-loaded / android-host-offered | Host Android SAF/offer, no transferencia saliente simulada |
| android-host-player | Película del Host seleccionada por SAF, ambos Players reales |

Las imágenes están en [screenshots](screenshots). Se inspeccionaron progreso,
pausa, consentimiento, Host/request, Ready, verificación y Player/fullscreen.
Sin overflow observado en las capturas; los paneles largos requieren scroll en
móvil/ventanas bajas, manteniendo acciones accesibles. No aparece una ETA durante
verificación/pausa. El botón Cancel identifica que elimina el parcial.

Se corrigió un track que parecía completo cuando el avance era0.4%; ahora
backgroundColor explícito separa bytes pendientes. La captura de transferencia
completa puede mostrar el hash LocalMedia aún activo: es deliberado y Ready sigue
inhibido hasta player/identity/clock. No se confunde descargado con reproducible.

NOT VISUALLY VERIFIED: recepción Flutter Linux física, providers SAF alternativos,
falta real de espacio/errores de disco, Windows/macOS/iOS. Tests de widgets y
validación de límites no sustituyen esas observaciones. Las capturas anteriores
fallidas del harness no se usan como evidencia de un flujo completo.
