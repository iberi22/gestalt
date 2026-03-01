import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:flutter/foundation.dart';

class ApiClient {
  final String baseUrl;

  ApiClient({this.baseUrl = 'http://127.0.0.1:3000'}); // Default to local Axum server

  Future<bool> healthCheck() async {
    try {
      final response = await http.get(Uri.parse('$baseUrl/health'));
      return response.statusCode == 200;
    } catch (e) {
      debugPrint('Health check failed: $e');
      return false;
    }
  }

  Future<List<dynamic>> getTimeline() async {
    try {
      final response = await http.get(Uri.parse('$baseUrl/timeline'));
      if (response.statusCode == 200) {
        return jsonDecode(response.body) as List<dynamic>;
      } else {
        debugPrint('Failed to load timeline: ${response.statusCode}');
        return [];
      }
    } catch (e) {
      debugPrint('Error fetching timeline: $e');
      return [];
    }
  }

  Future<bool> orchestrate(String goal) async {
    try {
      final response = await http.post(
        Uri.parse('$baseUrl/orchestrate'),
        headers: {'Content-Type': 'application/json'},
        body: jsonEncode({'goal': goal}),
      );
      return response.statusCode == 202;
    } catch (e) {
      debugPrint('Error triggering orchestration: $e');
      return false;
    }
  }

  Future<bool> sendChat(String message) async {
    try {
      final response = await http.post(
        Uri.parse('$baseUrl/chat'),
        headers: {'Content-Type': 'application/json'},
        body: jsonEncode({'message': message}),
      );
      return response.statusCode == 202;
    } catch (e) {
      debugPrint('Error sending chat: $e');
      return false;
    }
  }
}
