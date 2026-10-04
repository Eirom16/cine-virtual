# Producto

## Problema y visión

Amigos y familias separados físicamente quieren compartir una película sin
coordinar manualmente pausas, saltos y cuentas atrás. Cine Virtual busca ofrecer
una sesión compartida consistente, comprensible y recuperable en escritorio y
móvil, con posibilidad de operar el servidor propio.

El objetivo inmediato es probar sincronización con archivos idénticos. La visión
futura admite otras fuentes sin confundir coordinación con distribución del
contenido. No se ofrece ni se obtiene contenido por cuenta del usuario.

## Principios

- Estado visible: saber quién está conectado, quién está preparado y qué impide iniciar.
- Control explícito: las acciones de sala tienen permisos y una autoridad única.
- Recuperación: una interrupción breve no obliga a recrear toda la sesión.
- Privacidad: las rutas locales y los bytes del vídeo permanecen en el dispositivo.
- Portabilidad: diferencias de plataforma se resuelven mediante adapters.
- Foco: cada versión debe demostrar una experiencia completa antes de añadir medios de comunicación o nuevas fuentes.

## Personas y sala

Usuarios objetivo: grupos pequeños de amigos o familiares que ya tienen el mismo
archivo. No se requiere una cuenta en v0.1; se usa una identidad de sesión efímera.

Una sala es una sesión temporal, privada mediante invitación y alojada en un único
servidor autoritativo. Capacidad inicial de diseño: 16 miembros, configurable.
Los nombres visibles no son identidad ni credenciales.

**Host:** crea la sala, selecciona el contenido y controla Play/Pause/Seek. Puede
expulsar y transferir explícitamente el rol a otro miembro preparado. El servidor
no confía en que un cliente se autodenomine Host.

**Participant:** se une, selecciona su propia copia, informa su estado y marca
Ready. No emite controles globales en v0.1. Puede salir y solicitar snapshots.

**Moderator:** rol reservado para etapas posteriores. Podrá recibir permisos
acotados sin reemplazar al Host. No está habilitado en v0.1.

Permisos conceptuales: `play`, `pause`, `seek`, `change_media`, `kick_member`,
`manage_permissions`. En v0.1 son un perfil fijo del Host; no hay editor de permisos.
Ver [modelo de autoridad](ARCHITECTURE.md) y [protocolo](PROTOCOL.md).

## Flujo principal de v0.1

1. Host crea una sala y comparte una invitación; otros se unen y ven presencia.
2. Host elige un archivo; el dispositivo obtiene metadatos y SHA-256 completo.
3. Participantes eligen su copia. Coincidencia exige tamaño y digest completos
   iguales; nombres o títulos iguales no bastan.
4. Cada cliente carga el vídeo pausado, sincroniza su reloj y marca Ready para la
   selección vigente. Se muestra cualquier discrepancia o fallo de reproductor.
5. Cuando todos los miembros conectados están preparados, Host solicita Play.
   El servidor programa una transición compartida y todos siguen su timeline.
6. Pause y Seek usan el mismo modelo temporal. Cada cliente mide y corrige drift.
7. Tras perder conexión, el cliente pausa localmente, vuelve a sincronizar el
   reloj y restaura un snapshot. No reenvía controles antiguos automáticamente.

Una entrada tardía empieza no preparada y no detiene la reproducción del grupo.
Tras verificar el archivo y marcar Ready, el nuevo cliente converge al timeline.
Las personas desconectadas dentro del periodo de gracia no bloquean el inicio.
Un cliente que vuelve debe volver a preparar su reproducción.

## MVP v0.1 y aceptación

Dentro: crear/unirse/salir, presencia, invitación efímera, roles Host/Participant,
selección local, metadatos, SHA-256, coincidencia, Ready, controles programados,
clock sync, detección/corrección de drift, snapshot y reconexión básica. Transferir
Host explícitamente es una operación mínima de recuperación; moderación avanzada
queda fuera. Servidor único con memoria y ejecución self-hostable sencilla.

Se considera alcanzado al demostrar dos o más clientes reales con archivo
idéntico, impedir Ready/control inválido y recuperar una desconexión breve. La
meta provisional de medición es p95 de drift absoluto ≤150 ms tras estabilizar
durante una prueba de 10 minutos; debe publicarse entorno, RTT y capacidades de
seek. Los umbrales y la meta se revisan con evidencia, no constituyen una promesa.
No declarar compatibilidad de una plataforma sin ejecutar el spike en ella.

Fuera: cuentas, amigos, perfiles, chat/GIFs/reacciones, voz/cámara/screen sharing,
P2P de archivos, HTTP/HLS/DASH reales, Jellyfin/Plex, plugins, nube/CDN, biblioteca,
recomendaciones, PostgreSQL/Redis y federación. El MVP no garantiza supervivencia
a reinicios del servidor ni funcionamiento sin conexión.

## Evolución

Primero estabilizar recuperación y usabilidad de salas; después chat, P2P
experimental, comunicación audiovisual y providers. Streaming y estabilidad
llegan con criterios propios. [ROADMAP](ROADMAP.md) detalla las puertas de entrada.
No se incorporará una funcionalidad solo porque aparezca en la visión.
