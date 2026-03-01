import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:web_socket_channel/web_socket_channel.dart';
import 'package:glass_kit/glass_kit.dart';
import 'package:flutter_animate/flutter_animate.dart';
import '../services/api_service.dart';
import '../models/agent.dart';
import '../models/project.dart';
import '../widgets/project_card.dart';
import '../widgets/agent_status_pill.dart';
import '../widgets/log_console.dart';

class DashboardScreen extends StatefulWidget {
  const DashboardScreen({super.key});

  @override
  State<DashboardScreen> createState() => _DashboardScreenState();
}

class _DashboardScreenState extends State<DashboardScreen> {
  final ApiService _api = ApiService();
  final ScrollController _logController = ScrollController();
  final TextEditingController _inputController = TextEditingController();

  List<Project> _projects = [];
  List<Agent> _agents = [];
  List<dynamic> _logs = [];
  WebSocketChannel? _channel;
  bool _isLoading = false;

  @override
  void initState() {
    super.initState();
    _fetchStaticData();
    _connectWebSocket();
  }

  void _fetchStaticData() async {
    try {
      final projects = await _api.getProjects();
      final agents = await _api.getAgents();
      if (mounted) {
        setState(() {
          _projects = projects;
          _agents = agents;
        });
      }
    } catch (e) {
      debugPrint("Error fetching static data: $e");
    }
  }

  void _connectWebSocket() {
    try {
      _channel = _api.timelineStream;
      _channel?.stream.listen(
        (message) {
          if (!mounted) return;
          try {
            final event = json.decode(message);
            setState(() {
              _logs.add(event);
            });
            _scrollToBottom();
          } catch (e) {
            debugPrint("WS JSON Error: $e");
          }
        },
        onError: (e) => debugPrint("WS Error: $e"),
        onDone: () {
          debugPrint("WS Closed. Reconnecting...");
          Future.delayed(const Duration(seconds: 3), () {
            if (mounted) _connectWebSocket();
          });
        },
      );
    } catch (e) {
      debugPrint("WebSocket Connection Error: $e");
    }
  }

  void _scrollToBottom() {
    if (_logController.hasClients) {
      final isNearBottom = _logController.offset >= _logController.position.maxScrollExtent - 100;
      if (isNearBottom || _logs.length < 50) {
        _logController.animateTo(
          _logController.position.maxScrollExtent + 200,
          duration: const Duration(milliseconds: 300),
          curve: Curves.easeOut,
        );
      }
    }
  }

  @override
  void dispose() {
    _channel?.sink.close();
    _logController.dispose();
    _inputController.dispose();
    super.dispose();
  }

  Future<void> _sendCommand() async {
    if (_inputController.text.isEmpty) return;
    setState(() => _isLoading = true);
    await _api.sendGoal(_inputController.text);
    _inputController.clear();
    setState(() => _isLoading = false);
  }

