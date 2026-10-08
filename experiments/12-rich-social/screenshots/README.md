# Capturas — Rich Social

`rich-<ancho>x<alto>.png` y `picker-<ancho>x<alto>.png`: renders de widget tests,
Flutter software renderer, fonts locales cargadas solo para captura; no capturas
físicas. FakePlayer, fixture propio y nombres sintéticos Alex/Sam.

`*-physical.png`: capturas de Linux KDE Wayland 1366×768 o Samsung SM-J701M
720×1280 (y landscape). Debug, libmpv/Media3 reales, mismo vídeo sintético local.
Las últimas correcciones Dart se aplicaron por hot reload, sin recrear la sesión.
No búsqueda externa ni credencial. Ver [resultados](../RESULTS.md).

Revisión visual: tarjetas contain sin estirar; dimensiones reservadas; quotes y
counts legibles; picker compacto/adaptado; menús con targets accesibles; landscape
usa más columnas. Iteración corrigió placeholder tras autoscroll, grid landscape
excesivo y fuente de emoji de los renders. Capturas iniciales conservadas muestran
el recorrido; `final` identifica capturas posteriores cuando existe variante.
No invitation token, clave, query privada, path personal ni archivo de usuario.
