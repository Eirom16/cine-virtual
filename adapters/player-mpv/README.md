# Adapter libmpv experimental

In-process; implementa Player sin contaminar cine-core. Cargo no requiere SDK al
enlazar: libloading resuelve el API C cuando se crea el adapter. Solo Linux fue
probado. No es el motor definitivo, no hay soporte móvil validado ni empaquetado
redistribuible aprobado.

Código: [adapter](src/lib.rs), [FFI](src/ffi.rs),
[harness](src/measurements.rs), [CLI](src/bin/cine-player-spike.rs),
[tests opt-in](tests/real_player.rs).

Comandos, investigación, lifecycle, mediciones y límites:
[experimento 01](../../experiments/01-player-crossplatform/README.md).
