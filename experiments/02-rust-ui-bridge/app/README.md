# Cine Mobile Spike — pantalla de ingeniería

Prototipo aislado de Spike C, no UI de producto ni cliente de sala.
Flutter presenta snapshots; Rust decide SyncEngine/lifecycle; Media3 y SAF viven
en Kotlin/main Looper. Fuente/evidencia en [experimento](../README.md).

```sh
# Desde la raíz: compila el C ABI; Flutter requiere la variable local del .so.
cargo build -p cine-ui-bridge
python3 scripts/demo_mobile_ui.py --platform linux
python3 scripts/demo_mobile_ui.py --platform android
```

Manual Android: build sin SPIKE_AUTORUN y flutter run; seleccionar mediante SAF,
Play/Pause/Seek +5s. Resultados automáticos omiten URI/nombre/digest/serial.
Linux solo conecta Rust, sin vídeo embebido. iOS no está generado ni validado.
No servicios background, permisos de almacenamiento globales ni servidor remoto.
