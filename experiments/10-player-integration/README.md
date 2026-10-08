# Experimento 10 — Player integration & recovery

Objetivo: vídeo libmpv integrado en Flutter Linux, fullscreen y recuperación
Android conservando Client, protocolo y SyncEngine. No funciones sociales.

Base: master cc1116d limpio, CI 37538977527 SUCCESS. Entorno: KDE Wayland.
Auditoría, alternativas y fuentes: [embedding-spike](embedding-spike.md).
Evidencia: [RESULTS](RESULTS.md), [Linux](linux-results.json),
[Android](android-recovery-results.json) y [screenshots](screenshots/).

## Reproducción Linux

Requiere GTK3, SDK libmpv/epoxy, EGL compartible y toolchain Flutter fijado en CI.
El adapter Rust sigue cargando SDK dinámicamente; el runner Linux enlaza Render
API. No se empaquetan bibliotecas multimedia en el bundle experimental.

```sh
cargo build --workspace --locked
python3 scripts/generate_test_media.py
export CINE_BRIDGE_LIBRARY="$PWD/target/debug/libcine_ui_bridge.so"
cd experiments/02-rust-ui-bridge/app
flutter build linux --debug
GDK_BACKEND=wayland build/linux/x64/debug/bundle/cine_mobile_spike
```

Servidor local: `cargo run -p cine-server -- --bind 0.0.0.0:8765 --allow-lan`.
Usar IP LAN alcanzable por ambos dispositivos al crear sala. No adb reverse.
Create/Join → archivo sintético → hash → Ready → Player. Probar Space, flechas,
F, Esc, mouse, timeline, participantes, resize lento/rápido, Lobby/Player y
Developer disconnect/retry/Ready. Participant no puede emitir controles Host.

Spike aislado: ejecutar el mismo bundle con CINE_EMBEDDING_SPIKE apuntando al
archivo sintético local. Ruta de ingeniería explícita; no sustituye Client ni
actúa como renderer de producto. Play/Pause/Seek y destroy/recreate se ensayan
antes de conectar el render a la sesión real.

## Reproducción Android

Construir bridge y APK del ABI real (`scripts/build_mobile_bridge.py`); instalar
APK debug. Seleccionar mediante SAF la misma copia sintética de Linux. Host en
Android, Participant Linux, ambos Ready. Play hasta 30–60 s (o seek explícito a
una posición intermedia y reanudar). Home → esperar 5/15/30 s → volver a la app.
Repetir cinco veces mientras Playing, lejos del EOF. Registrar generación,
room/member, revisión de medio, hash, Ready, posiciones local/target, error y
correcciones. Revalidation conserva selección y timeline; no afirma process death.

Lock/unlock es opcional y requiere desbloqueo manual si hay keyguard seguro.
No se solicita ni se automatiza PIN. Un ensayo sin alcanzar foreground se
clasifica BLOCKED, separado de los cinco ensayos de Home/foreground.

## Observaciones y límites de evidencia

`observe_product.py URL_VM` lee snapshots del debug VM y emite una allowlist sin
credenciales/rutas/digest. Android requiere adb forward solo para su VM; WS va
por LAN. No guardar URL de VM en resultados. El helper depende del object model
del build fijado; no es API de producto ni prueba visual de frames.

Capturas son ventanas reales, corpus sintético; no mockups. La de 1920×1080 usa
una ventana Wayland sobredimensionada en monitor físico 1366×768; no equivale a
un segundo monitor. Tamaños incluyen decoración cuando la ventana no es fullscreen.
Mediciones de /proc en debug son aproximadas, CPU % de un núcleo, muestras 10 s;
no p95 ni benchmark de decoder/resolución alta. Los snapshots de dos dispositivos
se leen secuencialmente: no usarlos para estimar drift entre sus capturas.
