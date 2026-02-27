import 'package:flutter/material.dart';
import 'package:flutter_animate/flutter_animate.dart';
import 'package:gestalt_terminal/src/rust/api/mcp.dart';

class McpUiRenderer extends StatelessWidget {
  final List<McpComponent> components;

  const McpUiRenderer({super.key, required this.components});

  @override
  Widget build(BuildContext context) {
    return ListView.builder(
      padding: const EdgeInsets.all(16.0),
      itemCount: components.length,
      itemBuilder: (context, index) {
        return _buildComponent(context, components[index])
            .animate()
            .fadeIn(duration: 400.ms, curve: Curves.easeOutQuad)
            .slideY(begin: 0.1, end: 0, duration: 400.ms, curve: Curves.easeOutQuad);
      },
    );
  }

  Widget _buildComponent(BuildContext context, McpComponent component) {
    return component.map(
      card: (c) => _GlassContainer(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              c.title.toUpperCase(),
              style: TextStyle(
                fontSize: 14,
                fontWeight: FontWeight.w900,
                letterSpacing: 1.5,
                color: Colors.greenAccent.shade400,
              ),
            ),
            const SizedBox(height: 12),
            Text(
              c.content,
              style: const TextStyle(
                color: Colors.white70,
                height: 1.5,
                fontSize: 14,
              ),
            ),
          ],
        ),
      ),
      button: (b) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 4.0, horizontal: 2.0),
        child: ElevatedButton(
          onPressed: () {
            debugPrint("Action Triggered: ${b.actionId}");
            handleMcpAction(actionId: b.actionId, value: "clicked");
          },
          style: ElevatedButton.styleFrom(
            backgroundColor: const Color(0xFF2A2A2A),
            foregroundColor: Colors.white,
            elevation: 0,
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(8),
              side: BorderSide(color: Colors.white.withValues(alpha: 0.1)),
            ),
          ),
          child: Text(
            b.label.toUpperCase(),
            style: const TextStyle(fontWeight: FontWeight.bold, letterSpacing: 1.0),
          ),
        ),
      ),
      markdown: (m) => Padding(
        padding: const EdgeInsets.only(bottom: 16.0),
        child: _GlassContainer(
          color: Colors.transparent,
          border: false,
          child: Text(
            m.content,
            style: const TextStyle(
              fontFamily: 'monospace',
              color: Color(0xFFE0E0E0),
              fontSize: 13,
            ),
          ),
        ),
      ),
      row: (r) => Padding(
        padding: const EdgeInsets.only(bottom: 12.0),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: r.children
              .map((c) => Expanded(
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 4.0),
                      child: _buildComponent(context, c),
                    ),
                  ))
              .toList(),
        ),
      ),
      column: (c) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: c.children.map((child) => _buildComponent(context, child)).toList(),
      ),
      image: (i) => Padding(
        padding: const EdgeInsets.only(bottom: 16.0),
        child: Center(
          child: _GlassContainer(
            padding: EdgeInsets.zero,
            child: ClipRRect(
              borderRadius: BorderRadius.circular(12),
              child: Image.asset(
                i.url,
                width: 100,
                height: 100,
                fit: BoxFit.contain,
                errorBuilder: (ctx, _, __) => const Icon(Icons.broken_image, size: 50, color: Colors.grey),
              ),
            ),
          ),
        ),
      ),
      progressBar: (p) => Padding(
        padding: const EdgeInsets.only(bottom: 16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              p.label,
              style: const TextStyle(color: Colors.white54, fontSize: 12),
            ),
            const SizedBox(height: 8),
            LinearProgressIndicator(
              value: p.progress,
              backgroundColor: Colors.white10,
              color: Colors.greenAccent,
              minHeight: 6,
              borderRadius: BorderRadius.circular(3),
            )
            .animate(onPlay: (controller) => controller.repeat())
            .shimmer(duration: 1200.ms, color: Colors.white24),
          ],
        ),
      ),
      input: (inp) => Padding(
        padding: const EdgeInsets.only(bottom: 12.0),
        child: TextField(
          style: const TextStyle(color: Colors.white),
          decoration: InputDecoration(
            labelText: inp.label,
            labelStyle: const TextStyle(color: Colors.white38),
            filled: true,
            fillColor: Colors.white.withValues(alpha: 0.05),
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
              borderSide: BorderSide.none,
            ),
            enabledBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
              borderSide: BorderSide(color: Colors.white.withValues(alpha: 0.1)),
            ),
            focusedBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
              borderSide: const BorderSide(color: Colors.greenAccent),
            ),
            suffixIcon: IconButton(
              icon: const Icon(Icons.send, size: 18, color: Colors.white54),
              onPressed: () {
                // In a real app, you'd use a TextEditingController managed by state
                handleMcpAction(actionId: inp.fieldId, value: "submitted_value_placeholder");
              },
            ),
          ),
          onSubmitted: (value) {
             handleMcpAction(actionId: inp.fieldId, value: value);
          },
        ),
      ),
      vfsMonitor: (v) => _VfsMonitorWidget(
        version: v.version,
        shadowStates: v.shadowStates,
        patchFeed: v.patchFeed,
      ),
    );
  }
}

