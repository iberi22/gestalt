import 'dart:io';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_animate/flutter_animate.dart';
import 'package:glass_kit/glass_kit.dart';

class TerminalScreen extends StatefulWidget {
  const TerminalScreen({super.key});

  @override
  State<TerminalScreen> createState() => _TerminalScreenState();
}

class _TerminalScreenState extends State<TerminalScreen> {
  final TextEditingController _inputController = TextEditingController();
  final ScrollController _scrollController = ScrollController();
  final FocusNode _focusNode = FocusNode();

  final List<String> _outputLines = [
    "Neural-Link CLI Terminal Initialized.",
    "Type 'help' for built-in commands, or use standard 'gestalt' cli arguments.",
    "System ready.",
    "",
  ];

  bool _isProcessing = false;

  void _scrollToBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollController.hasClients) {
        _scrollController.animateTo(
          _scrollController.position.maxScrollExtent,
          duration: const Duration(milliseconds: 200),
          curve: Curves.easeOut,
        );
      }
    });
  }

  void _appendOutput(String text) {
    setState(() {
      _outputLines.add(text);
    });
    _scrollToBottom();
  }

  Future<void> _submitCommand(String command) async {
    if (command.trim().isEmpty) return;

    _appendOutput("> $command");
    _inputController.clear();
    setState(() => _isProcessing = true);

    try {
      final args = command.trim().split(RegExp(r'\s+'));

      // Built-in aliases for better UX
      if (args[0] == 'clear') {
        setState(() {
          _outputLines.clear();
          _outputLines.add("Terminal cleared.");
        });
        setState(() => _isProcessing = false);
        return;
      }

      if (args[0] == 'help') {
        _appendOutput("Built-in commands:");
        _appendOutput("  clear   - Clear terminal output");
        _appendOutput("  help    - Show this help message");
        _appendOutput("All other commands are passed to gestalt_cli via 'cargo run -p gestalt_cli -- <args>'");
        setState(() => _isProcessing = false);
        return;
      }

      // Detect OS and workspace context assuming Dev Mode
      // Execute the rust CLI via cargo
      final processArgs = ['run', '-p', 'gestalt_cli', '--', ...args];

      // We assume the flutter app is run from `e:\scripts-python\gestalt-rust\gestalt_app`
      // so the rust workspace root is one level up.
      final result = await Process.run(
        'cargo',
        processArgs,
        workingDirectory: '..',
      );

      if (result.stdout.toString().isNotEmpty) {
        final lines = const LineSplitter().convert(result.stdout.toString());
        for (var line in lines) {
          _appendOutput(line);
        }
      }

      if (result.stderr.toString().isNotEmpty) {
        final errLines = const LineSplitter().convert(result.stderr.toString());
        bool skipCargoSpam = true;
        for (var line in errLines) {
          // Hide standard cargo "Compiling" or "Finished" messages if desired,
          // but we will keep them for transparency since this is a developer terminal.
          _appendOutput("ERR: $line");
        }
      }

    } catch (e) {
      _appendOutput("Exception: $e");
    } finally {
      setState(() => _isProcessing = false);
      _focusNode.requestFocus();
    }
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(24.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.terminal_rounded, color: Colors.purpleAccent, size: 32),
              const SizedBox(width: 16),
              Text(
                "Command Center",
                style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                      color: Colors.white,
                      fontWeight: FontWeight.bold,
                    ),
              ),
              const Spacer(),
              if (_isProcessing)
                const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    valueColor: AlwaysStoppedAnimation<Color>(Colors.purpleAccent),
                  ),
                ).animate().fadeIn(),
            ],
          ),
          const SizedBox(height: 24),
          Expanded(
            child: GlassContainer.clearGlass(
              height: double.infinity,
              width: double.infinity,
              borderRadius: BorderRadius.circular(16),
              blur: 15,
              borderWidth: 1,
              borderColor: Colors.white.withOpacity(0.1),
              color: Colors.black.withOpacity(0.5),
              child: Padding(
                padding: const EdgeInsets.all(16.0),
                child: Column(
                  children: [
                    Expanded(
                      child: ListView.builder(
                        controller: _scrollController,
                        itemCount: _outputLines.length,
                        itemBuilder: (context, index) {
                          final line = _outputLines[index];
                          final isCommand = line.startsWith('>');
                          final isError = line.startsWith('ERR:');

                          Color textColor = Colors.white70;
                          if (isCommand) textColor = Colors.cyanAccent;
                          if (isError) textColor = Colors.redAccent;

                          return Padding(
                            padding: const EdgeInsets.symmetric(vertical: 2.0),
                            child: SelectableText(
                              line,
                              style: TextStyle(
                                fontFamily: 'Courier', // Standard monospace fallback
                                color: textColor,
                                fontSize: 14,
                                height: 1.4,
                              ),
                            ),
                          );
                        },
                      ),
                    ),
                    const SizedBox(height: 8),
                    Container(
                      decoration: BoxDecoration(
                        color: Colors.white.withOpacity(0.05),
                        borderRadius: BorderRadius.circular(8),
                      ),
                      padding: const EdgeInsets.symmetric(horizontal: 12),
                      child: Row(
                        children: [
                          const Icon(Icons.arrow_forward_ios, size: 14, color: Colors.purpleAccent),
                          const SizedBox(width: 12),
                          Expanded(
                            child: TextField(
                              controller: _inputController,
                              focusNode: _focusNode,
                              style: const TextStyle(
                                fontFamily: 'Courier',
                                color: Colors.white,
                              ),
                              decoration: InputDecoration(
                                hintText: "Enter command...",
                                hintStyle: TextStyle(color: Colors.white.withOpacity(0.3)),
                                border: InputBorder.none,
                                isDense: true,
                              ),
                              onSubmitted: _submitCommand,
                              enabled: !_isProcessing,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
