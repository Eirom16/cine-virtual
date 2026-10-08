# INFORME — SOCIAL EXPERIENCE PHASE 2: GIFs & RICH CHAT

Fecha: 2026-10-08. Esta fase extiende Phase 1. Estados distinguen implementación,
pruebas, mediciones e inferencias. **Rich Chat con fixture: IMPLEMENTED / TESTED.
Proveedor remoto: BLOCKED. REAL PROVIDER SEARCH NOT TESTED.**

## 1. Estado inicial

TESTED: master `a771497`, working tree limpio, origin/master sincronizado,
GitHub autenticado. Actions `37802575391` SUCCESS. Se inspeccionaron status,
branch, 20 commits, remotes y cinco ejecuciones; no se asumió contexto anterior.

## 2. Auditoría social existente

TESTED por inspección: lectura de README y documentos ARCHITECTURE, PROTOCOL,
SYNC, MEDIA, UI, TESTING, DECISIONS, CI; experimentos 09/10/11 y resultados de 11.
[audit.md](audit.md) registra SocialEntry, sequence, history, floating events,
snapshot, bridge, SocialView y UI antes de agregar campos. Se conservan RoomService,
membership, dedup, rate limiter y overlay, extendiendo sus puntos existentes.

## 3. Proveedores GIF investigados

TESTED por documentación oficial actual: GIPHY, Tenor y KLIPY.
[Evaluación y fuentes primarias](provider-evaluation.md). Tenor retiró la API el
30 junio 2026; una integración basada en supuestos de 2024 sería incorrecta.

## 4. Comparación de proveedores

GIPHY tiene search/trending/ID, ratings y variantes pequeñas, pero requiere key,
aprobación de producción y autorización para cachear. Tenor ya no es candidato.
KLIPY exige partner key y restringe almacenamiento de contenido. Fixture propio
permite QA sin catálogo, permisos de terceros ni Internet; no sustituye un proveedor.

## 5. Proveedor elegido o blocker

BLOCKED: GIPHY en producto. El usuario eligió explícitamente «GIPHY bloqueado por
defecto; continuar con fixtures y Rich Chat». IMPLEMENTED: adapter GIPHY real,
provider unavailable y fixture controlado para QA; no se habilita búsqueda externa.

## 6. Credenciales

NOT IMPLEMENTED: activación de proveedor remoto. No hubo key legítima disponible,
no se inventó ni buscó una key pública. Constructor runtime del adapter con gate
explícito; aplicación no ofrece inyección de secretos ni los compila. Tests usan
credenciales sintéticas; logs/diagnósticos no incluyen key ni URI de búsqueda.

## 7. Privacidad

IMPLEMENTED: fixture no contacta terceros. Arquitectura futura estudiada: quien
busca contactaría API directamente; receptores contactarían CDN para render,
exponiendo IP y asset solicitado. Servidor solo transportaría descriptor, nunca
queries/key. GIPHY prohíbe proxy y limita cache; activación necesita aprobación.
No servidor central nuevo ni analytics, cuentas o identificadores de sala enviados.

## 8. Arquitectura del provider

IMPLEMENTED / TESTED: GifProvider pequeño con search, trending y resolve;
UnavailableGifProvider, FixtureGifProvider, GiphyGifProvider; transporte inyectable,
parser estricto y GifFailure con códigos seguros. Flutter posee búsqueda/render;
Rust posee identidad/validación/autoridad. RoomService nunca llama HTTP bajo mutex.

## 9. Flujo search/render

IMPLEMENTED: picker → provider → descriptor elegido explícitamente → intent Rust
→ evento autoritativo → mismo historial → renderer. Fixture convierte identidad
fixture:celebrate al asset propio incluido; su URL .invalid nunca se descarga.
Flujo remoto API/CDN separado: PROVISIONAL, bloqueado hasta aprobación.

## 10. Modelo GIF

IMPLEMENTED: provider, provider_content_id, media_url, preview_url opcional,
width, height, alt_text. Identidad estable provider+ID; URL es hint de entrega,
no identidad ni garantía de lifetime. Sin blobs, títulos enormes o objetos del
proveedor. Text/Gif/System discriminados en dominio; DTO mantiene fallback legacy.

## 11. Protocolo GIF

