// This is a basic Flutter widget test.
//
// To perform an interaction with a widget in your test, use the WidgetTester
// utility in the flutter_test package. For example, you can send tap and scroll
// gestures. You can also use WidgetTester to find child widgets in the widget
// tree, read text, and verify that the values of widget properties are correct.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:gestalt_app/services/settings_service.dart';

import 'package:gestalt_app/main.dart';

void main() {
  testWidgets('App smoke test', (WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final settingsService = SettingsService(prefs);

    // Build our app and trigger a frame.
    await tester.pumpWidget(
      GestaltProviders(
        settingsService: settingsService,
        child: const GestaltApp(),
      ),
    );

    // Verify that the app title or main layout is present.
    // Since MainLayout might be complex, just ensuring it pumps without error is a good start.
    // checking for the title in the MaterialApp (this doesn't render text, but the app should build)
    expect(find.byType(GestaltApp), findsOneWidget);
  });
}
