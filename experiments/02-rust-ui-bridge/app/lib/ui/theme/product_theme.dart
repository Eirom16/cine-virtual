import 'package:flutter/material.dart';

abstract final class CineTokens {
  static const background = Color(0xFF0D0D15);
  static const surface = Color(0xFF171722);
  static const elevated = Color(0xFF20202E);
  static const border = Color(0xFF333344);
  static const text = Color(0xFFF3F1FC);
  static const muted = Color(0xFFB5B2C7);
  static const accent = Color(0xFFB2A0FF);
  static const healthy = Color(0xFF84DAB4);
  static const warning = Color(0xFFF2C47C);
  static const error = Color(0xFFFFA1AA);
  static const double xs = 8, sm = 12, md = 16, lg = 24, xl = 32, xxl = 48;
  static const double radius = 16, controlRadius = 12, elevation = 0;
  static const double desktop = 900, wide = 1200, maxContent = 1200;
  static const motion = Duration(milliseconds: 180);
  static const overlayIdle = Duration(seconds: 4);
  static const pageInsets = EdgeInsets.all(lg);
}

ThemeData productTheme() {
  const colors = ColorScheme.dark(
    primary: CineTokens.accent,
    onPrimary: Color(0xFF24194D),
    secondary: CineTokens.accent,
    surface: CineTokens.surface,
    onSurface: CineTokens.text,
    error: CineTokens.error,
    outline: CineTokens.border,
  );
  final base = ThemeData(
    useMaterial3: true,
    colorScheme: colors,
    scaffoldBackgroundColor: CineTokens.background,
    visualDensity: VisualDensity.standard,
  );
  return base.copyWith(
    textTheme: base.textTheme
        .copyWith(
          headlineLarge: const TextStyle(
            fontSize: 42,
            height: 1.15,
            fontWeight: FontWeight.w700,
            letterSpacing: -1.1,
          ),
          headlineMedium: const TextStyle(
            fontSize: 28,
            height: 1.2,
            fontWeight: FontWeight.w600,
            letterSpacing: -.5,
          ),
          titleLarge: const TextStyle(
            fontSize: 21,
            height: 1.3,
            fontWeight: FontWeight.w600,
          ),
          bodyLarge: const TextStyle(fontSize: 16, height: 1.5),
          bodyMedium: const TextStyle(fontSize: 14, height: 1.5),
          labelLarge: const TextStyle(
            fontSize: 14,
            fontWeight: FontWeight.w600,
          ),
        )
        .apply(bodyColor: CineTokens.text, displayColor: CineTokens.text),
    appBarTheme: const AppBarTheme(
      backgroundColor: CineTokens.background,
      elevation: CineTokens.elevation,
      scrolledUnderElevation: CineTokens.elevation,
      centerTitle: false,
    ),
    dividerTheme: const DividerThemeData(
      color: CineTokens.border,
      thickness: 1,
    ),
    cardTheme: CardThemeData(
      color: CineTokens.surface,
      elevation: CineTokens.elevation,
      margin: EdgeInsets.zero,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(CineTokens.radius),
        side: const BorderSide(color: CineTokens.border),
      ),
    ),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: CineTokens.background,
      contentPadding: const EdgeInsets.all(CineTokens.md),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(CineTokens.controlRadius),
        borderSide: const BorderSide(color: CineTokens.border),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(CineTokens.controlRadius),
        borderSide: const BorderSide(color: CineTokens.border),
      ),
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        minimumSize: const Size(48, 48),
        padding: const EdgeInsets.symmetric(
          horizontal: CineTokens.lg,
          vertical: CineTokens.md,
        ),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(CineTokens.controlRadius),
        ),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        minimumSize: const Size(48, 48),
        side: const BorderSide(color: CineTokens.border),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(CineTokens.controlRadius),
        ),
      ),
    ),
    iconButtonTheme: IconButtonThemeData(
      style: IconButton.styleFrom(minimumSize: const Size(48, 48)),
    ),
    snackBarTheme: const SnackBarThemeData(
      behavior: SnackBarBehavior.floating,
      backgroundColor: CineTokens.elevated,
      contentTextStyle: TextStyle(color: CineTokens.text),
    ),
    dialogTheme: DialogThemeData(
      backgroundColor: CineTokens.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(CineTokens.radius),
      ),
    ),
  );
}
