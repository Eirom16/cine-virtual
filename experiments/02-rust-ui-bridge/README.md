# 02 — Puente Rust/UI

Estado: plan, sin bindings. Evaluar Flutter candidato frente a un harness nativo;
comparar C ABI/FFI, bridge generado y canales de plataforma cuando correspondan.

Demostrar: comando tipado, stream de snapshots, eventos Player, cancelación de
hash, ownership/lifetime, errores redactados y liberación al salir/reconectar.
No pasar objetos de SDK multimedia ni punteros de UI al dominio. Superficie de
vídeo puede pertenecer al lado nativo.

Medir tiempo de build, complejidad de packaging por los cinco OS, dispatch/hilos,
coste de copias y mantenimiento. Probar Android/iOS con handles de archivo y
thread principal; desktop con teardown repetido. Resultado: API pequeña de
Application y ADR-007 confirmado/reemplazado, sin framework en cine-core.
Ver [ARCHITECTURE](../../docs/ARCHITECTURE.md).
