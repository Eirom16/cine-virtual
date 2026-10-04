# 01 — Player y primer recorrido de dos clientes

**Prioridad máxima. Estado: diseñado, no ejecutado.**

Objetivo: dos clientes → mismo vídeo local → conexión → PLAY → PAUSE → SEEK →
medición de drift. Validar la parte de mayor riesgo antes de construir UI final.
CLI/harness o UI mínima son suficientes; no requiere cuentas ni providers.

## Alcance del spike

1. Elegir dos candidatos iniciales por factibilidad (libVLC y libmpv como punto
   de partida), admitiendo players nativos/GStreamer si móvil lo exige.
2. Adapter Player: carga local, position con timestamp, seek completion, play,
   pause, capacidad rate y errores. Hash/probe con herramientas/bibliotecas de
   spike, sin rutas en payloads ni SHA casero.
3. RoomService efímero mínimo y WebSocket v1: create/join, metadata, Ready,
   controles del Host, secuencia y snapshot. No usar relay ciego de comandos.
4. Reutilizar Core para clock/timeline/drift, añadir timers Application con fake
   clock testable. Host programa por server, no reproduce al enviar el pedido.
5. Probar 10 min con ambos archivos idénticos, Pause/Seek y desconexión; copia
   distinta se rechaza. Registrar p50/p95/p99, RTT/jitter y correcciones/minuto.
6. Hacer prueba temprana en Android e iOS reales: URI/permiso, superficie,
   suspensión, audio/rate, empaquetado y licencias. Desktop inicial no basta.

## Criterios

Meta provisional: p95 drift absoluto ≤150 ms tras estabilización, lead inicial
500 ms, cero controles no autorizados y recuperación por snapshot. Documentar
limitaciones de keyframes/seek que impidan alcanzarla; revisar tuning con datos.
Publicar matriz Windows/Linux/macOS/Android/iOS con probado/no probado/fallo.
Si no hay dispositivo o toolchain, registrarlo como pendiente, no asumir éxito.

Resultado: evidencia comparativa y actualización ADR-006/007. No incorporar de
una vez todos los SDKs; descartar temprano candidatos inviables. Se permiten
backends distintos por plataforma detrás del mismo puerto. Contratos:
[PROTOCOL](../../docs/PROTOCOL.md), [SYNC](../../docs/SYNC.md), [MEDIA](../../docs/MEDIA.md).
