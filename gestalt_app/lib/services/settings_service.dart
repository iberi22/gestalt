import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

class SettingsService extends ChangeNotifier {
  static const String _keyBaseUrl = 'gestalt_base_url';
  static const String _keyToken = 'gestalt_api_token';

  final SharedPreferences _prefs;

  SettingsService(this._prefs);

  String get baseUrl => _prefs.getString(_keyBaseUrl) ?? 'http://127.0.0.1:3000';
  String get token => _prefs.getString(_keyToken) ?? '';

  bool get hasCredentials => token.isNotEmpty;

  Future<void> setCredentials(String baseUrl, String token) async {
    await _prefs.setString(_keyBaseUrl, baseUrl);
    await _prefs.setString(_keyToken, token);
    notifyListeners();
  }

  Future<void> clearCredentials() async {
    await _prefs.remove(_keyBaseUrl);
    await _prefs.remove(_keyToken);
    notifyListeners();
  }
}