IMPLEMENTED / TESTED: MESSAGE_SEND con content.type=gif o text; CHAT_MESSAGE
con content opcional. Nombres de producto, sin GIPHY_MESSAGE/TENOR_GIF. Message ID,
sender/name de membership, room/epoch, social_sequence y timestamps autoritativos;
selección no genera burbuja confirmada hasta eco. No historial paralelo de GIFs.

## 12. Capability negotiation

IMPLEMENTED / TESTED: rich_social_v1 requiere social_v1, opt-in en HELLO/ACCEPT.
Sin rich, intent rico devuelve FEATURE_NOT_SUPPORTED. protocol_version sigue 1.
Bridge/client proyectan rich_supported; UI deshabilita acciones ricas sin soporte.

## 13. Compatibilidad con clientes antiguos

TESTED: cliente social_v1 recibe kind chat, text `[GIF]` y snapshots sin addons
rich. No recibe eventos desconocidos; reply/reacciones no se presentan como tipos
nuevos. Sin social no recibe social. Snapshots históricos sin content/UTC conservan
fallback y presupuesto legacy; ninguna migración durable.

## 14. Seguridad de URLs

IMPLEMENTED / TESTED: HTTPS, allowlist de provider/hosts/path/query, sin usuario,
puerto, fragmento, backslash ni whitespace. Se rechazan javascript/data/file/ftp,
localhost, hosts internos y arbitrary URLs. Fixture exige ID y URL exactos.
Servidor no descarga ni hace proxy: no ruta SSRF server-side en esta fase.

## 15. GIF validation

TESTED: ID ASCII alfanumérico/guion/underscore 1–64 bytes; URL ≤1024 bytes;
alt ≤256 bytes sin controles; dimensiones enteras 1–640; DTO sin claves extra.
Allowlist sintáctica GIPHY no equivale a activación: RoomService rechaza GIPHY
por política. Fixture propio es el único medio aceptado/renderizado actualmente.

## 16. Historial/reconnect

IMPLEMENTED / TESTED: mismo buffer reciente ≤100 entradas/48 KiB conservador,
metadata y reacciones incluidas. Snapshot/replay recupera GIF, reply y reacciones.
Sin blobs ni DB. Tests mixtos y peor caso serializado verifican presupuesto.
Evidencia física: [results-reconnect.json](results-reconnect.json).

## 17. Dedup GIF

TESTED: se reutiliza dedup por evento/fingerprint/membership. Reintentar no crea
segunda entrada ni segundo toggle. Dos clientes WebSocket y replay verifican
IDs únicos; prueba física compara IDs completos entre Linux/Android tras resume.

## 18. GIF picker desktop

IMPLEMENTED / VISUALLY VERIFIED: botón junto al input, diálogo compacto,
búsqueda arriba, grid, loading, empty, retry, unavailable y paginación. Selección
explícita envía inmediatamente; cerrar no envía. Funciona en fullscreen Linux.

## 19. GIF picker mobile

IMPLEMENTED / TESTED / VISUALLY VERIFIED: bottom sheet con safe area y adaptación
al teclado, portrait/landscape. Player permanece montado; picker no navega a otro
Player. QA física usa fixture; widget tests cubren restricciones pequeñas.

## 20. Search/debounce

IMPLEMENTED / TESTED: 350 ms; generación invalida inmediatamente una búsqueda
vieja incluso durante debounce. Respuestas tardías/canceladas/dispose no reemplazan
resultados actuales. Timeout 8 s; no una request inmediata por cada tecla.

## 21. Pagination

IMPLEMENTED / TESTED: API pide 20, carga incremental, máximo 100 resultados
retenidos por picker. Cursor GIPHY validado; search offset ≤4999, trending ≤499.
Fixture tiene una sola página. Catálogo remoto real NOT TESTED.

## 22. Provider errors

IMPLEMENTED / TESTED: bloqueado, no configurado, offline, timeout, 429, empty,
JSON malformado, asset faltante, error del proveedor y retry. No se insertan errores
de búsqueda en historial ni se bloquea texto. Errores expuestos como códigos seguros.

## 23. GIF rendering

IMPLEMENTED / TESTED: Image.memory nativo Flutter, GIF silencioso; formato WebP
previsto en descriptor/adapter, render WebP físico NOT TESTED. Contain y tamaño
reservado; placeholder estable «GIF no disponible». No MP4/audio, paquetes nuevos
de cache/render ni cambios en Kotlin, Media3 o libmpv.

