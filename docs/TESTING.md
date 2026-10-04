# Estrategia de pruebas

## Ejecutable hoy

`cargo test --workspace --offline` ejecuta escenarios deterministas del Core con
reloj inyectado, observaciones y Player falso. Sin UI, vídeo, servidor o red real.
No se duerme en tests: el tiempo avanza con números. `cargo fmt`, Clippy y build
completan las comprobaciones de Rust. `python3 scripts/check_docs.py` comprueba
links locales, JSON de ejemplos, capítulos y formato básico de documentación.

| Escenario | Cobertura de esta pasada |
| --- | --- |
| A=100000 ms, B=100083 ms | Signo +83, rate de frenado tras tres muestras |
| Cliente retrasado y jitter de posición | Rate aumenta; cambio de signo evita oscilación |
| Drift pequeño/grande y paused | Deadband, hard seek, restauración rate y cooldown |
| Clock con latencia/jitter | Ecuaciones, filtro, muestra inválida y asimetría |
| Pending futura y recepción tardía | Timeline conserva current y proyecta estado al vencer |
| Eventos duplicados/antiguos/fuera de orden | SequenceGate ignora o exige snapshot |
| Desconexión/reconexión | Gate requiere snapshot; estado completo incluye pending |
| Cambio de epoch | Rechazo sin reinicialización explícita |
| Identidad multimedia | Tamaño+digest; metadata no reemplaza hash |
| Capacidades Player | Fake player y tolerancia sin rate |

Transferencia Host, autorización, tokens, readiness global y deduplicación de
requests **no están implementados**, por tanto sus tests de integración quedan
definidos abajo. SequenceGate no implementa un servidor ni valida permisos.
Un test de timeline con Player falso no demuestra precisión de SDK o scheduler.

## Al construir RoomService y protocolo

Fixture sin red: reloj falso, store en memoria y sesiones simuladas; assert de
estado y efectos. Crear/unirse, límite, roles, claim inválido, Ready viejo,
selección nueva, Host desconectado reemplazando pending, gracia/expiración,
transferencia solo pausado y rechazo de old authority. Mutación fallida no consume
sequence. Same event_id/payload no duplica efectos; otro payload se rechaza.
Reintentos en nueva conexión no restauran un control antiguo.

Fixtures JSON golden compartidas con cliente futuro: envelope, State con pending,
ACK/ERROR, unknown version/type, claves duplicadas, límites, signed drift y nulls.
Decoder debe rechazar input inválido antes de tocar dominio. Un schema o codec
real se incorporará con el spike WebSocket; ejemplos documentados no son codec.

## Simulación de red y sistema

Harness virtual con cola y reloj: latencia 0–1000 ms, jitter y asimetría, duplicados,
reordenación, gaps, pérdidas por cierre y reconexión durante pending. WS ordena
frames en una conexión; pérdidas/reordenación se simulan en el nivel de sesión
y aplicación. No modelar datagramas como si WS fuera UDP.

Invariantes: controles solo del Host autenticado; media_revision vigente; cada
mutación única aumenta una vez sequence; snapshot reemplaza backlog; ningún
timer antiguo se ejecuta tras reset; offset/target finitos y posición acotada.
Después de converger, drift permanece en tolerancia del adapter bajo red medida.
Más adelante property tests/fuzz de codec y máquina de estados; no dependencias
añadidas hasta disponer de un comportamiento real que probar.

## Pruebas con multimedia y plataformas

Corpus sintético o redistribuible y con licencia indicada: MP4/H264/AAC, MKV,
framerate variable, vídeos largos, seek cerca del final, corrupción y archivo
cambiado. Sin vídeos privados en repositorio/CI. Medir exactitud/keyframes,
audio/rate, decodificación hardware, pause/resume, URI Android y permisos Apple.
El Core portable se compila/prueba en Windows/Linux/macOS en CI cuando se conecte
el repositorio; Android/iOS necesitan toolchains y dispositivos para adapters.

Primer gate de integración: dos clientes reales con el mismo archivo, Play/Pause/
Seek, ensayo 10 min, métricas p95, desconexión, copia discrepante y Host inválido.
Android/iOS deben ensayarse antes de elegir motor/puente, no después de diseñar
una UI final. Registrar evidencia en experiments sin promocionar el spike a producto.
