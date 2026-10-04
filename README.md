# Cine Virtual

Base de un proyecto con vocación open source para ver contenido juntos desde
Windows, Linux, macOS, Android e iOS. Cada dispositivo conserva su archivo y el
servidor coordina la sala; el control de reproducción no transporta el vídeo.

**Estado: fundaciones, previo al MVP v0.1.** No hay aplicación ejecutable, servidor
WebSocket ni reproducción multimedia real. La licencia está pendiente: ver
[evaluación de licencias](docs/LICENSING.md). No se declara todavía una licencia
open source definitiva.

| Estado | Alcance real |
| --- | --- |
| Implemented | Primitivas Rust de reloj, timeline, corrección de drift, orden de eventos, comparación de identidad y puerto Player; tests con reproductor falso. |
| Planned | Salas, presencia, selección/metadatos/hash completo de archivos, Ready, controles del Host y recuperación básica para v0.1. |
| Experimental | Selección de motor multimedia, puente Rust/UI, WebSocket real, mediciones de reloj y WebRTC; actualmente solo planes de experimentación. |

El objetivo de v0.1 es demostrar dos clientes con el mismo vídeo local,
Play/Pause/Seek programados y recuperación tras una desconexión temporal.
Chat, distribución P2P, voz/cámara y providers son etapas posteriores, no
capacidades disponibles. El soporte de las cinco plataformas es un objetivo;
esta pasada se probó solamente en el entorno Linux disponible.

## Arquitectura

```text
UI → Application → Domain/Core
        │            │
        └──── puertos Player / transporte / reloj
                      ↑
              adapters de plataforma

WebSocket adapter → servicio de salas → estado autoritativo
```

El Core no depende de Flutter, Axum, Tokio ni de un reproductor concreto.
Rust es la base inicial del Core. Flutter y Axum/Tokio son candidatos; la
integración multimedia debe pasar un spike en móvil antes de elegirse.

## Organización

```text
docs/          producto, arquitectura, protocolo, sincronización, seguridad,
               testing, roadmap, decisiones, contribución y licencias
client/        contrato y responsabilidades del cliente futuro
core/          biblioteca Rust sin dependencias externas y tests
server/        contrato y responsabilidades del servidor futuro
experiments/   cinco planes de spikes, sin implementar integraciones
scripts/       validación de documentación
```

Empieza por [PRODUCT](docs/PRODUCT.md), [ARCHITECTURE](docs/ARCHITECTURE.md) y
[PROTOCOL](docs/PROTOCOL.md). [SYNC](docs/SYNC.md) precisa el modelo temporal;
[DECISIONS](docs/DECISIONS.md) registra las elecciones y su estado.

## Validación local

Rust 1.85 o posterior con Cargo, rustfmt y Clippy; Python 3.10 o posterior para
validar documentación. No hace falta UI, red ni reproductor instalado.

```sh
cargo test --workspace --offline
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo build --workspace --offline
python3 scripts/check_docs.py
```

Consulta [CONTRIBUTING](docs/CONTRIBUTING.md) antes de proponer cambios. El siguiente
trabajo es el [spike de dos clientes](experiments/01-player-crossplatform/README.md),
con pruebas tempranas en Android e iOS, sin convertirlo aún en producto.