## 24. Cache

IMPLEMENTED / TESTED: LRU de bytes solo en memoria por sesión, ImageCache Flutter;
clear en Developer y dispose. No disk cache, queries persistidas, secrets ni ruta
de almacenamiento social. Fixture local vuelve a resolver sin red al reconectar.
Cache remoto no está habilitado: es parte del blocker de términos.

## 25. Cache limits

TESTED: bytes ≤8 MiB/32 entradas, asset ≤2 MiB; ImageCache ≤24 MiB/64 entradas,
decode width ≤320. No equivalen a límite total del proceso: live images/codecs,
engine y Player consumen memoria adicional. Tests de eviction/clear/límites sin
cientos de descargas reales; JSON HTTP ≤2 MiB, sin seguir redirects.

## 26. Memory

MEASURED: RSS Linux y PSS Android en debug; [muestras](results-performance.json).
Baseline texto 435036/226191 KiB respectivamente; un GIF 448340/224651;
varios 480592/225500; picker Android 309248/270221; cierre 255476/237551.
Son estados secuenciales, un asset compartido, GC/hot reload; no estimación causal.

## 27. Bandwidth/formats

IMPLEMENTED: adapter selecciona fixed_width_downsampled con size declarado ≤2 MiB,
rating g, preview still, evita original y vídeo. Fixture 7044 bytes/160×100/8 frames,
0 bytes de tráfico GIF externo. Descarga remota con enforcement de tamaño real,
expiración/re-resolve y catálogo de renditions: NOT IMPLEMENTED hasta activación.

## 28. Offline behavior

TESTED con mocks: fallos del proveedor son estados UI; fixtures no requieren red.
Texto/room dependen de WebSocket, no de Internet hacia CDN. Render de contenido
remoto bloqueado muestra placeholder. Corte físico de Internet selectivo al CDN
NOT TESTED porque ningún proveedor remoto fue activado.

## 29. Accessibility GIF

IMPLEMENTED / TESTED: label «GIF enviado por …» más alt limitado; frames no se
anuncian. MediaQuery disableAnimations/accessibleNavigation decodifica solo primer
frame y dispone codec; test verifica RawImage sin Image animada. Offscreen retira
stream; no se afirma CPU cero para todo el engine ni QA con lector físico.

## 30. Reply protocol

IMPLEMENTED / TESTED: UUID reply_to_message_id, sin duplicar contenido original;
solo Text/Gif retenido de la misma sala al aceptar. Cross-room/unknown/sistema
rechazados. Referencia aceptada puede sobrevivir eviction del original con fallback;
no se permite crear ahora reply a un ID ya evicted.

## 31. Reply UI

IMPLEMENTED / TESTED / VISUALLY VERIFIED: composer con preview cancelable,
quote compacto resuelto por ID, «Mensaje anterior no disponible» si falta original.
Sin threads. Linux respondió físicamente al GIF enviado por Android.

## 32. Message reactions protocol

IMPLEMENTED / TESTED: MESSAGE_REACTION_SEND message_id/emoji, actor de binding.
Toggle por miembro/emoji, seis emojis ❤️ 😂 😮 😢 🔥 👏. Cuota independiente burst6,
refill1/s, conservada con resume. Unknown/system/cross-room y spoof rechazados.

## 33. Message reaction state

IMPLEMENTED / TESTED: mapa emoji→set de UUIDs, máximo16 por emoji/entry; agregado
por count. Eviction elimina también mapa; presupuesto incluye metadata. Toggle
incrementa watermark social y publica SOCIAL_STATE live_update=true; entries
conservan sequence original. Live update preserva floating queue, reconnect la vacía.

## 34. Reaction UI

IMPLEMENTED / TESTED / VISUALLY VERIFIED: menú de seis acciones, badges con counts,
estado propio y toggle accesible; targets mínimos44. Android añadió ❤️ a JAJAJA,
Linux recibió ❤️1. Floating reactions siguen efímeras y semánticamente separadas.

## 35. Copy

IMPLEMENTED / TESTED: texto plano via Clipboard; físicamente verificado JAJAJA.
GIF copia alt/descripción, nunca metadata ni URL interna. Desktop mantiene
SelectableText; no Markdown, mentions ni rich link previews.

## 36. Context menus

