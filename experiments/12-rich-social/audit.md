# Auditoría previa a implementación (2026-10-08)

Estado inicial TESTED: master a771497, limpio y sincronizado; CI 37802575391 SUCCESS.

- ChatMessage: SocialEntry del dominio con UUID de mensaje/miembro, nombre de membership,
  social_sequence, sent_at_ms monotónico, kind y texto. DTO replica esos campos.
- Social sequence: contador independiente de RoomState.sequence. No afecta Ready/timeline.
- History: VecDeque, 100 entries y presupuesto conservador JSON 48 KiB; sin blobs.
- Floating reactions: seis emojis; cola cliente 32/3 s y overlay Flutter 12/2.2 s,
  independientes del historial. No se modificarán sus semánticas.
- Snapshot: SOCIAL_STATE en create/join/resume/SYNC, reemplazo atómico, watermark;
  CHAT_MESSAGE consecutivo, gap solicita SYNC; replay no anima reacciones efímeras.
- Dedup: RoomService usa la cache existente por room/member/event + fingerprint,
  antes de mutar; no hace falta otra cache social.
- Bridge: Client.social_summary → mismo runtime Rust → JSON Application → SocialView.
  Intents chat/reaction en owners desktop/móvil; sin política social Kotlin.
- UI: notifier social separado de room/playback; publicación aplazada durante frame.
  Desktop panel lateral, mobile sheet con teclado/safe area, selección de texto,
  unread local, scroll bottom condicionado. Player/surface con identidad estable.
- Límites: chat bucket 5/2 s, floating bucket 8/500 ms, quotas por membership.

Extensiones compatibles: campos opcionales de presentación en DTO (UTC, reply,
reactions, contenido discriminado). Clientes social_v1 antiguos validan kind chat
con texto; NO pueden recibir kind gif ni eventos desconocidos. Se negocia
rich_social_v1 adicional a social_v1. Legacy recibe texto fallback y snapshots
compatibles; protocolo permanece v1. Dominio usará contenido discriminado.

Diseño: MESSAGE_SEND con contenido text/gif + reply opcional; mismo CHAT_MESSAGE
con addon content para rich y fallback text para legacy. MESSAGE_REACTION_SEND
toggle; snapshot autoritativo con nuevo watermark para recuperar/ordenar estado
sin crear otro historial. Reply solo a IDs aún retenidos de la misma sala;
referencias ya aceptadas sobreviven eviction con placeholder. Timestamp UTC
capturado al crear entry, solo presentación. Reactions por emoji/miembro ≤16,
6 emojis, cuota independiente; eviction elimina también sus reacciones.

Provider: bloqueo de integración externa pendiente de términos/cache y credencial.
No HTTP dentro de RoomService. No servidor fetch/proxy ni URLs arbitrarias.
