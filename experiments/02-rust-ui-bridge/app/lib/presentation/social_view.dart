import 'view_state.dart';

const socialEmojis = ['❤️', '😂', '😮', '😢', '🔥', '👏'];
const socialLabels = [
  'amor',
  'risa',
  'sorpresa',
  'tristeza',
  'fuego',
  'aplausos',
];

/// Bounded Rust projection; unread/visibility are local presentation only.
class SocialView {
  final Map<String, dynamic> data;
  final String memberId, error;
  final int unread;
  final bool pending, connected, blocked;
  const SocialView({
    this.data = const {},
    this.memberId = '',
    this.error = '',
    this.unread = 0,
    this.pending = false,
    this.connected = false,
    this.blocked = false,
  });
  List<Map<String, dynamic>> get entries =>
      (data['entries'] as List? ?? []).map(object).toList();
  List<Map<String, dynamic>> get reactions =>
      (data['reactions'] as List? ?? []).map(object).toList();
  bool get supported => data['supported'] == true;
  bool get canSend => connected && supported && !pending && !blocked;
  String get errorText => switch (error) {
    'RATE_LIMITED' => 'Vas muy rápido. Espera un momento y vuelve a intentar.',
    'PAYLOAD_TOO_LARGE' => 'El mensaje supera el límite de 2 KiB.',
    'INVALID_EVENT' => 'Escribe un mensaje de texto, con hasta 9 líneas.',
    'FEATURE_NOT_SUPPORTED' => 'Este servidor aún no tiene chat.',
    'NETWORK_DISCONNECTED' => 'Reconecta a la sala para enviar.',
    '' => '',
    _ => 'No se pudo enviar. Inténtalo de nuevo.',
  };
}
