# CI multiplataforma — pre-v0.1

**IMPLEMENTED AND EXECUTED ON GITHUB.** La primera ejecución hosted se analizó
mediante GitHub CLI en `Eirom16/cine-virtual`; su evidencia y las ejecuciones de
corrección se registran en [CI-HOSTED](CI-HOSTED.json). Build y runtime son gates
separados. El gate de precisión Android del
[slice 2](../experiments/07-linux-android-room/RESULTS.md) queda intacto: este
trabajo no modifica SyncEngine, Media3 timing ni protocolo. No signing Apple,
secrets, stores o releases; los pushes de diagnóstico fueron autorizados.

## Workflows y triggers

[ci.yml](../.github/workflows/ci.yml) es el único orquestador automático:
`pull_request`, push a `master`/`main` y `workflow_dispatch`. El job base Ubuntu
hace fmt/clippy/tests/build Rust, documentación, validación CI, pruebas de scripts,
Flutter pub get/analyze/test; **los builds costosos requieren base PASS**.

Cinco workflows separados declaran `workflow_call` y `workflow_dispatch`: [Linux](../.github/workflows/build-linux.yml),
[Windows](../.github/workflows/build-windows.yml),
[macOS](../.github/workflows/build-macos.yml),
[Android](../.github/workflows/build-android.yml) e
[iOS](../.github/workflows/build-ios.yml). No tienen otros triggers automáticos,
por lo que no duplican el orquestador. El despacho directo declarado en cada reusable no incluye el gate global;
la ruta manual operativa mediante CI/platform sí conserva la base.
El API hosted solo registró CI: view/dispatch directo de build-linux.yml devolvió
404 aunque el archivo existe en master. Por ello CI expone input manual `platform`
(all/linux/windows/macos/android/ios). Esa ruta usa el workflow registrado y
conserva el gate base; los no solicitados se muestran NOT APPLICABLE, nunca PASS.
No se alteraron settings para forzar registro de los workflows reutilizables.
Concurrency cancela runs obsoletos de esa rama/PR; matrices fail-fast=false dejan
visibles las arquitecturas restantes. No hay continue-on-error ni deployments.

No se filtran paths todavía: cambios de docs también pueden afectar los contratos
que deben validar los builds. Push solo a ramas principales evita duplicar cada
push de rama de trabajo y su PR. Hay una tabla por build y un summary global con
los resultados reales, incluso si base falla y los builds quedan BLOCKED.

## Runners investigados

Labels explícitos, verificados en documentación oficial el 2026-10-05:

| Plataforma | Label elegido | Arquitectura |
| --- | --- | --- |
| Linux/Android | ubuntu-24.04 | x64 |
| Windows | windows-2025 | x64 |
| macOS | macos-15 | arm64 |
| macOS | macos-15-intel | x64 |
| iOS device/simulator | macos-15 | host arm64 |

GitHub también publica macOS 26 y otras imágenes; evitamos `*-latest` y previews.
Labels explícitos no congelan el contenido semanal de la VM. Registrar OS, arch,
Xcode/Rust/Flutter en logs/metadata y revisar los primeros runs. No se descarga ni
selecciona Xcode arbitrariamente: se utiliza el instalado/seleccionado en la imagen.

## Herramientas, locks y cache

[toolchains.json](../.github/ci/toolchains.json) registra Rust 1.99.0, Flutter
3.47.6/Dart 3.13.5 y commit Flutter
5fc346839b5d0eef006ed8404392afb4dfae428d, Python 3.12, Temurin JDK 21,
Android API 36/build-tools 36.0.0/NDK 28.2.13676358. Se conserva la versión del
proyecto; el tag Flutter oficial se comprobó contra ese commit. El SDK se obtiene
por Git oficial y se verifica HEAD + versión Dart/Flutter, sin cambiar de canal.
El endpoint de archivo de releases devolvió 404 desde este entorno; no se inventó
un URL/checksum de SDK. Git por revision es la estrategia elegida para CI.

[rust-toolchain.toml](../rust-toolchain.toml) fija Rust sin cambiar los mínimos
Cargo. Cargo usa --locked, pub get --enforce-lockfile y pubspec.lock existente.
Gradle/AGP/Kotlin/Media3 conservan las versiones del proyecto. No cargo update,
flutter upgrade o gradle upgrade. [setup](../.github/actions/setup/action.yml)
y [ci_setup.py](../scripts/ci_setup.py) preparan herramientas. Instalar toolchains/targets de forma
secuencial antes de lanzar builds locales paralelos (rustup no serializa todas
las instalaciones concurrentes). El build Android
reutiliza [build_mobile_bridge.py](../scripts/build_mobile_bridge.py), Linux host
NDK explícito; no se presenta ese script como build Android desde cualquier SO.

