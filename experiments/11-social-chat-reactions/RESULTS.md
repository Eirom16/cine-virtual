# Social Experience Phase 1 — resultados

IMPLEMENTED, TESTED y VISUALLY VERIFIED: chat de sala y reacciones sobre la
sesión WebSocket existente, Linux/libmpv embebido ↔ Samsung Android/Media3.
La evidencia de runtime está separada de los renders de widgets y de los builds.

## Base y arquitectura

Base: master limpio/sincronizado, `318272e`, CI inicial `37719928797` SUCCESS.
Rust conserva protocolo, membership, identidad, autoridad, reconnect y estado.
Flutter proyecta snapshots y controla composición, drafts, unread, scroll y
animaciones. Kotlin conserva SAF/Media3. Sin cambios a SyncEngine/Core/timeline,
segunda conexión, servidor de chat ni persistencia durable.

Protocol v1 se mantiene mediante capability opt-in `social_v1` en HELLO/ACCEPT.
Servidor nuevo no entrega eventos sociales a clientes antiguos. Cliente nuevo
sin capability aceptada deshabilita social. CHAT_SEND y REACTION_SEND son intents;
CHAT_MESSAGE, SOCIAL_STATE y REACTION son eventos autoritativos en el mismo socket.

Chat/presencia usan secuencia social independiente. UUIDv4 autoritativo,
member y nombre derivados del binding; snapshot de nombre preserva el histórico
tras leave. Timestamp del reloj de servidor, nunca reloj del emisor. Se conserva
el reloj monotónico existente; no se presenta como fecha civil ni depende del
clock sync multimedia.

## Retención, validación y recuperación

Historial de chat + presencia: máximo 100 entradas y presupuesto JSON conservador
48 KiB, incluyendo escape de texto/nombre y allowance de metadata. No equivale a
bytes exactos de heap; con 16 miembros, las cuotas/mapas siguen acotados. Payload
máximo de protocolo 64 KiB. Mensaje: 2048 bytes UTF-8 antes de trim, vacío rechazado,
hasta 9 líneas, controles rechazados salvo LF/tab. Unicode y emoji preservados;
SelectableText de texto plano. No HTML ni Markdown ejecutable.

Snapshot social privado al entrar/resume/SYNC_REQUEST; reemplazo acotado y dedup
por scope/sequence. Gaps solicitan el mecanismo existente de snapshot. IDs estables,
cache idempotente 120 s por epoch/member/event_id, conflicto de payload rechazado.
No bubble optimista: solo echo confirmado. Draft se conserva al fallar; pending
finito, sin reintento infinito. Bucket chat 5 + 1/2 s; reacción 8 + 1/500 ms;
cuotas por miembro conservadas durante resume.

Presence joined/left/resumed comparte buffer, sin mensajes de playback ni pérdidas
breves de red. Resume visible coalescido 30 s por miembro. Estados de participants
existentes, sin away/busy inventados. Reacciones allowlist ❤️ 😂 😮 😢 🔥 👏:
sin historia/replay; cliente máximo 32 durante 3 s, overlay máximo 12 durante 2,2 s,
5 lanes deterministas. Animación fade/scale/subida, sin anuncios individuales al
screen reader. Exceso visual se descarta sin bloquear protocolo.

## Pruebas automatizadas

TESTED: cargo fmt, clippy workspace/all-targets/locked -D warnings, test/build
workspace locked. 111 tests passed, 8 opt-in ignored por defecto. Opt-in Linux:
4 libmpv SDK, 2 client SDK, 1 lease de vídeo y 1 nuevo servidor con dos decoders
reales; todos PASS. Este último cubre hash/Ready/Play/Pause/Seek, chat, reacción,
desconexión/resume e historial; no afirma frames ni Android.

TESTED: Flutter analyze sin incidencias y 42 widget tests PASS, preservando los
anteriores. Cobertura social: autores/sistema, cerrado/abierto, input/Enter/Shift,
errors/draft, unread, scroll/indicador, picker, overlay/cleanup, foco, copy/paste y
cursor sin atajos multimedia; notificaciones durante layout aplazadas al final
del frame. Se comprueba que social no notifica estructura de Room/Player ni
expone texto en diagnostics. Cinco viewports, incluyendo teclado.

TESTED: check_docs/check_ci y 10 tests Python PASS. Codec/RoomService/client/server
cubren spoof payload/envelope, Unicode/UTF-8 inválido en JSON, tamaño, whitespace,
allowlist, epoch/room incorrectos, sequence indebida, dedup y retry después de
ACK/socketclose/resume. Cliente antiguo opt-out no recibe social. RoomService
mantiene serialización de efectos y WebSocket await fuera del mutex.

