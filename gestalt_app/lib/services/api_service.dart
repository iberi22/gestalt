import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:web_socket_channel/web_socket_channel.dart';
import '../models/agent.dart';
import '../models/project.dart';
import '../models/task.dart';

class ApiService {
  final String baseUrl;
  final String token;

  ApiService({
    required this.baseUrl,
    required this.token,
  });

  String get wsUrl {
    if (baseUrl.startsWith('https://')) {
      return baseUrl.replaceFirst('https://', 'wss://');
    } else if (baseUrl.startsWith('http://')) {
      return baseUrl.replaceFirst('http://', 'ws://');
    }
    return baseUrl;
  }

  Map<String, String> get _authHeaders => {
    'Authorization': 'Bearer $token',
  };

  Map<String, String> get _postHeaders => {
    'Content-Type': 'application/json',
    'Authorization': 'Bearer $token',
  };

  WebSocketChannel get timelineStream {
    final uri = Uri.parse('$wsUrl/stream?token=$token');
    return WebSocketChannel.connect(uri);
  }

  Future<List<Agent>> getAgents() async {
    try {
      final response = await http.get(Uri.parse('$baseUrl/agents'), headers: _authHeaders);
      if (response.statusCode == 200) {
        final List<dynamic> jsonList = json.decode(response.body);
        return jsonList.map((json) => Agent.fromJson(json)).toList();
      }
    } catch (e) {
      print('Error fetching agents: $e');
    }
    return [];
  }

  Future<List<Project>> getProjects() async {
    try {
      final response = await http.get(Uri.parse('$baseUrl/projects'), headers: _authHeaders);
      if (response.statusCode == 200) {
        final List<dynamic> jsonList = json.decode(response.body);
        return jsonList.map((json) => Project.fromJson(json)).toList();
      }
    } catch (e) {
      print('Error fetching projects: $e');
    }
    return [];
  }

  Future<List<dynamic>> getTimeline() async {
    try {
      final response = await http.get(Uri.parse('$baseUrl/timeline'), headers: _authHeaders);
      if (response.statusCode == 200) {
        return json.decode(response.body);
      }
    } catch (e) {
      print('Error fetching timeline: $e');
    }
    return [];
  }

  Future<List<Task>> getTasks() async {
    try {
      final response = await http.get(Uri.parse('$baseUrl/tasks'), headers: _authHeaders);
      if (response.statusCode == 200) {
        final List<dynamic> jsonList = json.decode(response.body);
        return jsonList.map((json) => Task.fromJson(json)).toList();
      }
    } catch (e) {
      print('Error fetching tasks: $e');
    }
    return [];
  }

  Future<void> sendGoal(String goal) async {
    try {
      await http.post(
        Uri.parse('$baseUrl/orchestrate'),
        headers: _postHeaders,
        body: json.encode({'goal': goal}),
      );
    } catch (e) {
      print('Error sending orchestration goal: $e');
    }
  }

  // --- Project Management ---

  Future<bool> createProject(String name) async {
    try {
      final response = await http.post(
        Uri.parse('$baseUrl/projects'),
        headers: _postHeaders,
        body: json.encode({'name': name}),
      );
      return response.statusCode == 201;
    } catch (e) {
      print('Error creating project: $e');
      return false;
    }
  }

  Future<bool> deleteProject(String id) async {
    try {
      final response = await http.delete(Uri.parse('$baseUrl/projects/$id'), headers: _authHeaders);
      return response.statusCode == 204;
    } catch (e) {
      print('Error deleting project: $e');
      return false;
    }
  }

  // --- Task Management ---

  Future<bool> createTask(String project, String description) async {
    try {
      final response = await http.post(
        Uri.parse('$baseUrl/tasks'),
        headers: _postHeaders,
        body: json.encode({'project': project, 'description': description}),
      );
      return response.statusCode == 201;
    } catch (e) {
      print('Error creating task: $e');
      return false;
    }
  }

  Future<bool> updateTask(String id, {String? description, String? status}) async {
    try {
      final body = <String, dynamic>{};
      if (description != null) body['description'] = description;
      if (status != null) body['status'] = status;

      final response = await http.put(
        Uri.parse('$baseUrl/tasks/$id'),
        headers: _postHeaders,
        body: json.encode(body),
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error updating task: $e');
      return false;
    }
  }

  Future<bool> deleteTask(String id) async {
    try {
      final response = await http.delete(Uri.parse('$baseUrl/tasks/$id'), headers: _authHeaders);
      return response.statusCode == 204;
    } catch (e) {
      print('Error deleting task: $e');
      return false;
    }
  }

  Future<bool> runTask(String id) async {
    try {
      final response = await http.post(Uri.parse('$baseUrl/tasks/$id/run'), headers: _authHeaders);
      return response.statusCode == 200;
    } catch (e) {
      print('Error running task: $e');
      return false;
    }
  }

  Future<bool> checkHealth() async {
    try {
      final response = await http.get(Uri.parse('$baseUrl/health'), headers: _authHeaders);
      return response.statusCode == 200;
    } catch (e) {
      print('Error checking health: $e');
      return false;
    }
  }

  Future<List<String>> getModels() async {
    try {
      final response = await http.get(Uri.parse('$baseUrl/models'), headers: _authHeaders);
      if (response.statusCode == 200) {
        final List<dynamic> data = json.decode(response.body);
        return data.cast<String>();
      }
      return [];
    } catch (e) {
      print('Error fetching models: $e');
      return [];
    }
  }

  Future<bool> setModel(String modelId) async {
    try {
      final response = await http.post(
        Uri.parse('$baseUrl/config/model'),
        headers: _postHeaders,
        body: json.encode({'model_id': modelId}),
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error setting model: $e');
      return false;
    }
  }

  /// Get current agent mode (build/plan)
  Future<Map<String, dynamic>?> getMode() async {
    try {
      final response = await http.get(Uri.parse('$baseUrl/config/mode'), headers: _authHeaders);
      if (response.statusCode == 200) {
        return json.decode(response.body);
      }
    } catch (e) {
      print('Error getting mode: $e');
    }
    return null;
  }

  /// Set agent mode (build/plan)
  Future<bool> setMode(String mode) async {
    try {
      final response = await http.post(
        Uri.parse('$baseUrl/config/mode'),
        headers: _postHeaders,
        body: json.encode({'mode': mode}),
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error setting mode: $e');
      return false;
    }
  }
}
