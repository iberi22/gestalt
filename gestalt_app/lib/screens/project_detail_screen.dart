import 'package:flutter/material.dart';
import '../models/project.dart';
import '../models/task.dart'; // We need a Task model
import '../models/agent.dart';
import '../services/api_service.dart';

class ProjectDetailScreen extends StatefulWidget {
  final Project project;

  const ProjectDetailScreen({super.key, required this.project});

  @override
  State<ProjectDetailScreen> createState() => _ProjectDetailScreenState();
}

class _ProjectDetailScreenState extends State<ProjectDetailScreen> {
  final ApiService _api = ApiService();
  List<Task> _tasks = [];
  List<Agent> _agents = [];
  bool _isLoading = true;

  @override
  void initState() {
    super.initState();
    _fetchTasks();
    _fetchAgents();
  }

  Future<void> _fetchAgents() async {
    final agents = await _api.getAgents();
    if (mounted) {
      setState(() {
        _agents = agents;
      });
    }
  }

  Future<void> _fetchTasks() async {
    // Ideally fetch tasks by project ID.
    // ApiService currently gets ALL tasks, we can filter client-side for now or implement filter in backend.
    // Let's assume client-side filtering for MVP.
    final allTasks = await _api.getTasks();
    if (mounted) {
      setState(() {
        _tasks = allTasks.where((t) => t.projectId == widget.project.id).toList();
        _isLoading = false;
      });
    }
  }

  Future<void> _createTask() async {
    final TextEditingController controller = TextEditingController();
    String? selectedAgentId;

    await showDialog(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          backgroundColor: const Color(0xFF1E1E1E),
          title: const Text('New Task', style: TextStyle(color: Colors.white)),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                controller: controller,
                style: const TextStyle(color: Colors.white),
                decoration: const InputDecoration(
                  hintText: "Task Description",
                  hintStyle: TextStyle(color: Colors.white24),
                ),
              ),
              const SizedBox(height: 16),
              DropdownButtonFormField<String>(
                dropdownColor: const Color(0xFF2E2E2E),
                value: selectedAgentId,
                decoration: const InputDecoration(
                  labelText: "Assign Agent",
                  labelStyle: TextStyle(color: Colors.white70),
                ),
                style: const TextStyle(color: Colors.white),
                items: _agents.map((agent) {
                  return DropdownMenuItem(
                    value: agent.id,
                    child: Text(agent.name),
                  );
                }).toList(),
                onChanged: (value) {
                  setDialogState(() {
                    selectedAgentId = value;
                  });
                },
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Cancel', style: TextStyle(color: Colors.white54)),
            ),
            ElevatedButton(
              onPressed: () async {
                if (controller.text.isNotEmpty) {
                  final success = await _api.createTask(
                    widget.project.name,
                    controller.text,
                    agentId: selectedAgentId,
                  );
                  if (success) {
                    if (mounted) Navigator.pop(context);
                    _fetchTasks();
                  }
                }
              },
              style: ElevatedButton.styleFrom(backgroundColor: Colors.blueAccent),
              child: const Text('Add', style: TextStyle(color: Colors.white)),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _deleteTask(String id) async {
    final success = await _api.deleteTask(id);
    if (success) {
      _fetchTasks();
    }
  }

  Future<void> _runTask(String id) async {
    ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text("Task started...")));
    final success = await _api.runTask(id);
    if (success) {
       ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text("Task completed!")));
       _fetchTasks();
    } else {
       ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text("Task failed to start.")));
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: const Color(0xFF121212),
      appBar: AppBar(
        title: Text(widget.project.name),
        backgroundColor: const Color(0xFF1E1E1E),
        foregroundColor: Colors.white,
      ),
      body: Padding(
        padding: const EdgeInsets.all(24.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
             _buildHeader(),
             const SizedBox(height: 24),
             const Text("Tasks", style: TextStyle(color: Colors.white, fontSize: 20, fontWeight: FontWeight.bold)),
             const SizedBox(height: 12),
             Expanded(
               child: _isLoading
                 ? const Center(child: CircularProgressIndicator())
                 : _tasks.isEmpty
                   ? const Center(child: Text("No tasks in this project.", style: TextStyle(color: Colors.white30)))
                   : ListView.separated(
                       itemCount: _tasks.length,
                       separatorBuilder: (_,__) => const Divider(color: Colors.white10),
                       itemBuilder: (context, index) {
                         final task = _tasks[index];
                         return ListTile(
                           title: Text(task.description ?? "Unnamed Task", style: const TextStyle(color: Colors.white)),
                           subtitle: Text(task.status, style: TextStyle(color: Colors.blueAccent)),
                           leading: Icon(Icons.check_circle_outline, color: task.status == 'completed' ? Colors.green : Colors.grey),
                           trailing: Row(
                             mainAxisSize: MainAxisSize.min,
                             children: [
                               IconButton(
                                 icon: const Icon(Icons.play_arrow, color: Colors.greenAccent),
                                 onPressed: () => _runTask(task.id),
                               ),
                               IconButton(
                                 icon: const Icon(Icons.delete, color: Colors.redAccent),
                                 onPressed: () => _deleteTask(task.id),
                               ),
                             ],
                           ),
                         );
                       },
                     ),
             ),
          ],
        ),
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: _createTask,
        backgroundColor: Colors.blueAccent,
        child: const Icon(Icons.add),
      ),
    );
  }

  Widget _buildHeader() {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
         color: const Color(0xFF1E1E1E),
         borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text("Status: ${widget.project.status}", style: const TextStyle(color: Colors.white70)),
          const SizedBox(height: 8),
          Text("ID: ${widget.project.id}", style: const TextStyle(color: Colors.white30, fontFamily: 'monospace')),
        ],
      ),
    );
  }
}
