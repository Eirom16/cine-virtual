# Product UI — Phases 1–2

## Estado y alcance

IMPLEMENTED: Home → Create/Join → Lobby → archivo local → hash/verificación →
Ready → Player → Play/Pause/Seek, conectado a la Application real. El cliente
Flutter permanece en `experiments/02-rust-ui-bridge/app`; no se mueve el proyecto
nativo ni se sustituye el Core. Evidencia y limitaciones en
[experimento 09](../experiments/09-product-ui/RESULTS.md).

Android presenta la SurfaceView de Media3. Linux integra libmpv Render API en
una textura Flutter, sin segunda ventana mpv (ADR-011). Media3 y
libmpv continúan PROVISIONAL. iOS Player NOT IMPLEMENTED; Windows/macOS runtime
NOT TESTED y selección/reproducción deshabilitadas. Compilar no demuestra runtime.

## Auditoría de la UI anterior

| Elemento | Clasificación | Tratamiento |
| --- | --- | --- |
| CineBridge, C ABI, snapshots y errores seguros | REUSABLE | Misma frontera Application; DTO de presentación de solo lectura. |
| Media3, SAF, fd para hashing, SurfaceView y lifecycle | REUSABLE | Reutilizados; geometría de superficie conserva proporción. |
| Polling/snapshots del RoomScreen | NEEDS REFACTOR | Controller pequeño; progreso/posición separados de estructura. |
| Formularios, telemetría y botones juntos | NEEDS REFACTOR | Flujos de producto y diálogo Developer separados. |
| SpikeScreen y RoomScreen con autorun de experimentos | ENGINEERING ONLY | Accesibles por SPIKE_AUTORUN/ROOM_AUTORUN; se conservan las regresiones. |
| Entrada normal a pantalla de ingeniería | OBSOLETE | Sustituida por CineVirtualApp; una sola UI normal activa. |

## Fuente de verdad y responsabilidades

Rust conserva autoridad, protocolo v1, réplica, room state, generation, identidad
SHA-256, Ready, reloj, scheduler, SyncEngine y reconnect/snapshot. El nuevo DTO
`presentation_summary` proyecta StateDto y member_id; no transporta credenciales.
El owner desktop usa el Client<BackendPlayer> existente y su LocalMedia, no una
implementación alternativa de WebSocket o sincronización.

Kotlin conserva SDK Media3, Looper, observaciones, driver, URI SAF y permisos.
Flutter envía intents y presenta snapshots. `SessionGateway` permite sustituir
la frontera únicamente en tests. `ApplicationController` serializa operaciones,
polling y lifecycle. `RoomView`/`PlaybackView` son proyecciones; sus predicados
limitan botones, pero la autoridad final sigue en Rust/servidor.

No se modificaron algoritmos, thresholds, predictor, scheduler ni protocolo.
No hay cuentas, providers, transferencia de medios ni datos ficticios.
La extensión social se describe al final de este documento.

## Navegación y flujo

Navigator y estado local del shell: Home, Create, Join, Lobby y Player. Lobby y
Player requieren sesión real. Start envía Play existente; el participante abre
Player al observar reproducción real y Ready. Volver al lobby es deliberado y
no fuerza una nueva transición automática. Salir confirma cierre si eres Host.
Perder conexión conserva sesión, lobby y archivo; Reintentar invoca resume real.
No existe un segundo protocolo de start-room ni una segunda máquina de sala.

Invite es el JSON real con room_id, room_epoch e invite_token, o su envoltorio
con servidor. No se inventan códigos cortos. El campo se oculta y Copiar
invitación comparte el envoltorio por clipboard. Es una credencial: no aparece
en screenshots, logs ni Developer. No hay discovery. CINE_SERVER configura un
default; Linux usa localhost y Android necesita servidor avanzado al crear si
no existe default. Join puede tomar el servidor de la invitación.

## Sistema visual y responsive