Caches separadas por OS/arch/locks: Cargo registry/git, SDK Flutter por revision,
pub y Gradle caches/wrapper por ABI/configuración. **No se cachea target ni outputs
finales**. No claves de signing/credenciales; sin restore-key amplio. Un cache miss
debe instalar y construir de cero. Imágenes, apt/Homebrew y SDKs externos no son
un build hermético; reproducibilidad bit a bit no está demostrada.

## Builds y límites por plataforma

Linux instala clang/CMake/Ninja/pkg-config/GTK/libstdc++, libmpv-dev, libepoxy-dev
y ffmpeg. El runner Linux enlaza Render API y epoxy; el adapter Rust conserva
carga dinámica. Los headers nuevos pertenecen al mismo SDK libmpv existente. Ubuntu usa apt, no pacman. Compila workspace release,
CLI y bundle Flutter con bridge .so dentro de lib/. Los seis tests SDK decodifican
corpus sintético con outputs null: no equivalen a display/audio físico validado.
El test de lease y el test C++ puro verifican ownership/estado; frames y Wayland
se validan físicamente en experimento 10, fuera de hosted CI.

Windows compila workspace/tests y Flutter runner nativo con DLL Rust junto al exe.
El adapter libmpv usa carga dinámica: no requiere un SDK en link-time. **SDK mpv
Windows y sus dependencias no se provisionan/bundlean**: no hay packaging DLL
multimedia auditado. Compile PASS no prueba load/video. No se deshabilita el
adapter para esconder un blocker; runtime mpv fallaría explícitamente sin SDK.

macOS usa dos VMs nativas separadas, arm64/x64, compila workspace/tests y app
Flutter con dylib Rust en Contents/Frameworks. ARCHS explícito arm64/x86_64 y verificación lipo
impiden etiquetar un universal binary no demostrado. Homebrew documenta mpv para
ambas arquitecturas, pero **este CI no lo instala**: no hay runtime/packaging
multimedia Apple validado y no hace falta para compilar el loader. App Flutter
experimental es bridge/UI aislados; aún no integra surface libmpv desktop.

Android construye puente Rust arm64-v8a, armeabi-v7a y x86_64 y APK debug por ABI,
con pantalla ROOM_MODE, Media3 y SAF existentes. No autorun/configuración privada.
JDK/SDK/NDK se preparan explícitamente. Debug APK usa la clave SDK efímera estándar,
no una clave release; no se produce AAB ni se publica. Rust/Flutter tests host son
la base compartida; no se confunden con ejecutar tests Rust dentro de Android.
No job de emulador: el smoke previo requiere SAF, lifecycle y dispositivo/config
locales; incorporarlo automáticamente ahora daría una validación distinta/no
fiable. No pruebas físicas ni 10 minutos por push. Gate separado futuro.

### iOS unsigned, device y simulator

Rust staticlib usa targets reales aarch64-apple-ios y aarch64-apple-ios-sim, en dos
jobs ARM macOS. Device: `flutter build ios --release --no-codesign --no-pub`.
Simulator: `flutter build ios --debug --simulator --no-codesign --no-pub`.
No Intel simulator porque el host elegido es ARM y no se necesita x86_64 para ese
gate. deployment mínimo del scaffold Flutter actual: iOS 15.

El Xcode project enlaza la staticlib con force_load/exported C ABI symbols. Dart
usa DynamicLibrary.process(), nunca intenta cargar una .so Android. Se comprueban
los cuatro símbolos C en el ejecutable. `CODE_SIGNING_ALLOWED=NO`, sin team,
certificado, perfil, Apple ID, notarization, fastlane o secrets. El empaquetador
rechaza firmas con certificado. Si el linker Simulator genera una firma ad-hoc
automática, la retira y comprueba de nuevo que la app quedó unsigned; metadata
registra esa retirada. No se aplica ninguna firma ni se usa identidad Apple.

