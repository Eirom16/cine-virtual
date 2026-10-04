# Puente Application experimental — Spike C

API C v1 + JSON DTO propios, preparada para Dart FFI y otro consumidor nativo.
No es ABI pública definitiva. [Research](../../experiments/02-rust-ui-bridge/RESEARCH.md)
y [experimento](../../experiments/02-rust-ui-bridge/README.md) describen el gate.

Rust posee SyncEngine original, ClockFilter, generation/lifecycle y un deadline
local de prueba. Native Android posee Media3/SurfaceView/SAF, exclusivamente en
main Looper. Dart presenta y transporta intents/efectos; no decide drift ni roles.
Este prototipo aislado no contiene RoomService, WebSocket ni Ready de sala.

## ABI y DTOs

[Header](include/cine_bridge.h): create/destroy/call/hash_fd. Handles numéricos
internos, máximo 16 instancias; buffers caller-owned, entrada ≤64 KiB y salida
con capacidad ≥64 KiB obligatoria antes de mutar. Retorno positivo es longitud;
negativos son error ABI. FFI exige punteros válidos/no solapados: el caller
controla memoria y es código del mismo proceso, no una frontera de seguridad.

```json
{"api_version":1,"generation":1,"command":{"type":"seek","payload":{"position_ms":5000}}}
```

Envelope/version/enums/required fields/rangos se validan con serde + checks.
Reply incluye ok/error.code/message, generación, capabilities, muestra y efectos
(action/value/generation); no structs Core ni SDK como ABI. generation invalida
callbacks/comandos anteriores. Errores tipados de Application se normalizan en
Dart; SDK strings y rutas no atraviesan Core. Capabilities mínimas son
playback_rate/content_uri_input; precisión y hardware se observan, no se prometen.

No hay stream push: la UI consulta snapshots a 2 Hz; 10 Hz solo ensayo. Eventos
SDK nativos siguen fuera del Core. Effect confirma solicitud, no completion.
El loop nativo observa READY/eventos y first-frame por separado. No equivale
seekTo retornado a frame presentado. No existe scheduler autoritativo móvil aún.

## Hash y ownership

hash_fd duplica un FD prestado de ContentResolver. La capa nativa cierra el
original; Rust cierra su duplicado mediante OwnedFd/File. Solo FD regulares,
rewindable; pipes/cloud streams se rechazan por cancelación bloqueante no resuelta.
Worker de std::thread reutiliza hash_reader (1 MiB), comprueba size/mtime/ctime
antes/después, publica progreso y solo digest final. cancel/suspend usa token
cooperativo. destroy cancela/join fuera del mutex de registry. Regular-file read
puede tardar: producción deberá destruir fuera del UI isolate o usar close async.
Un hash y 1 MiB de buffer por instancia, no base de datos/cache por URI/mtime.
Digest permanece local y nunca se exporta a logs/resultados de este spike.

Mutex de registry solo cubre comandos/DTO pequeños, nunca lectura/hash ni SDK.
Ningún objeto !Send/!Sync cruza FFI, no unsafe Send/Sync. Threads de SDK los
posee Android. Completion de hashing se observa por snapshot, con estado
running/complete/cancelled/modified/read_failed.

Suspend/resume/error cambia generation, cancela deadline/hash, borra muestras,
ClockFilter y confianza, restaura rate y pausa. Resume mantiene snapshot_required;
no reanuda solo. El escenario offline calibra con fixtures locales y marca un
snapshot de prueba: **no constituye clock sync ni snapshot de red móvil**.
Deadline adicional de ensayo tiene rango ≤2 s; no reemplaza scheduled controls v1.

## Validación

```sh
cargo test -p cine-ui-bridge -p cine-local-media
cargo build -p cine-ui-bridge
python3 scripts/build_mobile_bridge.py --abi x86_64
python3 scripts/build_mobile_bridge.py --abi armeabi-v7a
```

El build Rust Linux no requiere Flutter/Android SDK ni mpv. Unix FD es adapter
local; en Windows hash_fd retorna unsupported, sin contaminar Core. Windows,
macOS e iOS todavía no compilados/ejecutados. Guardrails/pruebas en
[tests](tests/boundary.rs). ABI manual necesita revisión unsafe, versionado y
comparar codegen si la API crece; registry poison/panic no es recuperación de SDK.
