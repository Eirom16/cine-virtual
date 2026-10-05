# Cine Virtual

Base de un proyecto con vocación open source para ver contenido juntos desde
Windows, Linux, macOS, Android e iOS. Cada dispositivo conserva su archivo y el
servidor coordina la sala; el control de reproducción no transporta el vídeo.

**Estado: vertical slice Linux + Spike C móvil/UI; recorrido Linux ↔ Android ejecutado con precisión móvil pendiente, previo al MVP v0.1.** Servidor WebSocket
y dos clientes CLI integran libmpv in-process, archivo local, metadata, SHA-256
completo, Ready, controles programados, correcciones y resume con snapshot.
FakePlayer y los instrumentos anteriores se conservan. La licencia está
pendiente: ver
[evaluación de licencias](docs/LICENSING.md). No se declara todavía una licencia
open source definitiva.

| Estado | Alcance real |
| --- | --- |
| Implemented | Core determinista, RoomService en memoria, codec v1, autoridad/Ready, controles programados y resume; FakePlayer y dos libmpv reales sobre WebSocket localhost, archivo/probe/hash locales y control Linux ↔ Android físico; el gate de precisión móvil sigue pendiente. |
| Planned | Completar validación móvil/dispositivos, UI de producto, packaging y hardening antes de v0.1. |
| Experimental | Spikes A/B, vertical slice Linux (libmpv PROVISIONAL) y Spike C: UI de ingeniería Flutter, C ABI/Rust y candidato Media3; evidencia y límites por plataforma en experimentos 02 y 07. |

El objetivo de v0.1 es demostrar dos clientes con el mismo vídeo local,
Play/Pause/Seek programados y recuperación tras una desconexión temporal.
Chat, distribución P2P, voz/cámara y providers son etapas posteriores, no
capacidades disponibles. El soporte de las cinco plataformas es un objetivo;
la evidencia de cada plataforma se detalla en [Spike C](experiments/02-rust-ui-bridge/RESULTS.md); no existe soporte universal demostrado.

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
Rust es la base inicial del Core. Axum/Tokio sostienen el canal de control; Flutter sigue
provisional. La estrategia multimedia debe completar validación móvil y
revisión de distribución antes de fijarse.

## Organización

```text
docs/          producto, arquitectura, protocolo, sincronización, seguridad,
               testing, roadmap, decisiones, contribución y licencias
client/        CLI experimental, réplica/scheduler genéricos, FakePlayer/proxy real
core/          biblioteca Rust sin dependencias externas y tests
rooms/         RoomService y RoomStore, sin WebSocket ni JSON
protocol/      DTOs wire y codec v1 validado
server/        ejecutable WebSocket con Axum/Tokio
adapters/      LocalMedia/probe/hash, libmpv y API Application/UI experimental,
               separados de Core
experiments/   planes, investigación y resultados de control/multimedia
scripts/       diagnóstico, demos/corpus/mediciones y validación de docs
```

Empieza por [PRODUCT](docs/PRODUCT.md), [ARCHITECTURE](docs/ARCHITECTURE.md) y
[PROTOCOL](docs/PROTOCOL.md). [SYNC](docs/SYNC.md) precisa el modelo temporal;
[DECISIONS](docs/DECISIONS.md) registra las elecciones y su estado.

## Validación local

Rust ≥1.89 para el workspace del spike (verificado con 1.99), Cargo, rustfmt y
Clippy; Python ≥3.10. Core conserva su mínimo 1.85 y ninguna dependencia externa.
Se necesita acceso a crates.io en la primera compilación y TCP localhost en las
pruebas WebSocket. Los tests habituales/build no necesitan SDK multimedia
instalado; ejecutar el Player y tests opt-in sí requiere libmpv.

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
# Linux: requiere libmpv y ffmpeg/ffprobe, genera corpus si falta, ~10 min reales
python3 scripts/demo_real_media.py
```

Comandos manuales y evidencia en [Spike A](experiments/03-websocket-sync/README.md),
[Spike B](experiments/01-player-crossplatform/README.md) y
[vertical slice 1](experiments/06-real-vertical-slice/README.md) y
[Spike C móvil/UI](experiments/02-rust-ui-bridge/README.md) y
[vertical slice 2 Linux ↔ Android](experiments/07-linux-android-room/README.md).
Consulta [CONTRIBUTING](docs/CONTRIBUTING.md) antes de proponer cambios; la licencia
sigue pendiente y no se solicitan aportes públicos ni se publica una release.