**IOS PLAYER RUNTIME NOT IMPLEMENTED** se muestra explícitamente; no botones de
playback ni FakePlayer presentado como vídeo. AVPlayer sigue candidato investigado.
La pantalla solamente consulta el bridge. La evidencia de compile/linking hosted se registra por target en CI-HOSTED;
no se ha ejecutado la aplicación en iPhone ni Simulator.

Artifact contiene Runner.app compilado y staticlib, no .ipa, xcarchive ni IPA
instalable. Device .app sin firma no implica instalación posible en iPhone. No se
arranca Simulator; simulator build comprueba compile/linking, no runtime.

## Diagnóstico del primer run hosted

Run `37383775762`, commit `17d1a38`: base, Android tres ABIs, macOS Intel e iOS
device pasaron. Linux, Windows, macOS ARM64 e iOS Simulator fallaron; los logs
completos se leyeron con gh antes de corregir. No se ocultaron failures.

- Linux ya compilaba Rust/Flutter. Los tests SDK asumían duración exacta de
  30000/620000 ms (incluida otra aserción en el SDK del cliente que el primer
  run no alcanzó a ejecutar); el corpus Ubuntu reportó 30021/620021 ms por redondeo de
  contenedor/AAC. Se conserva una tolerancia acotada de frames y se comprueba
  SeekOutOfRange contra duration real + 1, sin modificar thresholds de sync.
- Windows compilaba Rust, pero el guard de metadata/test dependía de actualización
  inmediata del timestamp. El adapter ahora lee file ID, volumen y ChangeTime
  mediante Win32; el test fija explícitamente una modificación observable.
  Los timestamps no son identidad ni garantía contra escrituras adversariales:
  SHA-256 completo continúa siendo la identidad final. Error al consultar metadata
  Windows falla explícitamente. Se validó compile GNU local y el gate MSVC hosted
  sigue siendo la evidencia nativa relevante.
- macOS ARM64 heredaba NATIVE_ARCH_ACTUAL=arm64e, ausente en FlutterMacOS.
  El script pasa FLUTTER_XCODE_ARCHS=arm64 o x86_64; lipo conserva la comprobación
  estricta de app, bridge y CLI. El default local es ARCHS_STANDARD.
- Xcode terminó la app Simulator pero faltaban símbolos en Runner. Su layout
  debug dylib mueve código fuera del ejecutable principal. Se desactiva ese layout
  mediante FLUTTER_XCODE_ENABLE_DEBUG_DYLIB=NO para el bridge in-process; los cuatro
  símbolos se siguen comprobando, sin confundir device con Simulator ni firmar.
  El siguiente run reveló una firma ad-hoc automática al empaquetar Simulator:
  solo se retira si está identificada como ad-hoc sin Authority/Team, luego se
  comprueba unsigned de nuevo. Firmas device/certificado o errores fallan.