IMPLEMENTED / TESTED: menú visible, right click desktop y long press móvil,
Responder/Reaccionar/Copiar. Sin Edit/Delete/Report/Pin. Widgets prueban acciones;
menú y reacción usados físicamente. Right click/long press físico no medidos aparte.

## 37. Timestamps

IMPLEMENTED / TESTED: sent_at_utc_ms opcional del servidor, epoch UTC para
presentación local vía MaterialLocalizations. Sequence conserva orden/autoridad;
monotonic ms no se presentan como hora. Históricos sin UTC omiten hora; clock
changes no cambian orden. UI física Linux24h/Android12h según configuración.

## 38. Scroll/layout stability

IMPLEMENTED / TESTED: dimensiones reservadas antes de load/error, máximo240×180,
scroll bottom condicionado, preserve lector y botón nuevos mensajes. Regresión
demostró placeholder retenido tras autoscroll; corrección revalida visibilidad tras
layout y solicita frame. Test falla antes/pasa después; scroll físico inspeccionado.

## 39. Unread behavior

IMPLEMENTED / TESTED: GIF autoritativo cuenta como mensaje nuevo; abrir chat limpia
unread. Snapshots/reacciones no generan duplicados ni unread de mensajes viejos.
Indicador secundario para reacción a mensaje propio NOT IMPLEMENTED, opcional.

## 40. Flutter/Rust bridge

IMPLEMENTED / TESTED: intents message/message_reaction en owners desktop/móvil,
proyección rica del mismo Client/RoomService; sin autoridad Dart ni lógica social
Kotlin. Tests de bridge FFI requieren CINE_BRIDGE_LIBRARY apuntando al .so construido.

## 41. Presentation state

IMPLEMENTED: SocialView/notifier separado de room/playback, reply draft/selection,
GifSearch, GifMediaCache y diagnóstico agregado. No Player nuevo al abrir sheets.
El historial confirmado procede de Rust; no optimistic duplicates ni query en room.

## 42. Security tests

TESTED: esquemas/hosts/path/query arbitrarios, URL/alt oversized, dimensiones
inválidas, claves extra, provider/sender spoof, room/epoch erróneos, cross-room reply,
reaction unknown, cuotas y retries. Sin servidor fetch. No pentest externo ni
certeza de conformidad legal: activación proveedor continúa BLOCKED.

## 43. Protocol tests

TESTED: codec válido/inválido, descriptor/text discriminado, addons/snapshots,
legacy fallback, rich capability, replies, reactions, live update y presupuesto
con Unicode/escape. protocol_version1 conservado; tests existentes siguen verdes.

## 44. RoomService tests

TESTED: mixed500 mensajes/1000 toggles, mapas/buffer acotados, eviction, dedup,
rate limits, reply entre salas, proveedor deshabilitado y agregación. Sin red,
blobs ni DB. No modificación profunda de RoomService ni reemplazo de arquitectura.

## 45. Client tests

TESTED: dos clientes WebSocket con FakePlayer, text/GIF/reply/reacción/resume,
IDs únicos, cliente antiguo. Opt-in dos libmpv reales PASS con rich flow/reconnect.
Autoridad multimedia permanece independiente de social_sequence.

## 46. Flutter tests

TESTED: **60 PASS**, analyze sin issues. Picker/search/errores/debounce/cancel,
render/error/reduced motion, reply/cancel/fallback, count/toggle/copy/menu/unread,
autoscroll/preserve, fullscreen/keyboard y una sola creación AndroidView.
Última suite completa ejecutada con biblioteca FFI real; no dependencia Internet.

## 47. Provider tests

TESTED: siete tests de adapter/HTTP/mocks cubren search/trending/resolve,
pagination, empty, missing asset, malformed, rating, 429, offline, tamaño, timeout,
stale/cancelled/dispose, LRU. No test de key real ni consulta GIPHY física.

## 48. Responsive tests

TESTED / VISUALLY VERIFIED: 1366×768,1920×1080,390×844,844×390,568×320,
rich card/quote/counts/picker/menu y teclado simulado. Capturas de widget test
etiquetadas; dispositivo físico Android720×1280/landscape y Linux1366×768.
No se confunde resolución de captura con emulación de todos los dispositivos.

## 49. Real provider test

