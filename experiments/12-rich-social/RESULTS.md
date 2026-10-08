# Social Experience Phase 2 — resultados

Fecha: 2026-10-08. Base: master `a771497`. Informe completo:
[INFORME — GIFs & Rich Chat](REPORT.md). Investigación:
[proveedores](provider-evaluation.md). Decisión: ADR-013 en
[DECISIONS](../../docs/DECISIONS.md).

## Resultado

IMPLEMENTED / TESTED: mensajes GIF con descriptor neutral, eco autoritativo,
capability `rich_social_v1`, fallback legacy, historial/replay/dedup compartido,
reply, toggle/contadores de reacciones a mensajes, Copy, hora civil opcional,
picker responsive, placeholders, reduced motion y cache temporal acotado.

BLOCKED: GIPHY en producto por incompatibilidad de cache sin aprobación y ausencia
de credencial/integración aprobada. El usuario eligió mantenerlo bloqueado y
continuar con fixtures. REAL PROVIDER SEARCH NOT TESTED. No se distribuye key.
El adapter de API real se verifica con respuestas HTTP sintéticas; no implica
permiso comercial, búsqueda física ni descarga de medios GIPHY implementada.

TESTED / VISUALLY VERIFIED: Linux KDE Wayland/libmpv ↔ Samsung SM-J701M API28,
Media3, Wi-Fi LAN. Builds debug; las últimas correcciones Dart se aplicaron con
hot reload conservando la sesión, sin hot restart. Archivo local sintético 620 s;
no se repitió benchmark de sincronización de 600 s. Fixture GIF propio 160×100,
8 frames/7044 bytes, tres círculos y texto Cine Virtual. No contenido personal.

Escenario físico: ambos Ready y reproduciendo; Linux envía «¿Viste esa escena?»;
Android selecciona GIF; Linux lo recibe y responde «JAJAJA» al ID del GIF;
Android añade ❤️; Linux muestra ❤️ 1. Ambos envían floating 😂. Copy de JAJAJA
verificado contra clipboard. Pause, Play, Seek y fullscreen usados físicamente.
Chat/picker/reply/reacciones conservan el vídeo. Android portrait/landscape.

Background: con reproducción activa y chat con GIFs, Home → Linux envía texto y
GIF → foreground → recuperación media/Ready existente completa. Dos entradas
nuevas, IDs únicos, mismo membership, posición pasa de 502302 a 525991 ms y
playing=true tras recuperación. No se exige que el contador de generación de
observaciones permanezca igual: Phase 1 lo invalida deliberadamente al recuperar.

Disconnect Developer → Linux envía texto+GIF → Resume/snapshot: mismos IDs en
ambos clientes, sin duplicados, 7 GIFs/1 reply/1 reacción recuperados, playing=true.
Tres entradas nuevas incluyen la presencia resumed existente. Snapshot no vuelve
a animar floating reactions. Evidencia sanitizada: [reconnect](results-reconnect.json).

Identidad nativa: SurfaceView conserva identificadores de objeto alrededor de
chat/picker; textura libmpv conserva ID. El test Flutter también comprueba una sola
creación de platform view durante picker/reply/contexto/fullscreen. Background
puede recrear la superficie del sistema; no se promete inmutabilidad de cada layer
SurfaceFlinger durante suspensión, ni se añadió lógica Kotlin.

## Validación automática

- TESTED: cargo fmt, clippy workspace/all-targets/locked con -D warnings,
  test workspace locked (120 PASS, 8 opt-in ignorados), build workspace locked.
- TESTED: opt-in social_real_player: 1 PASS, dos libmpv reales, GIF/reply/reacción y resume.
- TESTED: Flutter analyze; suite completa 60 PASS incluye bridge real y pruebas provider/UI.
  Ver informe para GitHub Actions.
- TESTED: check_docs, check_ci (Python con PyYAML), tests Python de scripts.
- TESTED: fixtures HTTP/mocks, URLs/esquemas, metadata, spoof, room/epoch,
  capabilities antiguas, cuotas/dedup; soak 500 mensajes mixtos/1000 toggles,
  snapshot peor caso ≤48 KiB conservador, LRU y límites de resultados.

La suite Flutter requiere `CINE_BRIDGE_LIBRARY=$PWD/target/debug/libcine_ui_bridge.so`
(resolver path desde root). Una repetición sin esa variable falló al cargar FFI;
se corrigió la invocación. No fue una regresión del producto.

## Visual QA y corrección

VISUALLY VERIFIED: capturas de widgets a 1366×768, 1920×1080, 390×844, 844×390,
568×320, más capturas físicas identificadas por `physical` en
[screenshots](screenshots/README.md). Widget renders no equivalen a dispositivos.
Se inspeccionaron tarjetas, quotes, counts, picker, safe area y fullscreen.

Bug demostrado: GIF nuevo podía conservar placeholder tras autoscroll porque
el callback de visibilidad quedaba pendiente sin frame adicional. Regresión
`new GIF autoscroll resumes image stream inside viewport` falla antes y pasa
con recheck después de build/layout y ensureVisualUpdate. También comprueba
preservación del lector cuando llegan nuevos mensajes. Se amplió el grid por
ancho máximo de celda para evitar una tarjeta enorme en landscape; se corrigió
fallback de fuente de las capturas, sin añadir fuentes/dependencias al producto.

## Recursos y límites

MEASURED: [muestras debug](results-performance.json), RSS Linux y PSS Android,
CPU Linux en intervalos de 3 s; baseline texto, un GIF, varios, picker, scroll,
tras cerrar. Son muestras secuenciales con un solo asset compartido, hot reload y
GC; no benchmark controlado, causalidad ni cap del RSS completo. MEASURED adicional:
trace VM debug durante scroll Android9.52s, 116 scopes Frame/116 raster. Frame
p95 26.28ms/max71.17, 14 sobre16.67ms; raster p95 7.05ms/max51.15, 3 sobre16.67ms.
No son dropped frames presentados ni scopes sumables. El vídeo ya alcanzó EOF en
esta muestra: no demuestra jank durante playback. Frame timing Linux, profile/release
y soak físico de semanas: NOT TESTED / NOT MEASURED.

Cache IMPLEMENTED: bytes LRU 8 MiB/32 entradas, asset ≤2 MiB; Flutter ImageCache
24 MiB/64 entradas, decode width ≤320. No disco. Imágenes vivas, codecs, Player,
engine y bridge añaden memoria fuera de esos presupuestos. Picker ≤100 resultados,
páginas de 20, debounce 350 ms, timeout 8 s, respuestas JSON ≤2 MiB. GIF de chat
≤240×180 lógico con aspect ratio reservado. RoomService guarda solo descriptors.

No Internet GIF en QA: fixture se resuelve a asset propio; .invalid nunca se
consulta. El servidor no hace HTTP ni proxy: no ruta SSRF. Con proveedor bloqueado,
fallos remotos no pueden afectar WebSocket, Ready ni Player. Descarga remota real,
renovación de URLs expiradas y branding completo requieren una fase de activación
aprobada, sin habilitar arbitrary URLs.