Fuentes del diagnóstico: [metadata Win32](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_basic_info),
[layout debug Xcode](https://developer.apple.com/documentation/xcode/understanding-build-product-layout-changes)
y fuente flutter_tools fijada (environmentVariablesAsXcodeBuildSettings acepta
FLUTTER_XCODE_*). La evidencia por run y los artifacts se conserva en CI-HOSTED.

Una regresión intermitente Intel del run 37387906249 rechazó MEDIA_SELECT_REQUEST
con OUT_OF_SEQUENCE: el fixture enviaba el control antes de que Host recibiera
MEMBER_JOINED. Se añadió wait_state sobre la secuencia de Join, sin retries ni
cambios de protocolo/runtime. El test pasó cinco repeticiones locales completas.
La metadata registra source_status con nombres relativos al repositorio para
hacer visibles cambios generados: el paquete Windows de c206eae declaró dirty,
no se falsificó ese campo ni se presentó como build hermético. El status del
run d22127a identifica siete registrantes de plugins generados por Flutter
(Linux/macOS/Windows), no cambios en Core o protocolo. Normalizar/auditar esos
outputs es deuda de reproducibilidad, no evidencia de runtime Windows.

## Artifacts, integridad y metadata

[ci_build.py](../scripts/ci_build.py) separa Rust, Flutter, empaquetado y summary.
Artifacts de Actions duran 7 días, con nombre platform/arch/variant/commit corto.
Desktop: tar.gz Linux/macOS o ZIP Windows, CLI y Flutter bundle/bridge. Android:
ZIP por ABI que contiene APK debug. iOS: tar.gz unsigned device/simulator con .app
+ staticlib. No se redistribuyen libmpv/FFmpeg ni corpus en artifacts.

Cada archivo final tiene CHECKSUMS.txt SHA-256 y metadata.json: commit completo,
runner OS/arch/version, Rust/Flutter/Dart, Xcode o Android SDK/NDK/JDK, modo,
source_status relativo al repositorio (no contenido/diff) y
signed=false iOS. Paths de SDK se eliminan de metadata. SHA de artifact no tiene
relación con la identidad multimedia de salas. Tar conserva modos/symlinks Apple;
Actions no se usa para publicar GitHub Releases ni stores.

La licencia Cine Virtual sigue pendiente. Outputs experimentales de CI no son
aprobación legal de redistribución; configurar repositorio/visibilidad y resolver
licencia antes de publicación pública. No se creó LICENSE.

## Actions y seguridad

Solo acciones oficiales GitHub, fijadas a SHA completo consultado el 2026-10-05:

| Acción | Versión | SHA |
| --- | --- | --- |
| actions/checkout | v7.0.1 | 3d3c42e5aac5ba805825da76410c181273ba90b1 |
| actions/upload-artifact | v7.0.1 | 043fb46d1a93c77aae656e7c1c64a875d1fc6a0a |
| actions/cache | v6.1.0 | 55cc8345863c7cc4c66a329aec7e433d2d1c52a9 |
| actions/setup-python | v7.0.0 | 5fda3b95a4ea91299a34e894583c3862153e4b97 |
| actions/setup-java | v6.0.1 | de7274f081f381c8f8158605e0321c36c376e2e6 |

Permissions contents:read; checkout persist-credentials=false. No secrets ni
pull_request_target; PRs corren en VMs hospedadas, no en la laptop/dispositivo.
No interpolar título/branch/contenido de PR como comandos; summary usa resultados
JSON en env. Actualizaciones de Actions/tools deben ser cambios revisados.

## Significado de PASS y matriz de evidencia

PASS de build significa comando completado y paquete/símbolos requeridos presentes.
No significa Player/runtime, precisión, distribución, firma o aprobación de stores.
FAIL y BLOCKED permanecen visibles. Per-stage summaries separan Core/bridge,
Flutter, Player compile, package y Runtime NOT TESTED.

Snapshot hosted: run `37389305915`, commit `d22127a`, **todos los builds PASS**.
La evidencia machine-readable conserva también los runs fallidos anteriores.

| Plataforma | Rust Core/bridge | Flutter | Player build | Package | Runtime |
| --- | --- | --- | --- | --- | --- |
| Linux x64 | PASS hosted | PASS hosted | Loader PASS; seis SDK headless PASS | PASS hosted | SDK headless; display/audio NOT TESTED por CI |
| Windows x64 | PASS hosted | PASS hosted | Loader COMPILE PASS; SDK no bundled | PASS hosted | NOT TESTED |
| macOS arm64/x64 | PASS hosted ambos | PASS hosted ambos | Loader COMPILE PASS; SDK no bundled | PASS hosted ambos | NOT TESTED |
| Android 3 ABIs | Bridge PASS hosted | APK debug PASS hosted | Media3 COMPILE PASS | PASS hosted x3 | NOT TESTED por CI; físico previo separado |
| iOS arm64 device/sim | Bridge PASS hosted ambos | PASS hosted ambos | NOT IMPLEMENTED | Unsigned PASS hosted ambos | NOT TESTED |

Localmente en diagnóstico: 90 tests Rust, seis SDK opt-in, seis Flutter y seis
tests de scripts CI; fmt/clippy/docs/check_ci/actionlint y demos FakePlayer/Linux
real pasaron. El test WebSocket que falló por ordering se repitió cinco veces.
Compile/clippy de LocalMedia Windows GNU desde Linux pasó; el runner Windows
MSVC es la evidencia nativa de workspace y Flutter, no ese cross-check.

[CI-LOCAL](CI-LOCAL.json) conserva la pasada local histórica y añade las
verificaciones de diagnóstico. [CI-HOSTED](CI-HOSTED.json) registra commit/run,
plataforma, paquete, checksums y runtime separado. Los artifacts originales
locales de 17d1a38 no se relabelan como builds hosted.

## Reproducir y ejecutar manualmente

```sh
# Ruta registrada y comprobable; default all si se omite el input.
gh workflow run ci.yml -f platform=linux
gh run list --limit 5
gh run view RUN_ID
```


```sh
python3 -m venv /tmp/cine-ci-tools
/tmp/cine-ci-tools/bin/pip install -r .github/ci/requirements.txt
/tmp/cine-ci-tools/bin/python scripts/check_ci.py
python3 -m unittest discover -s scripts/tests -v
cargo test --workspace --locked
cargo build --workspace --locked
# Flutter existente del proyecto en PATH:
python3 scripts/ci_build.py flutter-checks
python3 scripts/ci_build.py rust --platform linux
python3 scripts/ci_build.py flutter --platform linux
python3 scripts/ci_build.py package --platform linux --arch x64
```

setup es opcional local si las versiones ya coinciden. ANDROID_SDK_ROOT y JAVA_HOME
se suministran al reutilizar Android scripts. Apple requiere Mac/Xcode y Windows
su SDK/Visual Studio real; no emular su PASS desde Linux.

Después del push autorizado por el propietario: Actions → CI → Run workflow, o
CI → Run workflow → platform para diagnóstico individual. Workflows deben
existir en rama por defecto para despacho UI. El remoto fue conectado por el propietario y los pushes de diagnóstico están
autorizados. No se configura publicación ni settings sensibles del repositorio.

Checklist de cada nueva ejecución:

1. Base: fmt/clippy/90 tests/seis Flutter/docs/check_ci verdes; locks sin modificaciones.
2. Linux: apt, seis SDK tests headless, tar/checksums/metadata/bundle.
3. Windows: MSVC Rust + CMake Flutter; DLL bridge presente, mpv runtime no validado.
4. macOS: ambos runners, Xcode y arch registrados; lipo confirma artifact separado.
5. Android: tres ABIs, SDK/NDK/JDK registrados; ZIP/APK de cada ABI.
6. iOS: device/sim, Rust targets/símbolos enlazados; .app unsigned, Player explícito.
7. Summary y artifacts: checksums coinciden; no confundir package con runtime.

Si falla: devolver URL/run/commit, job/arquitectura, primer step fallido, error
completo del compiler/linker y metadata/summary si existen. No enviar tokens,
certificados, serial/URI/path personal ni secretos. No ocultar fallos experimentales.

## Fuentes primarias consultadas

- [GitHub runners y labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
- [Workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax),
  [reusable workflows](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows),
  [cache](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching),
  [secure use/pinning](https://docs.github.com/en/actions/reference/security/secure-use).
- [Checkout](https://github.com/actions/checkout), [cache](https://github.com/actions/cache),
  [upload-artifact](https://github.com/actions/upload-artifact),
  [setup-python](https://github.com/actions/setup-python), [setup-java](https://github.com/actions/setup-java):
  releases y Git refs oficiales para verificar las versiones/SHA indicadas.
- [Flutter archive](https://docs.flutter.dev/install/archive),
  [tag fuente 3.47.6](https://github.com/flutter/flutter/tree/3.47.6),
  [Linux](https://docs.flutter.dev/platform-integration/linux/setup),
  [Windows](https://docs.flutter.dev/platform-integration/windows/setup),
  [macOS](https://docs.flutter.dev/platform-integration/macos/setup),
  [iOS builds](https://docs.flutter.dev/deployment/ios),
  [FFI legacy/static linking](https://docs.flutter.dev/platform-integration/legacy-ffi-plugin).
- [Rust Apple iOS targets](https://doc.rust-lang.org/rustc/platform-support/apple-ios.html).
- [Homebrew mpv](https://formulae.brew.sh/formula/mpv): posibilidad técnica, no runtime probado aquí.

## Product UI Phase 1

La entrada normal del mismo app Flutter es ahora CineVirtualApp; los autoruns
SPIKE_AUTORUN/ROOM_AUTORUN conservan la ingeniería. El gate Flutter cubre el
producto y las pruebas FFI previas con CINE_BRIDGE_LIBRARY. ROOM_MODE usado por
builds anteriores es compatible y ya no cambia la entrada normal. El nuevo
owner desktop reutiliza Client/libmpv; no se cambia la matriz ni sus permisos.
Resultados locales/físicos y run final en
[Product UI Phase 1](../experiments/09-product-ui/RESULTS.md). Build Windows/macOS
no valida multimedia; iOS permanece unsigned con Player NOT IMPLEMENTED.