## Dos dispositivos físicos

TESTED: misma sala, mismo archivo sintético 620 s seleccionado por GTK/SAF, hash
coincidente y Ready en ambos. Chat funciona en Lobby y Player: Linux envía
«¿Viste eso? 😂», Android recibe y responde «Si jajaja», Linux recibe; conversación
idéntica y deduplicada. Linux ❤️ visible en Android y Android 😂 visible en Linux.
Pause/Play/Seek mantienen chat. Fullscreen Linux y landscape/fullscreen Android
no desmontan la sesión al abrir social ni al reaccionar.

TESTED: Android desconectado, Host envía, resume conserva member y IDs anteriores,
recupera el mensaje y no duplica. Background/Home, mensaje desde Host, foreground
recupera chat y reproducción; reacción perdida no reaparece. Se repitió intercambio
con APK recién instalado y Linux arrancado limpio. La protección final de
publicación también se cargó en ambos procesos mediante hot reload controlado.
Datos sanitizados en [results-physical.json](results-physical.json).

TESTED: cuotas exactas con ráfagas deterministas Rust; réplica con 500 mensajes y
1000 reacciones. La carga física final usa 50 intentos mediante pulsaciones reales
del picker, sin invocar métodos por VM. Los intentos iniciales mediante VM quedan
PROVISIONAL y excluidos de la evaluación de fluidez: esa herramienta interfería
con los timers del debug runtime. No se incluye como herramienta del proyecto.

## Recursos y revisión visual

MEASURED: [results-performance.json](results-performance.json) contiene muestras
/proc de Linux debug con vídeo real, chat cerrado/abierto y ráfaga. CPU expresada
como porcentaje de un core; el caso burst usa el picker real y no aísla el coste de animación. Las muestras PSS Android anteriores se excluyen por el artefacto de debug. No se recalibra p95 ni se afirma ausencia de leaks
por una muestra corta. Linux: CPU de un core 26,98% normal y 28,22% con chat (8,04 s cada uno);
125,16% durante 50 intentos de picker (19,70 s). Cinco RATE_LIMITED observados,
room sequence/generación sin cambios, playback avanzando, FD 61 estable y cola
expirada a cero. RSS del burst 542940 → 576272 KiB, pico 580932 KiB. No es una
comparación release ni demuestra ausencia de jank/leaks; no se contaron intents
suprimidos por pending/animación del picker. TESTED: 500 mensajes/1000 reacciones sintéticas mantienen
los límites; timers/controladores se liberan y cola de reacción vuelve a cero.

VISUALLY VERIFIED: [capturas y metodología](screenshots/README.md). Iteración real:
header estrecho, espacio del teclado landscape y clipping de radios corregidos.
Vídeo protagonista, chat lateral compacto, sheet móvil, input y wrapping legibles,
reacciones fuera del timeline. Renders widget etiquetados de forma independiente.

## Bugs corregidos y límites

Corregidos: snapshot social inicial antes del mensaje, colisión de IDs entre
miembros con igual request ID, presupuesto Unicode excesivamente conservador,
overflow de header/teclado, shortcuts que bloqueaban paste/cursor, y publicación
durante frame al invocar intents desde herramientas de debug. El último requería
aplazar todas las proyecciones, incluida playback, no solo social. Las mediciones mediante invocaciones VM se descartaron y se repitieron con
pulsaciones reales, conservando solo lecturas de estado para observación.

PROVISIONAL: comparación CPU/memoria de build debug, muestra corta e instrumentada.
NOT TESTED: soak de horas, performance release y dispositivos Android adicionales.
NOT IMPLEMENTED: persistencia permanente, mensajes antiguos más allá del buffer,
optimistic bubbles/retry automático, GIF/stickers/uploads, cuentas, voz/cámara/
WebRTC y runtime iOS AVPlayer. Windows/macOS/iOS: build CI, no runtime multimedia
nuevo afirmado. La capa admite extensiones futuras, pero GIF y voz/video necesitan
sus decisiones y fases independientes; no están aprobados por este resultado.

## CI y Git

La matriz final y sincronización se verifican después del push de esta evidencia.
El informe final registra run, commit y resultado exactos; este archivo no convierte
una matriz anterior en prueba del HEAD final. CI publicado previo: `37792473530`
SUCCESS en Linux, Windows, macOS arm64/x64, Android 3 ABI e iOS device/simulator.
