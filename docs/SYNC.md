# Tiempo y sincronización

Contrato complementario a [PROTOCOL](PROTOCOL.md). Todo valor inicial de tuning
es configurable y debe verificarse en [los spikes](../experiments/README.md).

## Dominios de tiempo

El servidor usa milisegundos monotónicos desde su arranque, nunca UTC para
programar reproducción. Cada cliente usa milisegundos monotónicos de su propio
proceso. Offset convierte cliente a servidor: `server_now = client_now + offset`.
Los orígenes pueden ser diferentes; las cuatro muestras permiten estimar esa
diferencia. `clock_epoch` UUID del proceso cambia tras reinicio. Cada room_epoch
pertenece a exactamente un clock_epoch. Offset y timers no sobreviven a su cambio.
UTC se permite solo para logs, sin participar en orden ni deadlines.

En móvil, la semántica del reloj durante suspensión varía. En background,
suspensión/reanudación, salto detectado o reconexión: cancelar timers, pausar,
restaurar rate nominal, pedir snapshot y recalibrar. No garantizar reproducción
sincronizada en segundo plano en v0.1.

## Clock sync

TIME_PING incluye T1 y sample_id. El adapter servidor captura T2 al recibir y T3
inmediatamente antes de enviar TIME_PONG. Cliente captura T4 al recibir. No se
reutiliza una respuesta; se vincula a una muestra pendiente y clock_epoch.

```text
RTT = (T4 - T1) - (T3 - T2)
offset = ((T2 - T1) + (T3 - T4)) / 2
deadline_cliente = execute_at_servidor - offset
```

