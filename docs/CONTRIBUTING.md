# Contribuir

Antes de implementar, leer [PRODUCT](PRODUCT.md), [ARCHITECTURE](ARCHITECTURE.md),
[PROTOCOL](PROTOCOL.md) y [DECISIONS](DECISIONS.md). El repositorio está en
fundaciones; no añadir providers, chat o infraestructura fuera del hito vigente.
La [licencia pendiente](LICENSING.md) debe resolverse antes de aceptar aportes
públicos bajo términos no definidos. No se introduce CLA/DCO por cuenta del autor.

## Forma de trabajo

1. Describir problema, alcance y criterio verificable. Una propuesta tecnológica
   de riesgo empieza como experimento y registra evidencia por plataforma.
2. Mantener cambios pequeños, dominio puro y módulos con responsabilidad clara.
   No introducir dependencias exclusivas de plataforma fuera de adapters.
3. Probar invariantes y casos de fallo; no tests que repitan cada línea de código.
   Sin sleeps ni Player real en tests del Core. Reloj siempre inyectado.
4. Actualizar protocolo/ADRs ante cambios semánticos y marcar estado real en README.
   Documentación en español, identificadores/código en inglés, UTF-8/LF.
5. Ejecutar validación y revisar diff. PR explica problema, comportamiento,
   pruebas, riesgos y plataforma medida, sin presentar planes como funcionalidad.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
cargo build --workspace --offline
python3 scripts/check_docs.py
git diff --check
git diff
```

Rust mínimo 1.85 por edición 2024; std únicamente en esta base. Versiones futuras
de herramientas se adoptan deliberadamente; Cargo.lock se conserva para workspace.
Python ≥3.10 valida docs. CI futura reproducirá checks del Core en los tres OS
desktop; builds Android/iOS requieren runners/toolchains propios y pruebas reales.
No hay CI remota conectada en este entorno ni matriz móvil ya verificada.

No incluir vídeos privados, credenciales, paths personales en fixtures o logs.
Ante fallo de seguridad evitar publicarlo con secretos/datos; definir canal
privado de reporte al habilitar repositorio público, sin inventar un email ahora.
