# 13 — P2P Media Distribution Phase 1

Implementación directa LAN TCP/TLS 1.3 con señalización WSS de sala, consentimiento,
Host approval, bloques y SHA final, pausa/reanudación y Player existente.
Linux y Android físico; no P2P universal por Internet.

Estado inicial verificado: master limpio, origin/master
1fdd14e904f02491eba9e159d478c33179eb8e4c; CI 37863493740, 11/11 success.
Android SM-J701M/API28/armv7, Linux y Wi-Fi doméstico compartido.

- [Informe de 66 puntos](REPORT.md).
- [Resultados y límites de evidencia](RESULTS.md).
- [Comparación de transporte y fuentes primarias](TRANSPORT-EVALUATION.md).
- [Seguridad](SECURITY.md), [WAN roadmap](WAN-ROADMAP.md).
- [Contrato e instrucciones de producto](../../docs/P2P.md), [ADR-014](../../docs/DECISIONS.md).
- [Capturas y revisión visual](VISUAL-QA.md), [diagnóstico CI](CI-DIAGNOSIS.md).

## Reproducibilidad y privacidad

Primero `cargo build --workspace --locked`. Corpus propio generado mediante
`scripts/generate_test_media.py`; los MP4 se amplían con padding final para medir
bytes sin descargar material ajeno. Los bins/buffers y fixtures se excluyen de Git.
No usar documentos sensibles ni distribuir sin autorización.

Scripts físicos opt-in, requieren dispositivo Android desbloqueado, ADB autorizado,
NDK28.2, armv7 release bridge y APK debug con `--dart-define=P2P_QA=true`.
El modo QA solo configura una sala real y observa snapshots redacted; nunca
simula transferencia, clock, integridad, consentimiento, Ready ni Player.
Invitaciones/configs permanecen en pipes/archivos temporales privados. ADB configura
y captura; los bytes del archivo viajan por Wi-Fi TCP, sin adb reverse ni relay.

`measure_transport.py`: escalera 8 KiB/32/128 MiB; solo transporte.
`physical_android_linux.py`: transporte inverso 32 MiB; no SAF/producto.
`physical_linux_android.py`: Linux libmpv/CLI → Android Flutter/Media3, 256 MiB,
consentimiento, approval, pausa, Wi-Fi, SHA/Ready, playback y lifecycle.
`physical_android_host_product.py`: Host Android UI/SAF → Linux CLI/libmpv,
128 MiB, pausa/nuevo grant/Ready/playback; carga adicional con otro archivo.
`live_lifecycle.py`: procesos reales loopback WSS/TLS, cancelación offline,
Host disconnect/resume y cambio de medio.
`desktop_visual_qa.py`: app Flutter Linux real con XTest; usar PTY (`tty:true`).
El fixture Linux de selección QA evita el portal Wayland; no prueba ese chooser.

Estos scripts ocupan puertos locales dedicados y pueden actuar sobre Wi-Fi/app
Android. Ejecutarlos deliberadamente; no forman parte del widget test ni de CI.
El harness de pausa usa coordenadas 720×1280 inspeccionadas para este dispositivo;
configure CINE_P2P_PAUSE_POSITION con archivo privado JSON `[x,y]` si cambia layout.
La aplicación no depende de esas coordenadas ni de QA.

No sobrescribir experimentos previos ni publicar corpus multimedia completo en
artifacts. Las capturas usan únicamente nombres genéricos de fixtures propios.
