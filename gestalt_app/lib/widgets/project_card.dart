import 'package:flutter/material.dart';
import '../models/project.dart';
import '../screens/project_detail_screen.dart'; // Import detail screen

class ProjectCard extends StatelessWidget {
  final Project project;
  final VoidCallback? onDelete;

  const ProjectCard({super.key, required this.project, this.onDelete});

  @override
  Widget build(BuildContext context) {
    Color statusColor;
    if (project.status == 'active') {
      statusColor = Colors.green;
    } else if (project.status == 'completed') {
      statusColor = Colors.blue;
    } else {
      statusColor = Colors.grey;
    }

    return Card(
      elevation: 4,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: InkWell(
        onTap: () {
          Navigator.push(
            context,
            MaterialPageRoute(
              builder: (context) => ProjectDetailScreen(project: project),
            ),
          );
        },
        borderRadius: BorderRadius.circular(12),
        child: Padding(
          padding: const EdgeInsets.all(16.0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Icon(Icons.folder, color: Colors.indigoAccent),
                  Row(
                    children: [
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                        decoration: BoxDecoration(
                          color: statusColor.withValues(alpha: 0.1),
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(color: statusColor.withValues(alpha: 0.5)),
                        ),
                        child: Text(
                          project.status.toUpperCase(),
                          style: TextStyle(
                            color: statusColor,
                            fontSize: 10,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                      ),
                      if (onDelete != null)
                        IconButton(
                          icon: const Icon(Icons.delete, color: Colors.redAccent, size: 20),
                          onPressed: onDelete,
                        ),
                    ],
                  ),
                ],
              ),
              const Spacer(),
              Text(
                project.name,
                style: const TextStyle(
                  fontWeight: FontWeight.bold,
                  fontSize: 18,
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              const SizedBox(height: 4),
              Text(
                'ID: ${project.id}',
                style: const TextStyle(color: Colors.grey, fontSize: 10, fontFamily: 'monospace'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
