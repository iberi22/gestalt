import 'package:flutter/material.dart';
import 'package:xterm/xterm.dart';
import 'dart:async';
import 'package:gestalt_terminal/src/rust/api/terminal.dart';
import 'package:gestalt_terminal/src/rust/api/mcp.dart';
import 'package:gestalt_terminal/src/rust/frb_generated.dart';
import 'package:gestalt_terminal/mcp_ui.dart';
import 'package:gestalt_terminal/services/api_client.dart';

Future<void> main() async {
  await RustLib.init();
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'Gestalt Terminal',
      theme: ThemeData.dark().copyWith(
        scaffoldBackgroundColor: const Color(0xFF0A0A0A),
        scrollbarTheme: ScrollbarThemeData(
           thumbColor: WidgetStateProperty.all(Colors.grey[800]),
        ),
      ),
      home: const MainScreen(),
    );
  }
}

class MainScreen extends StatefulWidget {
  const MainScreen({super.key});

  @override
  State<MainScreen> createState() => _MainScreenState();
}

class _MainScreenState extends State<MainScreen> {
  // xterm terminal
  final Terminal _terminal = Terminal(maxLines: 10000);
  late final TerminalController _terminalController;

  // Services
  final ApiClient _apiClient = ApiClient();
  Timer? _pollingTimer;

  // MCP UI components
  final List<McpComponent> _mcpComponents = [];
  String _agentStatus = "OFFLINE";
  final Set<String> _processedEventIds = {};

  // VFS Monitor State
  int _vfsVersion = 0;
  final Set<String> _shadowStates = {};
  final List<String> _patchFeed = [];

  @override
  void initState() {
    super.initState();
    _terminalController = TerminalController();

    // Listen to input from the terminal (user typing) and send to PTY
    _terminal.onOutput = (data) {
      // Check for chat command interception (simple heuristic for now)
      // Note: This relies on the PTY echoing back or us intercepting before sending.
      // Since 'data' here is raw input chars, intercepting complex commands is hard without a buffer.
      // We rely on the Rust side interception for 'gestalt' commands for now.
      sendTerminalInput(input: data);
    };

    _initStreams();
    _startPolling();

    // Welcome message
    Future.delayed(const Duration(milliseconds: 500), () {
      _terminal.write("\r\n\x1B[1;32mGestalt Terminal System v1.0\x1B[0m\r\n");
      _terminal.write("Type 'gestalt scan' to analyze project...\r\n\r\n$ ");
    });
  }

  @override
  void dispose() {
    _pollingTimer?.cancel();
    super.dispose();
  }

  Future<void> _initStreams() async {
    // Initialize Terminal PTY
    // Note: initTerminal returns a stream of output from the shell
    final terminalStream = initTerminal();
    terminalStream.listen((data) {
      _terminal.write(data);
    });

    // Initialize MCP UI Stream (Local Rust events)
    final mcpStream = streamMcpUi();
    mcpStream.listen((component) {
      setState(() {
        _mcpComponents.add(component);
      });
    });
  }

  void _startPolling() {
    _pollingTimer = Timer.periodic(const Duration(seconds: 2), (timer) async {
      // Health Check
      final isOnline = await _apiClient.healthCheck();
      setState(() {
        _agentStatus = isOnline ? "ONLINE" : "OFFLINE";
      });

      if (isOnline) {
        // Poll Timeline
        final events = await _apiClient.getTimeline();
        for (var event in events) {
          final id = event['id'] as String? ?? 'unknown';
          if (!_processedEventIds.contains(id)) {
            _processedEventIds.add(id);
            _addEventToUi(event);
          }
        }
      }
    });
  }

  void _addEventToUi(dynamic event) {
    // Convert backend event to MCP Component
    final eventType = event['event_type'] as String? ?? 'Unknown';
    final payload = event['payload']; // Assuming JSON payload

    McpComponent? component;

    if (eventType == 'TaskCreated' || eventType == 'TaskStarted') {
       final taskDesc = payload.toString();
       component = McpComponent.card(
         title: "New Task: $eventType",
         content: taskDesc.length > 100 ? "${taskDesc.substring(0, 100)}..." : taskDesc
       );
    } else if (eventType == 'AgentConnected') {
       component = const McpComponent.card(title: "System", content: "Agent Connected to Timeline.");
    } else if (eventType.startsWith('vfs_')) {
       _handleVfsEvent(eventType, event['agent_id'] as String, payload);
       return;
    } else {
       // Generic fallback
       component = McpComponent.markdown(content: "**Event:** $eventType\n\n```json\n$payload\n```");
    }

    if (component != null) {
      setState(() {
        _mcpComponents.add(component);
      });
    }
  }

  void _handleVfsEvent(String eventType, String agentId, dynamic payload) {
    setState(() {
      final shortId = agentId.length > 8 ? agentId.substring(0, 8) : agentId;
      if (eventType == 'vfs_patch_applied') {
        _vfsVersion = payload['version'] as int? ?? _vfsVersion;
        _shadowStates.add(agentId);
        _patchFeed.add("$shortId: applied patch to ${payload['path']}");
      } else if (eventType == 'vfs_lock_acquired') {
        _shadowStates.add(agentId);
        _patchFeed.add("$shortId: acquired lock on ${payload['path']}");
      } else if (eventType == 'vfs_lock_conflict') {
        _patchFeed.add("$shortId: lock conflict on ${payload['path']} (held by ${payload['owner']})");
      } else if (eventType == 'vfs_flush_completed') {
        _vfsVersion = payload['version'] as int? ?? _vfsVersion;
        _shadowStates.clear(); // Flush is global in OverlayFs
        _patchFeed.add("System: VFS flushed to disk (V=$_vfsVersion)");
      }

      // Cap patch feed to prevent memory leak
      if (_patchFeed.length > 100) {
        _patchFeed.removeAt(0);
      }

      // Keep only one VfsMonitor at the top or update it in place.
      // For simplicity, let's replace/ensure it's present.
      _mcpComponents.removeWhere((c) => c.whenOrNull(vfsMonitor: (v) => true) ?? false);
      _mcpComponents.insert(0, McpComponent.vfsMonitor(
        version: _vfsVersion,
        shadowStates: _shadowStates.toList(),
        patchFeed: _patchFeed.toList(),
      ));
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Row(
        children: [
          // Terminal Area (Left)
          Expanded(
            flex: 2,
            child: Container(
              color: Colors.black,
              padding: const EdgeInsets.all(8),
              child: TerminalView(
                _terminal,
                controller: _terminalController,
                autofocus: true,
                theme: TerminalThemes.defaultTheme,
              ),
            ),
          ),

          // Divider
          const VerticalDivider(width: 1, color: Colors.grey),

          // MCP UI Area (Right)
          Expanded(
            flex: 1,
            child: Column(
              children: [
                Container(
                    width: double.infinity,
                    padding: const EdgeInsets.all(12),
                    alignment: Alignment.centerLeft,
                    color: const Color(0xFF1A1A1A),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        const Text("AGENT CONTEXT", style: TextStyle(fontWeight: FontWeight.bold, letterSpacing: 1.2)),
                        Container(
                          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                          decoration: BoxDecoration(
                            color: _agentStatus == "IDLE" ? Colors.grey[800] : Colors.green[900],
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: Text(_agentStatus, style: const TextStyle(fontSize: 10, fontWeight: FontWeight.bold)),
                        )
                      ],
                    )
                ),
                Expanded(
                  child: McpUiRenderer(components: _mcpComponents),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
