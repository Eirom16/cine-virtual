# cine-core

Biblioteca Rust de fundaciones, std únicamente. `cargo test --workspace --offline`
desde la raíz. Rust ≥1.85. No es una aplicación ni implementa toda la sala.

| Módulo | Funcionalidad actual |
| --- | --- |
| clock | Estimación T1–T4 y filtro de últimas ocho muestras |
| playback | Proyección de timeline y selección current/pending según tiempo |
| sync | Decisiones de drift con thresholds, rate, seek, histéresis y cooldown |
| replica | Secuencia/epoch, duplicados, gaps y recuperación con snapshot |
| media | Descriptor local portable, límites básicos y comparación tamaño/digest |
| player | Trait de dispatch; adapter falso en tests |

Tipos son API Rust interna, **no** schema JSON ni ABI FFI. `Timeline`/`MediaDescriptor`
construidos por adapters deben validarse antes de uso; structs públicos no impiden
crear valores inválidos. El Core no genera UUIDs, computa SHA-256, controla clocks
reales ni autentica. `SyncEngine.observe` espera muestras recientes y reloj
confiable evaluados por Application; efectos fallidos exigen reset+error.

No añadir runtime async para eludir elegir el puente. Los contratos diseñados
de Transport/RoomService están documentados, sin traits genéricos vacíos.
Modelo completo en [ARCHITECTURE](../docs/ARCHITECTURE.md), política en
[SYNC](../docs/SYNC.md), alcance de tests en [TESTING](../docs/TESTING.md).