**REAL PROVIDER SEARCH NOT TESTED. BLOCKED** por decisión explícita, términos/cache
y configuración aprobada ausente. Adapter real probado con fixtures, no una búsqueda
real. Ninguna key ni credencial privada publicada en artefactos/screenshots/logs.

## 50. Linux ↔ Android real test

TESTED / VISUALLY VERIFIED: Host Alex Linux, Participant Sam Samsung Media3,
ambos Ready/Play; texto→GIF→reply JAJAJA→❤️1, floating 😂, fullscreen, Pause/Seek/Play.
GIF controlado propio, no proveedor real. Mismo vídeo sintético local, no upload.
Logs/read-only VM sanitizados observan estado; acciones por input físico/ADB.

## 51. Reconnect

TESTED físicamente: Developer desconecta Android, Linux envía texto+GIF,
Resume/snapshot recupera mismos IDs, siete GIFs/una reply/una reacción y presence,
sin duplicados. Playback playing=true tras recuperación. Historial efímero sujeto
a lease/eviction Phase1; no persistencia permanente prometida.

## 52. Android background

TESTED físicamente con reproducción activa: Home→mensajes durante ausencia→foreground;
recuperación existente revalida media/Ready, mismo membership, dos entradas nuevas,
playing=true y timeline avanzado. Generación de observaciones cambia por diseño.
Identidad SurfaceView comprobada alrededor de UI social; layers durante background
no se declararon inmutables. iOS background no probado.

## 53. Performance

MEASURED: CPU Linux intervalos3s: un GIF39.7%, varios26%, picker40%, scroll54.6%,
cierre31% de un core. No comparación controlada: intervalos debug, GC/hot reload y
un asset. VISUALLY VERIFIED reproducción/scroll en QA; jank cuantitativo/frame timing,
profile/release Android y percentiles de sync NOT MEASURED / NOT TESTED.

## 54. Memory/resources

MEASURED: RSS/PSS del proceso completo en JSON; tras cerrar RSS Linux255476 KiB,
Android237551 KiB PSS. No crecimiento ilimitado demostrado en estas muestras;
soak físico de semanas NOT TESTED. TESTED determinísticamente presupuestos de
historial, mapas, dedup existente, cache y búsqueda. No descarga de cientos de GIFs.

## 55. Developer diagnostics

IMPLEMENTED: provider status/last safe error/search latency, cache entries/bytes,
rich capability, GIF/reply/reaction counts. Botón clear cache. Sin contenido completo,
queries, keys ni URLs de proveedor. Métricas aproximadas, no herramienta de profiling.

## 56. Screenshots

VISUALLY VERIFIED: [índice](screenshots/README.md), diez renders de tamaños
solicitados y capturas físicas Linux/Android de picker, GIF, reply/counts, floating,
fullscreen, background/replay y scroll. Nombres/datos sintéticos; no tokens ni rutas
personales. Capturas físicas debug con hot reload explicitado.

## 57. Visual QA

VISUALLY VERIFIED con iteración: aspect ratio/contain, radio, tamaño, quotes,
counts y fullscreen inspeccionados. Se corrigió placeholder tras autoscroll,
grid landscape demasiado grande y fuente de renders de emojis. Keyboard/safe area
cubiertos en widgets; no lector de pantalla físico ni todos los IME comerciales.

## 58. Bugs encontrados

TESTED: placeholder retenido después de append/autoscroll; captura sin fallback
emoji en badge; grid de dos columnas excesivo en landscape; adapter trending debía
limitar offset499. Una invocación de suite sin env FFI falló por configuración del
runner; no bug social. No evidencia suficiente para cambiar Player/SyncEngine.

## 59. Bugs corregidos

IMPLEMENTED / TESTED: recheck de visibilidad/build y ensureVisualUpdate con
regresión antes/después; grid maxExtent180; fuente de capturas aplicada a todo
TextTheme; cursor según endpoint y test. La suite final pasó completa con FFI.
Sin modificar Player, SyncEngine ni Kotlin para resolver estas correcciones.

## 60. Limitaciones actuales

BLOCKED: activación GIPHY/credencial/branding/cache y render remoto de producción.
NOT IMPLEMENTED: disk cache, descarga remota aprobada, refresh de URL expirada,
reaction notifications secundarias, threads, edit/delete/uploads/DB/accounts.
NOT TESTED: proveedor físico, WebP físico, profile/release/jank y lector real.

## 61. Windows/macOS/iOS

