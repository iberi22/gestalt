import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../models/project.dart';
import '../services/api_service.dart';
import '../widgets/project_card.dart';

class ProjectListScreen extends StatefulWidget {
  const ProjectListScreen({super.key});

  @override
  State<ProjectListScreen> createState() => _ProjectListScreenState();
}

class _ProjectListScreenState extends State<ProjectListScreen> {
  List<Project> _projects = [];
  bool _isLoading = true;

  @override
  void initState() {
    super.initState();
    _fetchProjects();
  }

  Future<void> _fetchProjects() async {
    final api = context.read<ApiService>();
    final projects = await api.getProjects();
    if (mounted) {
      setState(() {
        _projects = projects;
        _isLoading = false;
      });
    }
  }

  Future<void> _createProject() async {
    final TextEditingController controller = TextEditingController();
    await showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('New Project'),
        content: TextField(
          controller: controller,
          decoration: const InputDecoration(hintText: "Project Name"),
        ),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancel')),
          ElevatedButton(
            onPressed: () async {
              if (controller.text.isNotEmpty) {
                 final api = context.read<ApiService>();
                 final success = await api.createProject(controller.text);
                 if (success) {
                    if (mounted) Navigator.pop(context);
                    _fetchProjects();
                 }
              }
            },
            child: const Text('Create'),
          ),
        ],
      ),
    );
  }

  Future<void> _deleteProject(String id) async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Delete Project?'),
        content: const Text('This will delete the project and all its tasks.'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancel')),
          ElevatedButton(
             style: ElevatedButton.styleFrom(backgroundColor: Colors.red),
             onPressed: () => Navigator.pop(context, true),
             child: const Text('Delete', style: TextStyle(color: Colors.white)),
          ),
        ],
      ),
    );

    if (confirm == true) {
      final api = context.read<ApiService>();
      final success = await api.deleteProject(id);
      if (success) {
        _fetchProjects();
      }
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
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text("All Projects", style: TextStyle(color: Colors.white, fontSize: 28, fontWeight: FontWeight.bold)),
              ElevatedButton.icon(
                onPressed: _createProject,
                icon: const Icon(Icons.add),
                label: const Text("New Project"),
                style: ElevatedButton.styleFrom(
                  backgroundColor: Colors.indigoAccent,
                  foregroundColor: Colors.white,
                ),
              ),
            ],
          ),
          const SizedBox(height: 24),
          Expanded(
            child: _isLoading
              ? const Center(child: CircularProgressIndicator())
              : _projects.isEmpty
                ? const Center(child: Text("No projects found.", style: TextStyle(color: Colors.white30)))
                : GridView.builder(
                    gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                      crossAxisCount: 3,
                      childAspectRatio: 1.5,
                      crossAxisSpacing: 16,
                      mainAxisSpacing: 16,
                    ),
                    itemCount: _projects.length,
                    itemBuilder: (context, index) {
                      return ProjectCard(
                        project: _projects[index],
                        onDelete: () => _deleteProject(_projects[index].id),
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }
}