`ui/theme/product_theme.dart` centraliza colores, espaciado, radios, typography,
elevación, duración y breakpoints. Dark Material 3, fondos profundos, acento
violeta, verde para Ready/salud, ámbar para espera y rojo para errores. Fuentes
del sistema; no fonts remotas. Light NOT IMPLEMENTED.

Breakpoint desktop 900 logical pixels; contenido máximo 1200. Home centrada,
acciones en fila o columna compacta. Lobby comparte componentes: columna en
móvil, media más participantes lateral en desktop. SafeArea y scroll para
viewports pequeños. Player oculta app bar en landscape, utiliza vídeo ajustado
con letterbox, controles táctiles separados de la superficie y participantes
en panel desktop/bottom sheet móvil. Fullscreen Linux utiliza GTK; Android
immersiveSticky, layout adaptativo y SafeArea, sin locking de orientación.
Salir restaura chrome/system UI sin recrear la sesión o el Player. En API 28 se
restauran ambas barras explícitamente con modo manual: edgeToEdge no se aplica
en esa versión. Android 15/16 y sus restricciones de system UI NOT TESTED.

## Estados, errores y recuperación

Hash muestra bytes/progreso real y cancelación real. Ready distingue archivo
pendiente, hash, carga, mismatch, reloj, verificación, cancelación, error y Ready
solo cuando el snapshot permite hacerlo. Cancelar hash no habilita Ready.
Mismatch explica que hay que elegir la misma versión del archivo. El Host puede
Start cuando todos los miembros conectados están Ready; controles del
Participant y sus shortcuts autoritativos están deshabilitados.

Errores de conexión, sala, media, player, permiso y protocolo tienen mensaje
humano/acción y código técnico seguro en Developer. Estado global de conexión,
botón ocupado, estados inline y progreso evitan spinner para todo. El indicador
«Conexión lista» deriva de reloj confiable/Ready y ausencia de buffering; no
promete una precisión de sincronización ni muestra drift permanentemente.

Android foreground invoca recuperación existente, nueva observación/archivo y
Ready cuando corresponde; no crea una sala ni reinicia el Core por su cuenta.
Resume disponible por Reintentar; auto-retry con backoff en Flutter NOT IMPLEMENTED.

## Diagnósticos, accesibilidad y rendimiento

Developer abre un diálogo secundario con allowlist: endpoint saneado, IDs,
epoch, sequence, generation, reloj, player, drift, código seguro y progreso.
Se omiten token de invite/resume, URI/path, SHA completo y secretos de endpoint.
Los autoruns antiguos siguen disponibles explícitamente para scripts.

Material aporta focus/touch targets; tooltips/semántica, etiquetas, estados con
texto además de color y tests de text scaling. Space Play/Pause y flechas ±10 s
solo Host; F/doble click Linux alternan fullscreen, Esc sale o cierra panel. Mouse/tap muestran controles y
fade discreto tras inactividad. Scrubbing suspende el ocultamiento. Auditoría
WCAG completa NOT TESTED.

Polling Flutter 500 ms (2 Hz), publicación desktop 100 ms. Position/hash y
Developer tienen ValueNotifiers propios. La clave de estructura excluye
posición/drift/progreso: avanzar 20 snapshots no notifica al árbol de sala en
el test. No se interpola posición precisa ni se reconstruye el shell a 20 Hz.

## Validación

Widget tests usan estado de presentación fake, nunca backend falso en el
producto. Cubren formularios, autoridad, controles, Ready, mismatch, errores,
reconnect, cancelación, redacción y responsive. Ver instrucciones y resultados
reales en [experimento 09](../experiments/09-product-ui/README.md) y
[TESTING](TESTING.md). Las pruebas largas previas de sync no se repiten ni se
reinterpretan como medición de esta UI.

## Deudas observadas en el smoke

