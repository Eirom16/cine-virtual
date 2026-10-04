# Adapter LocalMedia

Frontera privada del dispositivo: handle de archivo, probe auxiliar acotado,
SHA-256 completo streaming y descriptor portable. Depende de cine-core, sha2 y
serde_json; Core no depende de este crate ni del filesystem.

LocalHandle retiene un FD y pathname canónico privados. Hash usa buffer de 1 MiB,
progreso por callback y cancelación retornando false; no carga el archivo completo.
Valida archivos regulares, tamaño positivo ≤1 TiB y estabilidad antes/después.
En Unix compara además dev/inode/ctime. Ninguno de esos atributos sustituye el
hash real. No hay cache persistente ni garantía de archivo inmutable.

ffprobe solo pide duration/format/codecs, sin shell ni tags/filename, con stdout
≤64 KiB y timeout de 10 s, kill/reap y stderr descartado. Si falla/falta, libmpv
puede aportar duración usable a descriptor; MIME/codecs son opcionales. Si probe
está presente, diferencia de duración ≤1000 ms. Descriptor nunca incluye pathname.

Tests no requieren ffprobe ni SDK. Para ver el caso de uso real:
[MEDIA](../../docs/MEDIA.md) y [slice](../../experiments/06-real-vertical-slice/README.md).
URI móviles/proveedores/hash por chunks quedan pendientes.
