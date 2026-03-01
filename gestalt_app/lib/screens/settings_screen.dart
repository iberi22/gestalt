import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../services/api_service.dart';
import '../services/settings_service.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  final _baseUrlController = TextEditingController();
  final _tokenController = TextEditingController();
  List<String> _models = [];
  String? _selectedModel;
  bool _isLoading = true;

  @override
  void initState() {
    super.initState();
    final settings = context.read<SettingsService>();
    _baseUrlController.text = settings.baseUrl;
    _tokenController.text = settings.token;
    _loadModels();
  }

  @override
  void dispose() {
    _baseUrlController.dispose();
    _tokenController.dispose();
    super.dispose();
  }

  Future<void> _loadModels() async {
    final api = context.read<ApiService>();
    final models = await api.getModels();
    if (mounted) {
      setState(() {
        _models = models;
        // Default to known Sonnet if list empty or not set
        if (_selectedModel == null && _models.isNotEmpty) {
          _selectedModel = _models.firstWhere(
              (m) => m.contains("sonnet") && m.contains("3-5"),
              orElse: () => _models.first);
        }
        _isLoading = false;
      });
    }
  }

  Future<void> _updateModel(String? newValue) async {
    if (newValue == null) return;
    setState(() {
      _selectedModel = newValue;
    });

    final api = context.read<ApiService>();
    final success = await api.setModel(newValue);
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(success ? "Model updated to $newValue" : "Failed to update model"),
          backgroundColor: success ? Colors.green : Colors.red,
        ),
      );
    }
  }

  void _saveSettings() {
    final baseUrl = _baseUrlController.text.trim();
    final token = _tokenController.text.trim();

    if (baseUrl.isNotEmpty && token.isNotEmpty) {
      context.read<SettingsService>().setCredentials(baseUrl, token);
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Settings saved successfully'), backgroundColor: Colors.green),
      );
    } else {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Please enter both Backend URL and API Token'), backgroundColor: Colors.red),
      );
    }
  }

  void _logout() {
    context.read<SettingsService>().clearCredentials();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.transparent, // Inherits from MainLayout
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(32.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              "System Settings",
              style: TextStyle(color: Colors.white, fontSize: 32, fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 32),

            // Connection Settings
            _buildSection(
              title: "Connection Configuration",
              subtitle: "Manage your connection to the Gestalt backend.",
              child: Column(
                children: [
                  TextField(
                    controller: _baseUrlController,
                    style: const TextStyle(color: Colors.white),
                    decoration: _inputDecoration("Backend URL"),
                  ),
                  const SizedBox(height: 16),
                  TextField(
                    controller: _tokenController,
                    obscureText: true,
                    style: const TextStyle(color: Colors.white),
                    decoration: _inputDecoration("API Token"),
                  ),
                  const SizedBox(height: 24),
                  Row(
                    children: [
                      ElevatedButton(
                        onPressed: _saveSettings,
                        style: ElevatedButton.styleFrom(backgroundColor: Colors.blueAccent, foregroundColor: Colors.white),
                        child: const Text("Save Changes"),
                      ),
                      const SizedBox(width: 16),
                      OutlinedButton(
                        onPressed: _logout,
                        style: OutlinedButton.styleFrom(side: const BorderSide(color: Colors.redAccent), foregroundColor: Colors.redAccent),
                        child: const Text("Logout / Clear Credentials"),
                      ),
                    ],
                  ),
                ],
              ),
            ),

            const SizedBox(height: 32),

            // AI Model Configuration
            _buildSection(
              title: "AI Model Configuration",
              subtitle: "Select the active Bedrock Foundation Model for the coding agent.",
              child: _isLoading
                  ? const CircularProgressIndicator()
                  : Container(
                      padding: const EdgeInsets.symmetric(horizontal: 16),
                      decoration: BoxDecoration(
                        color: const Color(0xFF2C2C2C),
                        borderRadius: BorderRadius.circular(8),
                        border: Border.all(color: Colors.white24),
                      ),
                      child: DropdownButtonHideUnderline(
                        child: DropdownButton<String>(
                          value: _selectedModel,
                          isExpanded: true,
                          dropdownColor: const Color(0xFF2C2C2C),
                          style: const TextStyle(color: Colors.white, fontSize: 16),
                          icon: const Icon(Icons.arrow_drop_down, color: Colors.blueAccent),
                          items: _models.map((String model) {
                            return DropdownMenuItem<String>(
                              value: model,
                              child: Text(model),
                            );
                          }).toList(),
                          onChanged: _updateModel,
                          hint: const Text("Select a model", style: TextStyle(color: Colors.white38)),
                        ),
                      ),
                    ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildSection({required String title, required String subtitle, required Widget child}) {
    return Container(
      padding: const EdgeInsets.all(24),
      decoration: BoxDecoration(
        color: const Color(0xFF1E1E1E),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: Colors.white10),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            title,
            style: const TextStyle(color: Colors.white, fontSize: 20, fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 8),
          Text(
            subtitle,
            style: const TextStyle(color: Colors.white54, fontSize: 14),
          ),
          const SizedBox(height: 24),
          child,
        ],
      ),
    );
  }

  InputDecoration _inputDecoration(String label) {
    return InputDecoration(
      labelText: label,
      labelStyle: const TextStyle(color: Colors.white70),
      enabledBorder: OutlineInputBorder(
        borderSide: const BorderSide(color: Colors.white12),
        borderRadius: BorderRadius.circular(8),
      ),
      focusedBorder: OutlineInputBorder(
        borderSide: const BorderSide(color: Colors.blueAccent),
        borderRadius: BorderRadius.circular(8),
      ),
      filled: true,
      fillColor: const Color(0xFF2C2C2C),
    );
  }
}
