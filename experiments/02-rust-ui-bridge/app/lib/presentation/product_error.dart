enum ErrorCategory {
  connection,
  room,
  media,
  player,
  permission,
  protocol,
  unknown,
}

class ProductError {
  final ErrorCategory category;
  final String message, help;
  const ProductError(this.category, this.message, this.help);
  factory ProductError.from(String code) {
    switch (code) {
      case 'TRANSFER_SPACE':
        return const ProductError(
          ErrorCategory.media,
          'No hay espacio suficiente.',
          'Libera el tamaño del archivo y 64 MiB de margen antes de recibirlo.',
        );
      case 'TRANSFER_STORAGE':
        return const ProductError(
          ErrorCategory.media,
          'El destino no está disponible.',
          'Elige otra carpeta o revisa el almacenamiento del dispositivo.',
        );
      case 'TRANSFER_MANIFEST':
        return const ProductError(
          ErrorCategory.media,
          'El archivo no es compatible con esta transferencia.',
          'Esta fase admite archivos completos de hasta 16 GiB. Verifica la oferta del anfitrión.',
        );
      case 'TRANSFER_MODIFIED':
        return const ProductError(
          ErrorCategory.media,
          'El archivo del anfitrión cambió.',
          'El anfitrión debe verificarlo y volver a ofrecerlo.',
        );
      case 'TRANSFER_INTEGRITY':
        return const ProductError(
          ErrorCategory.media,
          'El archivo no superó la verificación.',
          'No se cargará ese archivo. Solicita una nueva transferencia.',
        );
      case 'TRANSFER_EXPIRED':
        return const ProductError(
          ErrorCategory.permission,
          'La autorización de transferencia expiró.',
          'Solicita otra autorización para reanudar los bloques verificados.',
        );
      case 'TRANSFER_TLS':
      case 'TLS_PIN_REQUIRED':
        return const ProductError(
          ErrorCategory.connection,
          'No se pudo verificar la conexión segura.',
          'Usa el endpoint completo de una sala WSS obtenido por un canal de confianza.',
        );
      case 'TRANSFER_CONNECTION':
        return const ProductError(
          ErrorCategory.connection,
          'Se interrumpió la conexión de transferencia.',
          'Revisa la LAN y solicita reanudar. Se conservan los bloques verificados.',
        );
      case 'TRANSFER_LOAD_FAILED':
        return const ProductError(
          ErrorCategory.player,
          'No se pudo abrir la película recibida.',
          'El archivo se verificó, pero el reproductor no pudo cargarlo. Puedes volver a intentar o seleccionar otra copia.',
        );
      case 'MEDIA_MISMATCH':
        return const ProductError(
          ErrorCategory.media,
          'El archivo no coincide con el de la sala.',
          'Selecciona la misma versión que está usando el anfitrión.',
        );
      case 'INVITE_INVALID':
      case 'INVALID_INVITE':
        return const ProductError(
          ErrorCategory.room,
          'La invitación no es válida.',
          'Pide al anfitrión una nueva invitación completa.',
        );
      case 'ROOM_NOT_FOUND':
      case 'ROOM_CLOSED':
      case 'RESUME_EXPIRED':
        return const ProductError(
          ErrorCategory.room,
          'Esta sala ya no está disponible.',
          'Sal de la sala y crea una nueva o usa otra invitación.',
        );
      case 'ROOM_FULL':
        return const ProductError(
          ErrorCategory.room,
          'La sala está llena.',
          'Prueba con otra sala.',
        );
      case 'VERSION_UNSUPPORTED':
      case 'PROTOCOL_VERSION_UNSUPPORTED':
        return const ProductError(
          ErrorCategory.protocol,
          'La versión del servicio no es compatible.',
          'Usa una versión compatible de Cine Virtual y del servidor.',
        );
      case 'INVALID_ENDPOINT':
        return const ProductError(
          ErrorCategory.connection,
          'La dirección del servidor no es válida.',
          'Revisa la conexión en Configuración avanzada.',
        );
      case 'CONTROL_PENDING':
      case 'OUT_OF_SEQUENCE':
      case 'STALE_MEDIA':
      case 'STALE_AUTHORITY':
        return const ProductError(
          ErrorCategory.room,
          'La sala está actualizando su estado.',
          'Espera un momento y vuelve a intentarlo.',
        );
      case 'CLOCK_UNTRUSTED':
      case 'MEDIA_NOT_READY':
        return const ProductError(
          ErrorCategory.media,
          'Todavía no estás preparado.',
          'Espera a que el archivo, el reproductor y la conexión estén listos.',
        );
      case 'NOT_AUTHORIZED':
        return const ProductError(
          ErrorCategory.room,
          'El anfitrión controla esta acción.',
          'Espera a que el anfitrión continúe.',
        );
      case 'VIDEO_RENDER_FAILED':
      case 'VIDEO_CONTEXT_PENDING':
      case 'VIDEO_BRIDGE_FAILED':
      case 'VIDEO_LEASE_FAILED':
        return const ProductError(
          ErrorCategory.player,
          'No se pudo mostrar el vídeo.',
          'Vuelve a entrar a la sala. Si continúa, reinicia la aplicación.',
        );
      case 'PLAYER_UNSUPPORTED':
        return const ProductError(
          ErrorCategory.player,
          'El reproductor todavía no está disponible aquí.',
          'iOS no tiene runtime de vídeo. Windows y macOS no tienen reproducción validada.',
        );
      case 'PERMISSION_DENIED':
      case 'INVALID_URI':
        return const ProductError(
          ErrorCategory.permission,
          'No se pudo acceder al archivo.',
          'Vuelve a seleccionarlo y concede acceso de lectura.',
        );
      case 'HASH_FAILED':
      case 'MEDIA_NOT_READY_TIMEOUT':
        return const ProductError(
          ErrorCategory.media,
          'No se pudo preparar la película.',
          'Selecciona un archivo local válido que no esté siendo modificado.',
        );
      case 'BRIDGE_UNAVAILABLE':
        return const ProductError(
          ErrorCategory.player,
          'No se pudo iniciar el motor de Cine Virtual.',
          'Reabre la aplicación con el bridge de esta build instalado.',
        );
      case 'NETWORK_DISCONNECTED':
      case 'NETWORK_UNREACHABLE':
      case 'NETWORK_TIMEOUT':
      case 'OPERATION_CANCELLED_OR_TIMEOUT':
        return const ProductError(
          ErrorCategory.connection,
          'No se pudo conectar con la sala.',
          'Comprueba el servicio y la conexión. Puedes reintentar.',
        );
      default:
        return const ProductError(
          ErrorCategory.unknown,
          'No se pudo completar la operación.',
          'Vuelve a intentarlo. Los detalles están disponibles en Developer.',
        );
    }
  }
}
