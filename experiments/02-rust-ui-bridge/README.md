# 02 — Spike C: móvil, bridge y UI

Prototipo de ingeniería, **pre-v0.1**. Flutter continúa **provisional**;
desktop libmpv **PROVISIONAL FOR LINUX**; Android Media3 provisional;
iOS AVPlayer candidato investigado, sin runtime Apple.

## Objetivo y gate

Validar una frontera Application propia, el SyncEngine Rust original, ownership,
archivos móviles, surface y suspensión. Comparación antes de implementar en
[RESEARCH](RESEARCH.md); evidencia y límites en [RESULTS](RESULTS.md).
No sustituye el [vertical slice Linux](../06-real-vertical-slice/README.md).
No conecta WebSocket móvil ni decide Ready/autoridad de sala.

## Implementación

- [cine-ui-bridge](../../adapters/ui-bridge/README.md): C ABI experimental v1,
  JSON DTO, handles y buffers caller-owned. Rust posee Application/SyncEngine,
  ClockFilter, generación y worker hash; no expone structs Core como ABI.
- [Flutter app](app/README.md): pantalla mínima, Dart FFI para Rust y
  MethodChannel para SAF/Media3. Dart transporta intents/observaciones/efectos.
- Kotlin posee ExoPlayer Media3 1.11.1 y SurfaceView en main Looper. Player SDK
  nunca cruza FFI. La UI isolate recibe snapshots a 2 Hz, no decide drift.
- SAF abre content URI con permiso; Rust duplica FD regular, calcula SHA-256
  streaming (1 MiB) con progreso/cancelación/check de cambios; nativo cierra el
  original. No conversión de URI a path. Pipes/cloud providers quedan fuera.
- Suspend/resume invalida generation, deadline, confianza de reloj/muestras,
  cancela hash y exige snapshot. Offline clock/snapshot son **fixtures de ensayo**;
  recovery de red móvil sigue pendiente. Background pausa SDK/restaura rate.

## Entorno e instalación mínima

Se prepararon fuera de Git Flutter 3.47.6 (Dart 3.13.5), Temurin JDK 21,
Android command-line tools, platform-tools, API 36/build-tools 36.0.0,
NDK 28.2.13676358 y emulator Google APIs API 35 x86_64. Sin Android Studio.
Rust targets x86_64-linux-android/aarch64-linux-android/armv7-linux-androideabi. Ninja local para Linux;
GTK/CMake/toolchain ya existentes. No se instalaron mpv/VLC/GStreamer móviles.
Los binarios de SDK, medios generados y outputs están ignorados.

```sh
export CINE_SPIKE_TOOLS="$HOME/.local/share/cine-spike-tools"
export ANDROID_SDK_ROOT="$CINE_SPIKE_TOOLS/android"
export JAVA_HOME="$CINE_SPIKE_TOOLS/jdk21"
export PATH="$CINE_SPIKE_TOOLS/flutter/bin:$CINE_SPIKE_TOOLS/bin:$PATH"
rustup target add x86_64-linux-android aarch64-linux-android armv7-linux-androideabi
cargo build -p cine-ui-bridge
python3 scripts/build_mobile_bridge.py --abi x86_64
python3 scripts/build_mobile_bridge.py --abi arm64-v8a
python3 scripts/build_mobile_bridge.py --abi armeabi-v7a
```

El script de build verifica NDK y copia la biblioteca a jniLibs ignorado.
Crear un AVD API 35 con avdmanager o conectar un dispositivo autorizado. Solo un
endpoint adb para la demo. Android 9/API 28 en SM-J701M físico (armeabi-v7a) también fue probado con audio.
El emulador requiere KVM para este entorno.
En máquinas con poca RAM compilar antes de arrancar el AVD; Gradle se limita a
2 GiB/2 workers. El caller arranca y detiene el emulador que posee.

## Build y ejecución

Desde raíz:

```sh
python3 scripts/demo_mobile_ui.py --platform linux
python3 scripts/demo_mobile_ui.py --platform android
```

Si el APK/bundle ya fue compilado con `SPIKE_AUTORUN=true`, usar `--no-build`.
En emulador el APK automatizado usa `SPIKE_SILENT=true`: el ensayo final mide
clock/rate de vídeo sin AudioTrack; audio/percepción deben validarse en hardware.
El build manual deja audio habilitado. El intento anterior con audio se conserva
en [diagnostics](results-android-diagnostics.json), con sus ratios inconsistentes.
Android genera/reutiliza corpus con [generate_test_media](../../scripts/generate_test_media.py),
instala APK debug, coloca `normal.mp4` sintético en Downloads y opera el **selector
SAF real** con uiautomator; ejecuta hash + oracle Java, load/play/pause/seek/rate,
efectos SyncEngine y home/resume. Retira medio de prueba y force-stop al finalizar.
No serial, URI, usuario ni digest en JSON. Requiere pantalla desbloqueada; el harness reconoce DocumentsUI inglés/español.
No sobrescribe un archivo existente con el nombre de fixture. Logcat se filtra
desde el inicio del ensayo, no se borra el historial global del teléfono.

Manual, desde `app/`:

```sh
export CINE_BRIDGE_LIBRARY="$PWD/../../../target/debug/libcine_ui_bridge.so"
flutter run -d linux
flutter devices
flutter run -d DEVICE_ID
```

Para Android, seleccionar el ID local mostrado por `flutter devices` si hace
falta; el selector usa permiso SAF. La pantalla enseña archivo, posición,
duración, estado, hash y conexión Rust con Play/Pause/Seek +5 s. Ninguna UI final.
Linux valida la frontera/UI, **no vídeo embebido**.

## Pruebas y métricas

```sh
cargo test --workspace
cd experiments/02-rust-ui-bridge/app
CINE_BRIDGE_LIBRARY="$PWD/../../../target/debug/libcine_ui_bridge.so" flutter test
flutter analyze
```

[results-linux-ui.json](results-linux-ui.json), [results-android.json](results-android.json),
[teléfono Android 9](results-android-device.json) y [regresión Linux](results-linux-regression.json) contienen evidencia sanitizada.
Las mediciones separan dispatch de READY SDK; READY no certifica frame exacto.
Bridge: 1000 comandos State y snapshots a 2/10 Hz; roundtrip, no latencia
unidireccional de un evento push. No prueba 60 Hz, audio perceptual ni precisión
sincronizada móvil. SDK test destroyed es guard de acceso; no emula APIs Apple.

## Conclusión y siguiente validación

La frontera permite conservar SyncEngine/protocolo y adaptar SDKs distintos.
FRB/cbindgen permanecen alternativas si crecen API/streams; no se integran cuatro
motores. Próximo gate: completar proveedores SAF/lifecycle físico y precisión A/V prolongada,
Application de red + clock/snapshot y Mac/Xcode para AVPlayer. No promover stack
universal ni declarar v0.1 completado. ADR-006/007 preservan su historial y carácter
provisional. Licencia del proyecto pendiente; sin releases ni distribución pública.
