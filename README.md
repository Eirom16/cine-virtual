# Cine Virtual

Base de un proyecto con vocación open source para ver contenido juntos desde
Windows, Linux, macOS, Android e iOS. Cada dispositivo conserva su archivo y el
servidor coordina la sala; el control de reproducción no transporta el vídeo.

**Estado: Spike A de control funcionando, previo al MVP v0.1.** Hay servidor
WebSocket y clientes CLI con FakePlayer; todavía no hay reproducción multimedia
real. La licencia está pendiente: ver
[evaluación de licencias](docs/LICENSING.md). No se declara todavía una licencia
open source definitiva.

| Estado | Alcance real |
| --- | --- |
| Implemented | Core determinista, RoomService en memoria, codec v1, autoridad/Ready, controles programados y resume; validación con FakePlayer y WebSocket localhost real. |
| Planned | Archivos locales reales, metadatos/hash completo, adapter multimedia y validación de plataformas para v0.1. |
| Experimental | Spike A ejecutable de control; motor multimedia, puente Rust/UI y WebRTC continúan como planes sin integración. |

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
Rust es la base inicial del Core. Axum/Tokio se usan en este spike; Flutter sigue
como candidato. La integración multimedia debe pasar un spike en móvil antes de elegirse.

## Organización

```text
docs/          producto, arquitectura, protocolo, sincronización, seguridad,
               testing, roadmap, decisiones, contribución y licencias
client/        CLI experimental, réplica/scheduler y FakePlayer
core/          biblioteca Rust sin dependencias externas y tests
rooms/         RoomService y RoomStore, sin WebSocket ni JSON
protocol/      DTOs wire y codec v1 validado
server/        ejecutable WebSocket con Axum/Tokio
experiments/   planes de spikes y resultados del experimento de control
scripts/       diagnóstico, demo de tres procesos y validación de docs
```

Empieza por [PRODUCT](docs/PRODUCT.md), [ARCHITECTURE](docs/ARCHITECTURE.md) y
[PROTOCOL](docs/PROTOCOL.md). [SYNC](docs/SYNC.md) precisa el modelo temporal;
[DECISIONS](docs/DECISIONS.md) registra las elecciones y su estado.

## Validación local

Rust ≥1.89 para el workspace del spike (verificado con 1.99), Cargo, rustfmt y
Clippy; Python ≥3.10. Core conserva su mínimo 1.85 y ninguna dependencia externa.
Se necesita acceso a crates.io en la primera compilación y TCP localhost en las
pruebas WebSocket. No se necesita reproductor instalado.

```sh
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
python3 scripts/check_docs.py
```

Para reproducir el experimento:

```sh
python3 scripts/check_spike_environment.py
cargo build --workspace
python3 scripts/demo_control.py
```

Comandos manuales y evidencia en [Spike A](experiments/03-websocket-sync/README.md).
Consulta [CONTRIBUTING](docs/CONTRIBUTING.md) antes de proponer cambios; la licencia
sigue pendiente y no se solicitan aportes públicos ni se publica una release.
