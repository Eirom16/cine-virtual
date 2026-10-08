# Proveedores GIF — investigación oficial, 2026-10-08

| Proveedor | API / credenciales | Búsqueda / formatos | Restricción decisiva | Decisión |
| --- | --- | --- | --- | --- |
| GIPHY | API v1, key; beta 100 llamadas/h, producción sujeta a revisión/precio | Search, trending, ID; GIF/WebP/MP4, rating g disponible | Cache de medios/URLs requiere aprobación; acceso directo, sin proxy | Adapter implementado y probado con fixtures; BLOCKED en producto |
| Tenor | API v2 con key Google; dejó de aceptar clientes nuevos en enero 2026 | Históricamente search/featured/ID y media_filter, contentfilter | Retirada oficial 30 junio 2026 | No integrar una API retirada |
| KLIPY | API con key de partner, test 100/h; producción requiere solicitud | GIF/search/trending y compatibilidad Tenor | Términos restringen almacenamiento de contenido (excepción thumbnails de búsqueda) | No resuelve el requisito de cache de mensajes |
| Fixture controlado | Sin credencial ni red | Un GIF sintético propio, búsqueda local acotada | No demuestra catálogo ni búsqueda remota real | Elegido para QA explícita, no provider de producción |

Fuentes primarias: [GIPHY API](https://developers.giphy.com/docs/api/),
[términos GIPHY](https://support.giphy.com/hc/en-us/articles/360028134111-GIPHY-API-Terms-of-Service),
[privacidad GIPHY](https://support.giphy.com/hc/en-us/articles/360032872931-GIPHY-Privacy-Policy),
[retiro Tenor](https://support.google.com/tenor/answer/10455265?hl=en),
[quickstart histórico Tenor](https://developers.google.com/tenor/guides/quickstart),
[cache histórico Tenor](https://developers.google.com/tenor/guides/rate-limits-and-caching),
[KLIPY API](https://docs.klipy.com/),
[KLIPY developer](https://klipy.com/developers),
[términos KLIPY](https://klipy.com/support/api-terms).

GIPHY requiere marcas oficiales y atribución de usuario/fuente cuando exista.
La política pública no garantiza lifetime indefinido de URLs; usar provider+ID
como identidad, URL como hint de entrega. El adapter conserva URL íntegra,
solicita rating g y una rendition pequeña; rechaza respuestas sin asset válido.
Activación posterior exige revisar branding completo, revalidación, paths actuales,
licencia/contrato y tratamiento de credenciales. El texto de atribución del adapter
no sustituye la integración de marcas oficiales. Esa activación NOT IMPLEMENTED.

El proveedor remoto podría conocer IP, búsqueda y GIF solicitado por los clientes.
La política de privacidad de GIPHY enumera IP, identificadores de dispositivo y
queries entre datos automáticos. No se suministran member_id, room_id ni telemetría
de clicks. No se crea una cuenta Cine Virtual ni infraestructura de analytics.

No se encontró una autorización primaria inequívoca para tratar cualquier key
como pública. No se distribuye ninguna key. El constructor del adapter admite
configuración runtime en memoria; el producto NO expone un mecanismo para
inyectar secretos ni activar GIPHY. Fixtures usan credenciales sintéticas solo en
tests HTTP, sin Internet. El usuario eligió explícitamente mantener GIPHY bloqueado.

REAL PROVIDER SEARCH NOT TESTED. No prueba de búsqueda física ni de licencia
comercial concedida. Ninguna URL externa fue descargada para la QA de chat.
