# Capturas y revisión visual

**Capturas físicas, VISUALLY VERIFIED**, 2026-10-08: Linux KDE/Wayland,
1366×768, libmpv embebido; Samsung SM-J701M/API28, Media3, 720×1280 y
1280×720. Corpus sintético; nombres controlados Alex/Sam. Sin invitaciones,
URI, paths, SHA, IP, tokens ni mensajes personales en las capturas publicadas.

| Archivo | Evidencia |
| --- | --- |
| linux-lobby-chat.png | Lobby real, Ready, conversación compartida |
| linux-player-chat.png | Vídeo Linux real con panel lateral |
| linux-reaction-overlay.png | Risa enviada desde Android sobre vídeo Linux |
| linux-fullscreen-reactions.png | Fullscreen real con capa social |
| android-chat-portrait.png | Sheet de conversación real, portrait |
| android-reaction-portrait.png | Corazón enviado desde Linux, Media3 visible |
| android-player-landscape-reaction.png | Media3 landscape/fullscreen y reacción |
| android-reconnect-history.png | Historial recuperado tras resume/background |
| android-chat-landscape-keyboard.png | Teclado y editor en landscape |

**Renders de widgets, TESTED**, `widget-*.png`: viewports 1366×768,
1920×1080, 390×844, 844×390, 568×320. Fake gateway, fuentes reales
Roboto/MaterialIcons/Noto Emoji. El Player está explícitamente no disponible.
No son capturas de Android ni prueba de frames multimedia.

Revisión e iteración: se corrigieron el header estrecho, el espacio con teclado
en landscape, el clipping de radios y los atajos de edición. Las capturas finales
se inspeccionaron: texto/emoji legibles, input visible, proporción secundaria de
chat, wrapping correcto y reacciones separadas del timeline. El panel fullscreen
superpone vídeo sin reducirlo; mobile usa sheet, sin desmontar la superficie.
La película sintética tiene colores intensos para distinguir los frames reales;
no representa el color del design system.

El icono GIF/micrófono visible en el teclado landscape pertenece al IME del
sistema Android; la app no implementa GIF, voz ni permisos asociados.
