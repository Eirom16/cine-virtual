# Experimentos técnicos

El experimento 03 valida control con FakePlayer y el 01 valida multimedia aislada
en Linux (Spike B). El recorrido 06 integra ambos como vertical slice 1,
no como un nuevo spike aislado. El 02 ejecuta Spike C de bridge/UI/móvil; los demás siguen como planes.
Duración acotada, corpus sintético,
métricas reproducibles y decisión escrita; no promover código experimental al
producto por inercia. No crear carpetas vacías ni commits de binarios/vídeos.

| Orden | Experimento | Pregunta |
| --- | --- | --- |
| 1 | [01-player-crossplatform](01-player-crossplatform/README.md) | ¿Player controla vídeo real con precisión? Probado aislado Linux; integración en 06, móvil pendiente. |
| Apoyo al 1 | [02-rust-ui-bridge](02-rust-ui-bridge/README.md) | ¿Cómo cruzar UI/Rust sin romper threading/lifecycle? Spike C con DTO/C ABI, Flutter y candidato Media3. |
| Apoyo al 1 | [03-websocket-sync](03-websocket-sync/README.md) | ¿Control autoritativo v1, Ready y snapshot funcionan sobre red real? |
| Apoyo al 1 | [04-clock-sync](04-clock-sync/README.md) | ¿Qué precisión/lead requiere la red y el scheduler real? |
| Integración A+B | [06-real-vertical-slice](06-real-vertical-slice/README.md) | ¿Dos Player reales, identidad/Ready, red, drift y resume convergen durante 10 min? |
| Diferido | [05-webrtc](05-webrtc/README.md) | ¿Es viable comunicación audiovisual/P2P posterior? |

El objetivo final del experimento 01 incluye un recorrido vertical mínimo; se
validó primero Player aislado. 02–04 apoyan ese recorrido.
Formato del resultado futuro: fecha, versión exacta de SDK/toolchain, plataforma/
dispositivo, instrucciones, corpus/licencia, datos agregados, fallos y recomendación.
Dejar conclusión y enlaces en el README del spike cuando se ejecute. El Spike A
de 03 contiene integración WebSocket ejecutada y métricas localhost;
Spike B añadió después el experimento 01 con libmpv real aislado + Core en Linux.
El recorrido 06 combina ambos con dos vídeos reales Linux y evidencia guardada;
no se validó móvil. Esa documentación conserva límites y resultados sin promover v0.1.

## Vertical slice 2

[07-linux-android-room](07-linux-android-room/README.md) integra Application de
red Rust compartida con Flutter/Media3 Android y CLI/libmpv Linux. Gate físico
por Wi-Fi/LAN, identidad/Ready, controles, background y resume. Resultados
aprobados/fallidos se distinguen en su evidencia; no convierte los candidatos
provisionales en stack definitivo ni declara v0.1 completa.

## 14 — P2P WAN foundation

[Experimento 14](14-p2p-wan/README.md): investigación TCP/TLS vs ICE/WebRTC/QUIC,
spike relay TLS interior sobre sockets salientes, simulación NAT/netem y ARMv7.
Infraestructura WAN ausente: BLOCKED/NOT TESTED, sin deployment público.
