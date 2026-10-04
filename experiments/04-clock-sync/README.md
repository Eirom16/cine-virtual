# 04 — Reloj y scheduler

Estado: plan, con estimador/filtro puro ya probado en Core, sin medición de red.
Implementar T1–T4 en adapters usando reloj monotónico, warmup, confianza,
renovación y clock_epoch. Medir scheduler/seek además del RTT.

Simular latencia, jitter, asimetría, timestamp viejo, suspensión móvil y reinicio.
Comparar filtro de mínimos/mediana con datasets sintéticos y reales. Variar lead
300–2000 ms, muestras 500 ms y thresholds; medir deadlines perdidos y drift.
Asimetría no desaparece con NTP conceptual: registrar sesgo/incertidumbre.

Resultado: tuning por capacidades con datos, criterios de clock_trusted y
cancelación/reprogramación de timers; no cambiar reloj del sistema. Ver
[SYNC](../../docs/SYNC.md) y [ADR-003](../../docs/DECISIONS.md).
