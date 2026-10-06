# Cine Virtual — Flutter client

Entrada normal: UI de producto Home → Create/Join → Lobby → archivo real →
Ready → Player. Flutter presenta snapshots/envía intents; Rust conserva sala,
autoridad y sincronización. Media3/SAF permanecen en Kotlin. Arquitectura en
[UI](../../../docs/UI.md), evidencia en [Phase 1](../../09-product-ui/RESULTS.md).

Desde la raíz del repositorio:

```sh
cargo build -p cine-ui-bridge
cd experiments/02-rust-ui-bridge/app
CINE_BRIDGE_LIBRARY="$PWD/../../../target/debug/libcine_ui_bridge.so" flutter run -d linux
```

Linux necesita libmpv y ffprobe; selección por diálogo GTK, vídeo en ventana
nativa independiente. Android necesita bridge para ABI y build APK mediante
`scripts/build_mobile_bridge.py`; selección SAF, sin paths manuales ni permiso
global de almacenamiento. Servidor en Configuración avanzada o CINE_SERVER.

Diagnósticos en menú Developer. Las pantallas de ingeniería siguen disponibles
con `--dart-define=SPIKE_AUTORUN=true` o `ROOM_AUTORUN=true` para scripts de
regresión; no son la experiencia normal. No publicar invitaciones/resume tokens.

Windows/macOS UI compila en CI, reproducción NOT TESTED/deshabilitada. iOS
compila unsigned, Player NOT IMPLEMENTED. libmpv y Media3 PROVISIONAL.