  Future<void> _showCreateProjectDialog() async {
    final TextEditingController controller = TextEditingController();
    await showDialog(
      context: context,
      builder: (context) => AlertDialog(
        backgroundColor: const Color(0xFF1E1E1E),
        title: const Text('Create New Project', style: TextStyle(color: Colors.white)),
        content: TextField(
          controller: controller,
          style: const TextStyle(color: Colors.white),
          decoration: const InputDecoration(
            hintText: "Project Name",
            hintStyle: TextStyle(color: Colors.white24),
            enabledBorder: UnderlineInputBorder(borderSide: BorderSide(color: Colors.white24)),
            focusedBorder: UnderlineInputBorder(borderSide: BorderSide(color: Colors.blueAccent)),
          ),
          autofocus: true,
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancel', style: TextStyle(color: Colors.white54)),
          ),
          ElevatedButton(
            onPressed: () async {
              if (controller.text.isNotEmpty) {
                final success = await _api.createProject(controller.text);
                if (success) {
                  if (mounted) Navigator.pop(context);
                  _fetchStaticData();
                }
              }
            },
            style: ElevatedButton.styleFrom(backgroundColor: Colors.blueAccent),
            child: const Text('Create', style: TextStyle(color: Colors.white)),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.transparent,
      body: Padding(
        padding: const EdgeInsets.all(32.0),
        child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Header
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    "System Architecture",
                    style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                          color: Colors.white,
                          fontWeight: FontWeight.bold,
                        ),
                  ),
                  const Text(
                    "Monitoring local and cloud neural nodes",
                    style: TextStyle(color: Colors.white54, fontSize: 16),
                  ),
                ],
              ),
              Row(
                children: [
                  ..._agents.map((a) => Padding(
                    padding: const EdgeInsets.only(left: 12.0),
                    child: AgentStatusPill(agent: a),
                  )),
                ],
              ),
            ],
          ).animate().fadeIn(duration: 600.ms).slideY(begin: -0.1, end: 0),

          const SizedBox(height: 40),

          // Primary Actions Row
          Row(
            children: [
              Expanded(
                child: _buildInfoCard(
                  "ACTIVE AGENTS",
                  "${_agents.length}",
                  Icons.psychology,
                  Colors.purpleAccent,
                ),
              ),
              const SizedBox(width: 24),
              Expanded(
                child: _buildInfoCard(
                  "CONNECTED REPOS",
                  "${_projects.length}",
                  Icons.account_tree,
                  Colors.blueAccent,
                ),
              ),
              const SizedBox(width: 24),
              Expanded(
                child: _buildInfoCard(
                  "SYSTEM STATUS",
                  "OPTIMAL",
                  Icons.verified_user,
                  Colors.greenAccent,
                ),
              ),
            ],
          ).animate().fadeIn(delay: 200.ms).slideY(begin: 0.1, end: 0),

          const SizedBox(height: 40),

          // Command Bar
          GlassContainer.clearGlass(
            height: 70,
            width: double.infinity,
            borderRadius: BorderRadius.circular(16),
            borderWidth: 1,
            borderColor: Colors.white.withOpacity(0.1),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 20),
              child: Row(
                children: [
                  const Icon(Icons.flash_on, color: Colors.orangeAccent),
                  const SizedBox(width: 16),
                  Expanded(
                    child: TextField(
                      controller: _inputController,
                      style: const TextStyle(color: Colors.white, fontSize: 18),
                      decoration: const InputDecoration(
                        hintText: "Issue a new directives to the swarm...",
                        hintStyle: TextStyle(color: Colors.white24),
                        border: InputBorder.none,
                      ),
                      onSubmitted: (_) => _sendCommand(),
                    ),
                  ),
                  if (_isLoading)
                    const CircularProgressIndicator(color: Colors.purpleAccent, strokeWidth: 2)
                  else
                    IconButton(
                      icon: const Icon(Icons.send_rounded, color: Colors.purpleAccent),
                      onPressed: _sendCommand,
                    ),
                ],
              ),
            ),
          ).animate().fadeIn(delay: 400.ms).scale(begin: const Offset(0.95, 0.95)),

          const SizedBox(height: 40),

          // Log Terminal
          Expanded(
            child: GlassContainer.clearGlass(
              width: double.infinity,
              height: double.infinity,
              borderRadius: BorderRadius.circular(24),
              borderWidth: 1,
              borderColor: Colors.white.withOpacity(0.05),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Padding(
                    padding: const EdgeInsets.all(16.0),
                    child: Row(
                      children: [
                        const Icon(Icons.terminal, color: Colors.greenAccent, size: 20),
                        const SizedBox(width: 12),
                        const Text(
                          "NEURAL LOG STREAM",
                          style: TextStyle(color: Colors.greenAccent, fontWeight: FontWeight.bold, letterSpacing: 1),
                        ),
                      ],
                    ),
                  ),
                  const Divider(color: Colors.white10, height: 1),
                  Expanded(
                    child: LogConsole(events: _logs, scrollController: _logController),
                  ),
                ],
              ),
            ),
          ).animate().fadeIn(delay: 600.ms),
        ],
      ),
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: _showCreateProjectDialog,
        backgroundColor: Colors.blueAccent,
        child: const Icon(Icons.add_to_photos, color: Colors.white),
      ).animate().fadeIn(delay: 800.ms).scale(),
    );
  }

  Widget _buildInfoCard(String title, String value, IconData icon, Color color) {
    return GlassContainer.clearGlass(
      height: 120,
      width: double.infinity,
      borderRadius: BorderRadius.circular(20),
      borderWidth: 1,
      borderColor: color.withOpacity(0.2),
      child: Padding(
        padding: const EdgeInsets.all(20.0),
        child: Row(
          children: [
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: color.withOpacity(0.1),
                shape: BoxShape.circle,
              ),
              child: Icon(icon, color: color, size: 28),
            ),
            const SizedBox(width: 20),
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Text(title, style: const TextStyle(color: Colors.white54, fontSize: 12, fontWeight: FontWeight.bold)),
                Text(value, style: const TextStyle(color: Colors.white, fontSize: 24, fontWeight: FontWeight.bold)),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
