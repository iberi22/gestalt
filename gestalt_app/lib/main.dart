import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'screens/main_layout.dart';
import 'screens/login_screen.dart';
import 'services/api_service.dart';
import 'services/settings_service.dart';
import 'theme/neo_glass_theme.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final prefs = await SharedPreferences.getInstance();
  final settingsService = SettingsService(prefs);

  runApp(
    GestaltProviders(
      settingsService: settingsService,
      child: const GestaltApp(),
    ),
  );
}

class GestaltProviders extends StatelessWidget {
  final SettingsService settingsService;
  final Widget child;

  const GestaltProviders({
    super.key,
    required this.settingsService,
    required this.child,
  });

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: settingsService),
        ProxyProvider<SettingsService, ApiService>(
          update: (_, settings, __) => ApiService(
            baseUrl: settings.baseUrl,
            token: settings.token,
          ),
        ),
      ],
      child: child,
    );
  }
}

class GestaltApp extends StatelessWidget {
  const GestaltApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Neural-Link v2.0',
      debugShowCheckedModeBanner: false,
      theme: NeoGlassTheme.lightTheme,
      darkTheme: NeoGlassTheme.darkTheme,
      themeMode: ThemeMode.dark,
      home: Consumer<SettingsService>(
        builder: (context, settings, _) {
          return settings.hasCredentials
              ? const MainLayout()
              : const LoginScreen();
        },
      ),
    );
  }
}
