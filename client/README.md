# Cliente futuro

No hay UI ni app generada. Flutter es candidato (ADR-007), sujeto a spike de
multimedia y puente. Plataformas: Windows/Linux/macOS/Android/iOS.

Application coordinará crear/unirse/resume, estado observable, selección local,
probe/hash en worker, Ready, reloj, scheduler y efectos del SyncEngine. UI solo
invoca casos de uso. Transport no aplica permisos ni Player recibe DTOs wire.
Adapters locales pueden usar URI/handles móviles en vez de rutas.

Primer entregable: dos clientes de experimento con UI mínima o CLI, sin diseño
visual final. Contratos en [ARCHITECTURE](../docs/ARCHITECTURE.md),
[PROTOCOL](../docs/PROTOCOL.md) y [spike prioritario](../experiments/01-player-crossplatform/README.md).