Phase 2 reprodujo físicamente el bug preexistente de recovery Host: attach
volvía a seleccionar el medio, fallaba INVALID_STATE mientras Playing y dejaba
Media3 en 0. Recovery ahora usa el intent local revalidate, valida hash/identidad
y después Ready; no publica MEDIA_SELECT_REQUEST. La selección explícita sigue
usando attach. La reconciliación existente restaura la timeline autoritativa.
La evidencia anterior de Phase 1 permanece en experimento 09; resultados
actuales, capturas reales y límites en [experimento 10](../experiments/10-player-integration/RESULTS.md).
Para compartir entre dispositivos, configurar al crear un endpoint alcanzable
por todos; localhost es solo para uso en el mismo dispositivo. El invite no
inventa discovery ni transforma direcciones automáticamente.


## Presentación Linux

Render nativo por eventos de libmpv → FBO EGL compartido → copia GPU a
FlTextureGL → Texture Flutter. Los frames no pasan por Dart ni reconstruyen
widgets. Controles/header Flutter se componen encima; participants puede reducir
la región de vídeo. `DesktopVideo` comunica tamaño físico, observa errores a 1 Hz
y muestra un mensaje de producto ante fallo. Developer incluye estado del render,
backend, dimensiones, generación de presentación y frames.

El surface conserva textura al navegar y descarta frames durante cambio de medio.
Fullscreen no cambia generación de media. Scrubbing previsualiza localmente y
solo envía Seek al terminar. No auto-hide durante scrub, panel, modal o error.
El contexto EGL perdido requiere reiniciar la app; no se afirma recovery GPU.


Android conserva la misma platform view durante auto-hide/show de controles;
la región Expanded tiene key estable. El test regresivo verifica una creación y
un dispose al salir, sin adjudicarle validación física de frames. SafeArea y
layout de controles permanecen fuera de SurfaceView para recibir touch.

## Social Experience Phase 1

IMPLEMENTED: conversación compartida en Lobby/Player, texto plano SelectableText,
autores históricos con nombre autoritativo y distinción «tú»/otros/sistema.
Panel desktop 300 px (fullscreen overlay 320 px), tabs Chat/Participantes.
Lobby desktop muestra la misma conversación; móvil abre bottom sheet SafeArea,
con inset del teclado. Player no se comprime con teclado; superficie continúa
montada. Fullscreen desktop abre overlay conservando región de vídeo; móvil
sheet sobre Player. Reaccionar no cambia fullscreen ni playback.

Enter envía en desktop, Shift+Enter permite nueva línea, IME móvil normal y
composición activa no se envía. El editor consume shortcuts Space/flechas/F;
Esc cierra panel antes de salir fullscreen. Límite 2 KiB y reglas de texto se
validan localmente y en servidor. UI espera eco autoritativo: nunca añade burbuja
optimista. Pending solo en botón, timeout/error conserva draft para retry manual.
RATE_LIMITED/error humano inline; error de reacción mediante snackbar.

Unread local cuenta chats ajenos cuando conversación está cerrada; abrir Chat
marca vistos. Al leer arriba no se fuerza scroll; aparece «Nuevos mensajes».
Si está al final se sigue la lista. Draft y conversación sobreviven Lobby/Player;
salir/cambiar epoch limpia draft/unread. No persistencia local durable.

Picker pequeño ❤️ 😂 😮 😢 🔥 👏 con nombres semánticos. ReactionOverlay aislado:
12 simultáneas, cinco lanes deterministas, subida 70 px, scale/fade durante 2,2 s;
exceso se descarta visualmente. Posición sobre región de vídeo, lejos de timeline.
IgnorePointer/ExcludeSemantics evita tap interception y anuncios caóticos de
cada emoji a screen readers. No notificaciones del SO. Indicadores de presencia
usan connected/Ready/Host existentes; no away/busy/online inventados.

Developer muestra count/budget/sequence/dropped reactions/pending/rate limit/error,
sin conversación, tokens, paths, URI o hashes. Cadencia UI sigue 2 Hz; latencia
visual puede incluir hasta 500 ms de polling. No se declara entrega instantánea.
