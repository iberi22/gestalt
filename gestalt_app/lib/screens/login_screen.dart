import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:glass_kit/glass_kit.dart';
import '../services/settings_service.dart';

class LoginScreen extends StatefulWidget {
  const LoginScreen({super.key});

  @override
  State<LoginScreen> createState() => _LoginScreenState();
}

class _LoginScreenState extends State<LoginScreen> {
  final _baseUrlController = TextEditingController(text: 'http://127.0.0.1:3000');
  final _tokenController = TextEditingController();

  @override
  void dispose() {
    _baseUrlController.dispose();
    _tokenController.dispose();
    super.dispose();
  }

  void _login() {
    final baseUrl = _baseUrlController.text.trim();
    final token = _tokenController.text.trim();

    if (baseUrl.isNotEmpty && token.isNotEmpty) {
      context.read<SettingsService>().setCredentials(baseUrl, token);
    } else {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Please enter both Backend URL and API Token')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Container(
        decoration: const BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
            colors: [Color(0xFF0F0C29), Color(0xFF302B63), Color(0xFF24243E)],
          ),
        ),
        child: Center(
          child: SingleChildScrollView(
            child: GlassContainer.clearGlass(
              width: 400,
              height: 450,
              borderRadius: BorderRadius.circular(24),
              borderWidth: 1,
              borderColor: Colors.white.withOpacity(0.1),
              child: Padding(
                padding: const EdgeInsets.all(32.0),
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    const Icon(Icons.auto_awesome_outlined, size: 64, color: Colors.purpleAccent),
                    const SizedBox(height: 24),
                    const Text(
                      'NEURAL LINK',
                      style: TextStyle(
                        color: Colors.white,
                        fontSize: 24,
                        fontWeight: FontWeight.bold,
                        letterSpacing: 4,
                      ),
                    ),
                    const SizedBox(height: 32),
                    TextField(
                      controller: _baseUrlController,
                      style: const TextStyle(color: Colors.white),
                      decoration: InputDecoration(
                        labelText: 'Backend URL',
                        labelStyle: const TextStyle(color: Colors.white70),
                        enabledBorder: OutlineInputBorder(
                          borderSide: BorderSide(color: Colors.white.withOpacity(0.3)),
                          borderRadius: BorderRadius.circular(12),
                        ),
                        focusedBorder: OutlineInputBorder(
                          borderSide: const BorderSide(color: Colors.purpleAccent),
                          borderRadius: BorderRadius.circular(12),
                        ),
                      ),
                    ),
                    const SizedBox(height: 16),
                    TextField(
                      controller: _tokenController,
                      obscureText: true,
                      style: const TextStyle(color: Colors.white),
                      decoration: InputDecoration(
                        labelText: 'API Token',
                        labelStyle: const TextStyle(color: Colors.white70),
                        enabledBorder: OutlineInputBorder(
                          borderSide: BorderSide(color: Colors.white.withOpacity(0.3)),
                          borderRadius: BorderRadius.circular(12),
                        ),
                        focusedBorder: OutlineInputBorder(
                          borderSide: const BorderSide(color: Colors.purpleAccent),
                          borderRadius: BorderRadius.circular(12),
                        ),
                      ),
                    ),
                    const SizedBox(height: 32),
                    SizedBox(
                      width: double.infinity,
                      height: 50,
                      child: ElevatedButton(
                        onPressed: _login,
                        style: ElevatedButton.styleFrom(
                          backgroundColor: Colors.purpleAccent,
                          foregroundColor: Colors.white,
                          shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(12),
                          ),
                        ),
                        child: const Text('Connect to Neural Link', style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold)),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
