import 'package:flutter/material.dart';
import '../models/agent.dart';

class AgentStatusPill extends StatelessWidget {
  final Agent agent;

  const AgentStatusPill({super.key, required this.agent});

  @override
  Widget build(BuildContext context) {
    Color color;
    IconData icon;

    switch (agent.status.toLowerCase()) {
      case 'online':
        color = Colors.green;
        icon = Icons.check_circle;
        break;
      case 'busy':
        color = Colors.orange;
        icon = Icons.directions_run;
        break;
      default:
        color = Colors.grey;
        icon = Icons.offline_bolt;
    }

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.2),
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: color.withValues(alpha: 0.5)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 14, color: color),
          const SizedBox(width: 6),
          Text(
            agent.name.toUpperCase(),
            style: TextStyle(
              color: color,
              fontWeight: FontWeight.bold,
              fontSize: 12,
            ),
          ),
          const SizedBox(width: 4),
          Text(
            '(${agent.status})',
            style: TextStyle(
              color: color.withValues(alpha: 0.8),
              fontSize: 12,
            ),
          ),
        ],
      ),
    );
  }
}
