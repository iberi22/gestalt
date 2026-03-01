import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:web_socket_channel/web_socket_channel.dart';
import 'services/api_service.dart';

class AgentView extends StatefulWidget {
  const AgentView({super.key});

  @override
  State<AgentView> createState() => _AgentViewState();
}

class _AgentViewState extends State<AgentView> {
  final TextEditingController _controller = TextEditingController();
  final ScrollController _scrollController = ScrollController();
  String _agentStatus = "Offline";
  List<dynamic> _events = [];
  bool _isLoading = false;
  WebSocketChannel? _channel;

  @override
  void initState() {
    super.initState();
    _fetchAgentsInit();
    _connectWebSocket();
  }

  @override
  void dispose() {
    _channel?.sink.close();
    _controller.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  Future<void> _fetchAgentsInit() async {
    final api = context.read<ApiService>();
    final agents = await api.getAgents();
    if (agents.isNotEmpty) {
      bool anyBusy = agents.any((a) => a.status == 'busy');
      bool anyOnline = agents.any((a) => a.status == 'online');
      if (mounted) {
        setState(() {
          _agentStatus = anyBusy ? "Busy 🏃" : (anyOnline ? "Online 🟢" : "Offline ⚫");
        });
      }
    }
  }

  void _connectWebSocket() {
    try {
      final api = context.read<ApiService>();
      _channel = api.timelineStream;
      _channel?.stream.listen(
        (message) {
          if (!mounted) return;
          try {
            final event = json.decode(message);
            setState(() {
              _events.add(event);
            });
             if (_scrollController.hasClients) {
               final isNearBottom = _scrollController.offset >= _scrollController.position.maxScrollExtent - 100;
               if (isNearBottom || _events.length < 50) {
                 _scrollController.animateTo(
                   _scrollController.position.maxScrollExtent + 200,
                   duration: const Duration(milliseconds: 300),
                   curve: Curves.easeOut
                 );
               }
            }
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
        }
      );
    } catch (e) {
      debugPrint("WebSocket Connection Error: $e");
    }
  }

  Future<void> _runOrchestration() async {
    if (_controller.text.isEmpty) return;

    final goal = _controller.text;
    _controller.clear();
    setState(() => _isLoading = true);

    try {
      final api = context.read<ApiService>();
      await api.sendGoal(goal);
    } catch (e) {
      // ignore
    } finally {
      if (mounted) {
        setState(() => _isLoading = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Row(
          children: [
            const Text('Gestalt Agent'),
            const Spacer(),
            Text(_agentStatus, style: const TextStyle(fontSize: 16)),
          ],
        ),
        backgroundColor: Colors.black87,
        foregroundColor: Colors.white,
      ),
      body: Column(
        children: [
          Expanded(
            child: ListView.builder(
              controller: _scrollController,
              itemCount: _events.length,
              padding: const EdgeInsets.all(16),
              itemBuilder: (context, index) {
                final event = _events[index];
                return _buildEventCard(event);
              },
            ),
          ),
          _buildInputArea(),
        ],
      ),
    );
  }

  Widget _buildEventCard(Map<String, dynamic> event) {
    final type = event['event_type'] ?? 'unknown';

    Color color = Colors.grey.shade200;
    IconData icon = Icons.info_outline;

    if (type.contains('llm')) {
      color = Colors.purple.shade50;
      icon = Icons.psychology;
    } else if (type == 'create_project' || type == 'create_task') {
      color = Colors.green.shade50;
      icon = Icons.check_circle_outline;
    } else if (type == 'error') {
      color = Colors.red.shade50;
      icon = Icons.error_outline;
    }

    return Card(
      color: color,
      margin: const EdgeInsets.only(bottom: 8),
      child: ListTile(
        leading: Icon(icon, color: Colors.black54),
        title: Text(type, style: const TextStyle(fontWeight: FontWeight.bold)),
        subtitle: Text(json.encode(event['payload'] ?? event['metadata'] ?? {})),
        trailing: Text(
          event['timestamp']?.toString().split('T').last.split('.').first ?? '',
          style: const TextStyle(fontSize: 12),
        ),
      ),
    );
  }

  Widget _buildInputArea() {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: Colors.white,
        boxShadow: [
          BoxShadow(blurRadius: 5, color: Colors.black12, offset: const Offset(0, -2))
        ],
      ),
      child: Row(
        children: [
          Expanded(
            child: TextField(
              controller: _controller,
              decoration: const InputDecoration(
                hintText: 'Give the agent a goal...',
                border: OutlineInputBorder(),
              ),
              onSubmitted: (_) => _runOrchestration(),
            ),
          ),
          const SizedBox(width: 8),
          IconButton(
            onPressed: _isLoading ? null : _runOrchestration,
            icon: _isLoading
              ? const SizedBox(width: 24, height: 24, child: CircularProgressIndicator(strokeWidth: 2))
              : const Icon(Icons.send, color: Colors.blueAccent),
          ),
        ],
      ),
    );
  }
}
