import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../models/project.dart';
import '../models/task.dart';
import '../services/api_service.dart';

class ProjectDetailScreen extends StatefulWidget {
  final Project project;

  const ProjectDetailScreen({super.key, required this.project});

  @override
  State<ProjectDetailScreen> createState() => _ProjectDetailScreenState();
}

class _ProjectDetailScreenState extends State<ProjectDetailScreen> {
  List<Task> _tasks = [];
  bool _isLoading = true;

  @override
  void initState() {
    super.initState();
    _fetchTasks();
  }

  Future<void> _fetchTasks() async {
    final api = context.read<ApiService>();
    final allTasks = await api.getTasks();
    if (mounted) {
      setState(() {
        _tasks = allTasks.where((t) => t.projectId == widget.project.id).toList();
        _isLoading = false;
      });
    }
  }

  Future<void> _createTask() async {
    final TextEditingController controller = TextEditingController();
    await showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('New Task'),
        content: TextField(
          controller: controller,
          decoration: const InputDecoration(hintText: "Task Description"),
        ),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancel')),
          ElevatedButton(
            onPressed: () async {
              if (controller.text.isNotEmpty) {
                 final api = context.read<ApiService>();
                 final success = await api.createTask(widget.project.name, controller.text);
                 if (success) {
                    if (mounted) Navigator.pop(context);
                    _fetchTasks();
                 }
              }
            },
            child: const Text('Add'),
          ),
        ],
      ),
    );
  }

  Future<void> _deleteTask(String id) async {
    final api = context.read<ApiService>();
    final success = await api.deleteTask(id);
    if (success) {
      _fetchTasks();
    }
  }

  Future<void> _runTask(String id) async {
    ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text("Task started...")));
    final api = context.read<ApiService>();
    final success = await api.runTask(id);
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
