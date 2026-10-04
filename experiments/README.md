# Experimentos técnicos

El experimento 03 valida control con FakePlayer y el 01 valida multimedia aislada
en Linux (Spike B). Los demás siguen como planes.
Duración acotada, corpus sintético,
métricas reproducibles y decisión escrita; no promover código experimental al
producto por inercia. No crear carpetas vacías ni commits de binarios/vídeos.

| Orden | Experimento | Pregunta |
| --- | --- | --- |
| 1 | [01-player-crossplatform](01-player-crossplatform/README.md) | ¿Player controla vídeo real con precisión? Probado aislado Linux; dos clientes/móvil pendientes. |
| Apoyo al 1 | [02-rust-ui-bridge](02-rust-ui-bridge/README.md) | ¿Cómo cruzar UI/Rust sin romper threading/lifecycle? |
| Apoyo al 1 | [03-websocket-sync](03-websocket-sync/README.md) | ¿Control autoritativo v1, Ready y snapshot funcionan sobre red real? |
| Apoyo al 1 | [04-clock-sync](04-clock-sync/README.md) | ¿Qué precisión/lead requiere la red y el scheduler real? |
| Diferido | [05-webrtc](05-webrtc/README.md) | ¿Es viable comunicación audiovisual/P2P posterior? |

El objetivo final del experimento 01 incluye un recorrido vertical mínimo; se
validó primero Player aislado. 02–04 apoyan ese recorrido.
Formato del resultado futuro: fecha, versión exacta de SDK/toolchain, plataforma/
dispositivo, instrucciones, corpus/licencia, datos agregados, fallos y recomendación.
Dejar conclusión y enlaces en el README del spike cuando se ejecute. El Spike A
de 03 contiene integración WebSocket ejecutada y métricas localhost;
Spike B añadió después el experimento 01 con libmpv real aislado + Core en Linux.
El siguiente gate es combinar ambos como vertical slice; aún no se ejecutó con
dos vídeos reales ni se validó móvil.