Compilación por matrix Actions; evidencia de runtime rich prioritariamente
Linux/Android. No se afirma runtime multimedia Windows/macOS/iOS por compilar.
iOS Player sigue **NOT IMPLEMENTED**. Ver ejecución de HEAD en Actions para
conclusión final de builds; no equiparar job en curso a PASS.

## 62. Regresiones multimedia

TESTED: cargo workspace, opt-in dos libmpv, Flutter identidad AndroidView y
prueba física Ready/Play/Pause/Seek/fullscreen/background/resume. Cambios social
no tocan timeline/Ready; media recovery Phase1 se conserva. No benchmark600s
repetido, ni afirmación nueva de precisión temporal/jank.

## 63. GitHub Actions

Validación final exige ejecución de HEAD completada y SUCCESS, no solo gate local.
[Actions de master](https://github.com/Eirom16/cine-virtual/actions) permite consultar
commit y matriz exactos; URL/cierre final se adjuntan en la entrega de esta sesión.
TESTED: ejecución intermedia [37857600609](https://github.com/Eirom16/cine-virtual/actions/runs/37857600609)
completada SUCCESS: gate Rust/Flutter/docs y todos los builds Linux, Windows,
Android3ABI, macOSIntel/arm64 e iOSdevice/simulator. El cierre verificará además
la ejecución del último commit de documentación y correcciones.

## 64. ADR/documentación

IMPLEMENTED: ADR-013 con Context/Options/Decision/Credential handling/Privacy/
Caching/Protocol neutrality/Consequences. README, ARCHITECTURE, PROTOCOL, UI,
TESTING, DECISIONS actualizados. Experimento12 conserva auditoría, fuentes,
resultados/JSON/capturas; no sobrescribe experimento11.

## 65. Commits

`18524ce` protocolo/rich state/bridge/tests; `44ad08d` provider/cache/picker/rich UI;
`42452fd` visibilidad/autoscroll, grid landscape y cursor. Documentación y evidencia
en commit posterior. Historial pequeño, sin force push/rebase destructivo.

## 66. Estado Git final

Cierre exige working tree limpio, diff --check, log15 y HEAD...origin/master=0/0,
con push autorizado. Comprobación exacta posterior al último commit/push se comunica
en entrega final; no se atribuye limpieza a un checkout durante edición.

## 67. ¿Está Rich Chat terminado para este MVP?

**IMPLEMENTED / TESTED con fixtures**: replies, reacciones, Copy, timestamps,
historial/reconnect, desktop/mobile y Player físico. Arquitectura GIF integrada y
segura para fixture; **PROVISIONAL / BLOCKED como catálogo GIF de producción**.
No declarar GIF externo terminado hasta aprobar proveedor y probar search/render real.

## 68. ¿Está Cine Virtual listo para comenzar voz?

INFERRED: todavía conviene cerrar proveedor y hardening antes de ampliar multimedia.
Esta sesión no implementa voz. Voz requiere ADR y fase propia de permisos/lifecycle,
privacidad, recursos y transporte; el éxito de rich chat no valida micrófono ni RTC.

## 69. ¿Conviene voz o P2P como siguiente gran fase?

INFERRED: si el objetivo es interacción social, voz aporta más directamente que P2P;
P2P cambia distribución de medios y alcance operativo. No es autorización para
ninguna. Prioridad inmediata: resolver el bloqueo GIF y repetir QA de producción.

## 70. Próximo paso recomendado

Obtener aprobación documentada de cache/atribución y política de credenciales de
un proveedor viable; revisar ADR-013, habilitar API/CDN acotadas, completar branding,
probar búsqueda real Linux/Android y repetir memory/profile QA con varios assets.
No añadir proxy general, DB/cuentas ni arbitrary URL como atajo.

## 71. Resumen para otro arquitecto

Phase1 sigue siendo la base. Rich opt-in v1 añade contenido discriminado neutral,
reply por ID y reacciones acotadas con snapshots, manteniendo fallback legacy y
autoridad Rust. Flutter encapsula provider/search/cache/render y UX; Player intacto.
Fixture demuestra transporte/producto/recovery físico. GIPHY permanece bloqueado
conscientemente; tests de adapter no demuestran búsqueda ni licencia real. Evidencia
local/pipeline y límites están separados; no se inició voz, vídeo, P2P ni cuentas.
