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
