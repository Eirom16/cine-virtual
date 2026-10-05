# Decisión de licencia pendiente

El proyecto tiene intención open source, pero **todavía no tiene licencia
definitiva ni archivo LICENSE**. No interpretar este documento como permiso de
redistribución. Manifest Cargo tiene `publish=false` y no afirma SPDX inexistente.
ADR-009 explica por qué se deja pendiente; resolver antes de publicación/release
y de aceptar contribuciones públicas bajo términos supuestos.

| Alternativa | Política y ajuste al proyecto |
| --- | --- |
| MIT | Permisiva sencilla, exige conservar aviso; favorece reutilización privada y forks sin compartir cambios. No contiene cláusula expresa de patentes como Apache. |
| Apache-2.0 | Permisiva con concesión explícita de patentes y condiciones de avisos; adecuada si prima adopción comercial y reutilización del Core. |
| GPL-3.0 | Copyleft al distribuir trabajos cubiertos; operar cambios solo como servicio de red no activa por sí mismo una obligación equivalente a AGPL. |
| AGPL-3.0 | Copyleft con requisito de ofrecer fuente correspondiente a usuarios remotos cuando interactúan con una versión modificada cubierta. Interesante para el servidor self-hostable. |

La comparación se basa en los textos primarios de
[MIT](https://opensource.org/license/mit),
[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0.html),
[GPL-3.0](https://www.gnu.org/licenses/gpl-3.0.html) y
[AGPL-3.0, sección 13](https://www.gnu.org/licenses/agpl-3.0.html#section13).
Licencia permisiva y copyleft permiten self-hosting; la diferencia es cómo deben
compartirse ciertos derivados. AGPL no exige publicar todas las obras ajenas de
una infraestructura por el simple hecho de alojarlas juntas.

Propuesta para revisión humana: decidir si las modificaciones del servicio deben
volver a quienes lo usan. Si sí, evaluar AGPL-3.0 para el proyecto/servidor; si no,
Apache-2.0 es la candidata permisiva preferida por sus términos de patentes.
No adoptar aquí dual licensing ni licencias distintas por carpeta.

Después de elegir: confirmar titularidad, version exacta/`-only` vs `-or-later`,
incorporar texto oficial LICENSE, actualizar README/Cargo/headers y política
de contribución. Revisar dependencias efectivamente distribuidas, enlazado,
codecs, stores móviles y obligaciones de avisos/fuente. Los builds de
[FFmpeg](https://ffmpeg.org/legal.html) pueden cambiar obligaciones según opciones.
La evaluación técnica móvil y de licencias ocurre antes de seleccionar el SDK.

## Build multimedia de Spike B

[RESEARCH](../experiments/01-player-crossplatform/RESEARCH.md) distingue header
ISC/API libmpv, engine GPL por defecto, posibilidad LGPL condicionada, plugins y
dependencias. El paquete local mpv enlaza FFmpeg construido GPL/version3; no se
aprueba para distribución ni tiendas. libloading 0.9.0 usa ISC; no cambia la
licencia del engine ni la decisión pendiente de Cine Virtual. Linking, relinking,
fuente/avisos y términos móviles necesitan revisión del build concreto antes de
empaquetar. No se crea LICENSE ni se publica una release en este spike.

## Gate móvil/UI — Spike C

La estrategia híbrida investigada usa Media3 Apache-2.0 en Android y recomienda
AVPlayer nativo en iOS, sin copiar el build mpv/FFmpeg GPL Linux a APK/IPA.
Flutter BSD-3-Clause y el bridge Rust manual no resuelven licencia del proyecto.
FRB (MIT) y cbindgen (MPL-2.0) fueron evaluados, no añadidos como dependencias.

**TECHNICAL RISK — LEGAL REVIEW REQUIRED:** distribución GPL/LGPL en App Store,
linking estático, objetos relinkables, signing/DRM, avisos/fuentes y cierre de
plugins/codecs necesitan revisión del build concreto y términos de tienda. No se
afirma compatibilidad/incompatibilidad jurídica universal. Investigación primaria
y alternativas en [RESEARCH](../experiments/02-rust-ui-bridge/RESEARCH.md).
No se crea LICENSE ni se publica APK/IPA/release; los builds son locales.

## CI experimental

[CI](CI.md) prepara artifacts de Actions, sin releases ni stores. No se bundlean
libmpv/FFmpeg. Compilar un artifact no aprueba su redistribución; la licencia
del proyecto y los avisos de componentes siguen pendientes. Esta pasada no
publicó artifacts remotamente ni añadió una licencia.
