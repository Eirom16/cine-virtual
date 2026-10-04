# Experimentos técnicos

Son planes de spikes, no capacidades integradas. Duración acotada, corpus sintético,
métricas reproducibles y decisión escrita; no promover código experimental al
producto por inercia. No crear carpetas vacías ni commits de binarios/vídeos.

| Orden | Experimento | Pregunta |
| --- | --- | --- |
| 1 | [01-player-crossplatform](01-player-crossplatform/README.md) | ¿Dos clientes con mismo vídeo local pueden sincronizar Play/Pause/Seek y medir drift con SDK portable? |
| Apoyo al 1 | [02-rust-ui-bridge](02-rust-ui-bridge/README.md) | ¿Cómo cruzar UI/Rust sin romper threading/lifecycle? |
| Apoyo al 1 | [03-websocket-sync](03-websocket-sync/README.md) | ¿Control autoritativo v1, Ready y snapshot funcionan sobre red real? |
| Apoyo al 1 | [04-clock-sync](04-clock-sync/README.md) | ¿Qué precisión/lead requiere la red y el scheduler real? |
| Diferido | [05-webrtc](05-webrtc/README.md) | ¿Es viable comunicación audiovisual/P2P posterior? |

El experimento 01 es un recorrido vertical mínimo; 02–04 son verificaciones
que lo apoyan, no prerrequisitos para construir una aplicación completa.
Formato del resultado futuro: fecha, versión exacta de SDK/toolchain, plataforma/
dispositivo, instrucciones, corpus/licencia, datos agregados, fallos y recomendación.
Dejar conclusión y enlaces en el README del spike cuando se ejecute. Hoy ninguno
contiene una integración multimedia o de red ejecutada.
