# Roadmap por evidencia

No son fechas ni promesas de distribución. El estado actual incluye Spike A de control validado
con FakePlayer, anterior a v0.1. [PRODUCT](PRODUCT.md) gobierna alcance, [TESTING](TESTING.md)
gobierna aceptación. Las cinco plataformas son objetivo; se publica cobertura
medida por versión, no se declara soporte por heredar un SDK.

| Versión | Resultado focal | Puerta de salida |
| --- | --- | --- |
| Pre-v0.1 | Núcleo puro y spikes Player/puente/red/reloj | Dos clientes reales; decisión multimedia con evidencia Android/iOS |
| v0.1 | Salas efímeras, mismo archivo local, hash, Ready, controles programados, drift y resume básico | Prueba compartida 10 min, copy mismatch, autorización, snapshot tras desconexión, métricas publicadas |
| v0.2 | Recuperación robusta y usabilidad de salas | Suspensión móvil, leases, UX de errores, transferencia más robusta, reconexión bajo jitter; valorar persistencia solo si necesaria |
| v0.3 | Chat de texto | Mensajes acotados, accesibilidad, abuso y permisos; GIFs/reacciones se evalúan por foco |
| v0.4 | Distribución P2P experimental de archivos | Consentimiento, integridad por chunks, cancelación/reanudación y mediciones; sin asumir acceso a todo el dispositivo |
| v0.5 | Voz y cámara experimental | Spike WebRTC, TURN/privacidad, permisos móviles y presupuesto de recursos; screen sharing condicionado por plataforma |
| v0.6 | Primera integración media provider | Un provider concreto con identidad/capacidades claras, credenciales fuera del estado compartido; arquitectura de plugins aún por validar |
| v0.7 | Streaming HTTP/HLS/DASH progresivo | VOD primero, identidad/timeline y seguridad URLs; live tiene gate separado |
| v1.0 | Primera versión estable | Compatibilidad publicada, protocolo estable, seguridad, packaging/actualización, self-host documentado, licencia resuelta y pruebas reales |

Federación, recomendaciones y bibliotecas avanzadas no tienen versión comprometida.
PostgreSQL/Redis no son hitos por sí mismos. Cada fuente necesita aceptación
propia, no se anuncia Jellyfin/Plex o plugins antes de tener una integración real.
v0.2 amplía recuperación ya presente en v0.1; no posterga esa función básica.
