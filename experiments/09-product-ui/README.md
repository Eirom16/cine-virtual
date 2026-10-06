# Experimento 09 — Product UI Phase 1

## Objetivo

Convertir el cliente Flutter existente en una experiencia de producto real:
Home → Create/Join → Lobby → archivo → hash/verificación → Ready → Player →
Play/Pause/Seek. No nueva implementación de protocolo, autoridad o sync.

Arquitectura y auditoría: [UI](../../docs/UI.md). Resultado/evidencia:
[RESULTS](RESULTS.md). El app permanece en `../02-rust-ui-bridge/app`.

## Reproducir el smoke manual

1. Generar corpus con `python3 scripts/generate_test_media.py` y compilar Rust.
2. Ejecutar `cine-server` local/LAN con `--allow-lan` solo si corresponde.
3. Compilar/iniciar Flutter Linux con CINE_BRIDGE_LIBRARY apuntando al .so.
4. Home → Crear sala; nombre y servidor en Configuración avanzada.
5. Seleccionar `long-duration.mp4` por diálogo; esperar hash/carga y Ready.
6. Copiar invitación; abrir Android, Unirse, nombre/invitación completa.
7. Seleccionar la misma copia sintética por SAF, esperar y confirmar Ready.
8. Host Start, Pause, Play y Seek; comprobar que Participant refleja controles
   reales y no puede emitir autoridad. Repetir con Android como Host.
9. Background/foreground Android, verificar recuperación sin crear otra sala.
10. Capturar únicamente app/superficie propia, sin picker, tokens, endpoint
    privado, seriales, paths ni archivos personales. Revisar screenshots.

`SPIKE_AUTORUN` y `ROOM_AUTORUN` son regresiones de ingeniería separadas; no
sustituyen el recorrido manual normal. Mocks solo en widget tests.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
python3 scripts/check_docs.py
python3 scripts/check_ci.py
# app/: requiere bridge compilado para los tests FFI anteriores
CINE_BRIDGE_LIBRARY=/ruta/target/debug/libcine_ui_bridge.so flutter analyze
CINE_BRIDGE_LIBRARY=/ruta/target/debug/libcine_ui_bridge.so flutter test
```

Linux requiere libmpv/ffprobe. Android bridge por ABI con
`scripts/build_mobile_bridge.py`, SDK/JDK existentes. iOS build unsigned no
implica runtime; Windows/macOS multimedia no se declara validado.
