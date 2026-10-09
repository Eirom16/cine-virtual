# Diagnóstico de CI — sockets aceptados

Run37965374514, HEADf622bc0: gate principal, Linux, Android e iOS pasaron; tests
Rust dentro del step Compile Rust fallaron en Windows/macOS. No error del
compilador ni fallo ocultado con continue-on-error. El test real
`authenticated_wss_room_grant_and_separate_tls_file_connection` recibió
TRANSFER_Io con cero bytes; no alcanzó SHA ni Ready.

Causa reproducida: listener no bloqueante para poder cancelar/polling; los
sockets aceptados pueden heredar ese modo en Windows/BSD. El worker TLS usa
I/O bloqueante con timeouts. Linux accept no hereda O_NONBLOCK, por eso no fallaba
localmente. La diferencia se documenta en [Microsoft accept](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-accept)
y [Linux accept](https://man7.org/linux/man-pages/man2/accept.2.html).

Reproducción determinista en Linux: configurar el socket aceptado con
set_nonblocking(true) antes del test TLS pause/resume existente. Sin corrección,
el test falló en0.12s, Err(Io) antes de poder pausar. Restaurando la corrección,
la misma transferencia autenticada logra pausa a un chunk y resume con grant
nuevo/SHA correcto. No se amplía el timeout ni se ignora el test.

Corrección: `transport::socket` configura explícitamente
set_nonblocking(false), read/write timeouts3s y nodelay. Handshake absoluto5s,
revocación y shutdown conservados. El listener continúa no bloqueante; solo el
socket del worker cambia, fuera del runtime/UI/Player. El test fuerza el modo
heredado en todas las plataformas. Tests específicos y workspace/clippy/build
locales repetidos; nuevo push y run completo obligatorios antes de cerrar.

La corrección es portable, no agrega soporte de Player Windows/macOS/iOS.
Los tests TLS/WSS reales de CI tampoco equivalen a probar sus UIs o multimedia.