class _VfsMonitorWidget extends StatelessWidget {
  final int version;
  final List<String> shadowStates;
  final List<String> patchFeed;

  const _VfsMonitorWidget({
    required this.version,
    required this.shadowStates,
    required this.patchFeed,
  });

  @override
  Widget build(BuildContext context) {
    return _GlassContainer(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                "VFS MONITOR",
                style: TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w900,
                  letterSpacing: 1.5,
                  color: Colors.blueAccent.shade200,
                ),
              ),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                decoration: BoxDecoration(
                  color: Colors.blueAccent.withValues(alpha: 0.2),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  "V=$version",
                  style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.bold,
                    color: Colors.blueAccent,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 16),
          const Text(
            "ACTIVE SHADOW STATES",
            style: TextStyle(fontSize: 10, fontWeight: FontWeight.bold, color: Colors.white38),
          ),
          const SizedBox(height: 8),
          if (shadowStates.isEmpty)
            const Text("No active agents", style: TextStyle(fontSize: 12, color: Colors.white24))
          else
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: shadowStates.map((agent) => Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                decoration: BoxDecoration(
                  color: Colors.white.withValues(alpha: 0.05),
                  borderRadius: BorderRadius.circular(4),
                  border: Border.all(color: Colors.white.withValues(alpha: 0.1)),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const Icon(Icons.person, size: 12, color: Colors.greenAccent),
                    const SizedBox(width: 4),
                    Text(agent, style: const TextStyle(fontSize: 11, color: Colors.white70)),
                  ],
                ),
              )).toList(),
            ),
          const SizedBox(height: 16),
          const Text(
            "PATCH FEED",
            style: TextStyle(fontSize: 10, fontWeight: FontWeight.bold, color: Colors.white38),
          ),
          const SizedBox(height: 8),
          ...patchFeed.reversed.take(5).map((patch) => Padding(
            padding: const EdgeInsets.only(bottom: 6.0),
            child: Row(
              children: [
                Icon(
                  patch.contains("conflict") ? Icons.error_outline : Icons.check_circle_outline,
                  size: 14,
                  color: patch.contains("conflict") ? Colors.redAccent : Colors.greenAccent,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    patch,
                    style: TextStyle(
                      fontSize: 12,
                      color: patch.contains("conflict") ? Colors.redAccent.withValues(alpha: 0.8) : Colors.white60,
                      fontFamily: 'monospace',
                    ),
                  ),
                ),
              ],
            ),
          )),
        ],
      ),
    );
  }
}

class _GlassContainer extends StatelessWidget {
  final Widget child;
  final EdgeInsetsGeometry padding;
  final Color? color;
  final bool border;

  const _GlassContainer({
    required this.child,
    this.padding = const EdgeInsets.all(16),
    this.color,
    this.border = true,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      padding: padding,
      margin: const EdgeInsets.only(bottom: 16.0),
      decoration: BoxDecoration(
        color: color ?? const Color(0xFF1A1A1A).withValues(alpha: 0.8),
        borderRadius: BorderRadius.circular(12),
        border: border ? Border.all(color: Colors.white.withValues(alpha: 0.08)) : null,
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.2),
            blurRadius: 10,
            offset: const Offset(0, 4),
          ),
        ],
      ),
      child: child,
    );
  }
}