Estas ecuaciones siguen el modelo de cuatro timestamps de
[RFC 5905](https://www.rfc-editor.org/rfc/rfc5905.html). No implementamos NTP.
El Core resta con enteros i128 antes de convertir a f64, evitando overflow en
timestamps i64. Los adapters verifican que los timestamps sean no negativos y
estén dentro del rango entero seguro del protocolo.

Inicio: enviar ocho muestras separadas 100 ms; exigir al menos tres válidas.
Filtro implementado: conservar últimas ocho, tomar las tres de RTT más bajo,
usar mediana de sus offsets y mínimo RTT como diagnóstico. Rechazar T4<T1,
T3<T2, RTT negativo o >2000 ms. RTT asimétrico introduce un error que estas
ecuaciones no eliminan; no prometer exactitud inferior a la latencia asimétrica.

Application futura estima incertidumbre conservadora: RTT mínimo/2 + dispersión
máxima de offsets seleccionados. Solo marca clock_trusted si ≤100 ms y última
muestra válida hace ≤15 s. Actualizar cada 5 s; cambio de offset >100 ms obliga
a snapshot/recalibración; ajustes pequeños se suavizan antes de reprogramar.
Filtro puro implementado; incertidumbre, antigüedad y suavizado aún diseñados.

## Playback programado

Host solicita control con posición y revisiones esperadas, nunca con execute_at
autoritativo. Servidor valida Host, media, readiness y revisiones; asigna secuencia
y `execute_at_ms = server_now + lead_ms`.

Lead inicial 500 ms; adaptativo futuro dentro de 300–2000 ms considerando RTT p95
y jitter de los conectados. Dentro de v0.1 se mide ese margen y se puede configurar;
no debe usarse para ocultar latencias ilimitadas. El Host sigue el mismo evento
que todos: no reproduce optimistamente al enviar la solicitud.

Timeline actual sigue vigente hasta execute_at. Entonces la nueva timeline
define estado, posición y rate nominal. Para playing:

```text
target(t) = clamp(position_anchor + rate × max(0, t-anchor_time), 0, duration)
drift = actual_position - target
```

Para paused/stopped, target es constante. Pause congela la posición proyectada
al execute_at, no la posición de cuando se envió el pedido. Seek fija posición
en execute_at y conserva el modo. Play comienza en la posición solicitada.
Rate global es 1.0 en v1; las correcciones locales no cambian el timeline global.
Llegar a duración implica detener localmente; el estado efectivo se muestra como
ended aunque el último timeline fuera playing. Una orden posterior permite seek.

Application precarga pausado; para Play programa seek/preparación cerca del
deadline y play al vencimiento, según capacidades medidas del adapter. No se
supone que seek sea instantáneo. Si no llega a tiempo, informa no preparado y
converge al estado actual; no ejecuta un play/pause viejo en cadena. Tras recibir
tarde un evento, proyecta su timeline a `server_now` y ejecuta ese estado actual.
Timer se liga a room_epoch, media_revision, authority_revision y secuencia;
snapshot/cambio de media/host/desconexión lo cancela. Un gap obliga a snapshot.

## Drift correction

Muestrear cada 500 ms con posición y tiempo capturados conjuntamente. Un adapter
que mide en otro hilo debe aportar timestamp y antigüedad de la muestra. No usar
posición vieja como actual. La política recibe observaciones válidas y produce
efectos; no conoce sockets ni reproductor concreto.

| Configuración inicial | Valor | Interpretación |
| --- | --- | --- |
| deadband_ms | 50 | ≤50 ms: sin corrección; restaurar rate nominal si estaba ajustado |
| hard_seek_ms | 250 | >250 ms sostenidos: seek al target |
| consecutive_samples | 3 | Exigir tres muestras fuera de tolerancia con el mismo signo |
| cooldown_ms | 2000 | Separación mínima entre correcciones nuevas |
| max_rate_delta | 0.02 | Corrección suave de hasta ±2% relativo al rate nominal |
| convergence_ms | 3000 | Horizonte de rate: nominal × (1-clamp(drift/horizonte)) |

Cliente adelantado tiene drift positivo y reduce rate; retrasado lo aumenta.
Entre 50 y 250 ms se aplica soft correction solo si Player soporta rate.
Sin esa capacidad se tolera el error intermedio y se usa seek al superar hard.
Pausado: no hay rate correction; tras tres muestras fuera de deadband, seek.
Buffering o reloj no fiable: suspender correcciones y restaurar rate. El estado
de preparación se informa por separado; el cliente converge tras recuperarse.

Antioscilación: histéresis mediante deadband, tres muestras con signo consistente,
cooldown, no repetir un rate ya aplicado y restauración inmediata al converger
o al cambiar el signo del drift durante una corrección suave.
Antes de un hard seek se restaura rate si había corrección suave; el siguiente
sample produce seek, un efecto por observación. Una transición/media/resume
resetea el motor; Application restaura explícitamente rate al reset. Un error
de dispatch resetea estado y reporta PLAYER_ERROR, nunca supone que se ejecutó.

Seek tardío debe reestimar target antes de aplicarse. Tolerancia final queda
limitada por precisión de seek/keyframes, timer, frames y muestras del SDK.
Los defaults no garantizan que un adapter logre 50 ms. La política implementada
se ensaya en [core/tests](../core/tests/sync_scenarios.rs).

## Recuperación y entrada tardía

Sin conexión se pausa localmente y se cancela todo control pendiente; no puede
seguir actuando como Host. Transport reintenta con backoff+jitter (0.5–8 s,
presupuesto máximo de gracia 30 s), pero Application no reenvía PLAY/SEEK pendientes.
Resume devuelve snapshot completo, permisos actuales, secuencia y transición
pendiente. Recalibrar reloj, verificar medio vigente, preparar, marcar Ready,
restaurar modo/posición proyectados. Un snapshot puede estar pausado ahora con
Play futuro: conservar ambas timelines.

El token de resume conserva member_id en la gracia; si expira, join nuevo no
hereda permisos. Un nuevo epoch exige limpiar réplica completa y tokens; no
aceptar un evento arbitrario para sustituir la sala. La incorporación tardía usa
el mismo proceso que resume y no rebobina al grupo. No se reproduce un backlog
de controles pasados: el snapshot autoritativo resuelve el estado.
