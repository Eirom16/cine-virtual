# Experimento 11 — Social Experience Phase 1

Base verificada: master 318272e, limpio/sincronizado; CI inicial 37719928797 SUCCESS.
Objetivo: chat y reacciones reales sobre la sesión existente Linux/libmpv ↔
Android/Media3, sin cambiar SyncEngine ni protocolo multimedia.

Contrato y límites: [PROTOCOL](../../docs/PROTOCOL.md), ADR-012 en
[DECISIONS](../../docs/DECISIONS.md), presentación en [UI](../../docs/UI.md).
Resultados separados de implementación/build/runtime en [RESULTS](RESULTS.md).

## Reproducir

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
python3 scripts/check_docs.py
python3 scripts/check_ci.py
python3 -m unittest discover -s scripts/tests -v
```

Flutter usa el SDK fijado y CINE_BRIDGE_LIBRARY apuntando a target/debug/libcine_ui_bridge.so;
flutter analyze y flutter test desde experiments/02-rust-ui-bridge/app.
Builds/dispositivos conservan [instrucciones de Phase 2](../10-player-integration/README.md).
Crear sala con endpoint LAN controlado, Alex Host Linux, Sam Participant Android;
seleccionar el corpus sintético por picker/SAF, verificar y Ready en ambos.
Chat en Lobby, Play, abrir Chat sin desmontar vídeo, intercambiar texto y usar
picker. Repetir en fullscreen/portrait/landscape, Pause/Seek/Play y reconnect.
Android Home, Host envía texto, foreground recupera historial y Player.
No usar datos personales ni incluir invitación, URI, path, SHA o debug VM URL en evidencia.

Las pruebas de UI y bindings no equivalen a frames reales. Capturas deben proceder
de ventanas/dispositivo y revisarse visualmente; identificar bloqueos de dispositivo
sin inventar PASS. No se recalibra p95 multimedia ni se añaden GIF/voz/vídeo.
