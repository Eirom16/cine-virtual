# 05 — WebRTC futuro

Estado: diferido fuera de v0.1. No se incorporan dependencias ahora.

Demostrar posteriormente una llamada de voz mínima y canal de datos con
señalización autorizada en sala. Evaluar NAT/ICE/STUN/TURN, permisos móviles,
coste de relay, cancelación, cifrado, privacidad y coexistencia con Player.
Cámara/screen sharing solo después de voz básica y capacidades por plataforma.

P2P de archivos requiere protocolo de chunks/integridad/consentimiento propio;
un data channel no lo implementa por sí solo. El control autoritativo permanece
separado. Resultado: evidencia de viabilidad, costes y ADR nuevo, sin federación
ni sistema completo de videollamadas. Ver [ROADMAP](../../docs/ROADMAP.md).
