import 'package:flutter/material.dart';
import 'package:flutter_animate/flutter_animate.dart';
import 'package:glass_kit/glass_kit.dart';
import 'dashboard_screen.dart';
import 'project_list_screen.dart';
import 'settings_screen.dart';
import 'terminal_screen.dart';

class MainLayout extends StatefulWidget {
  const MainLayout({super.key});

  @override
  State<MainLayout> createState() => _MainLayoutState();
}

class _MainLayoutState extends State<MainLayout> {
  int _selectedIndex = 0;

  final List<Widget> _screens = [
    const DashboardScreen(),
    const ProjectListScreen(),
    const Center(child: Text("Agent Forge", style: TextStyle(color: Colors.white, fontSize: 24))),
    const SettingsScreen(),
    const TerminalScreen(),
  ];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      extendBodyBehindAppBar: true,
      body: Stack(
        children: [
          // Background Gradient
          Container(
            decoration: const BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [Color(0xFF0F0C29), Color(0xFF302B63), Color(0xFF24243E)],
              ),
            ),
          ),
          Row(
            children: [
              // Glass Sidebar
              GlassContainer.clearGlass(
                width: 260,
                height: double.infinity,
                borderWidth: 0,
                blur: 20,
                color: Colors.white.withOpacity(0.05),
                child: Column(
                  children: [
                    const SizedBox(height: 60),
                    const Icon(Icons.auto_awesome_outlined, size: 48, color: Colors.purpleAccent)
                        .animate()
                        .shimmer(duration: 2.seconds),
                    const SizedBox(height: 16),
                    Text(
                      "NEURAL LINK",
                      style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                            color: Colors.white,
                            fontWeight: FontWeight.bold,
                            letterSpacing: 4,
                          ),
                    ),
                    const SizedBox(height: 60),
                    _navItem(Icons.grid_view_rounded, "Dashboard", 0),
                    _navItem(Icons.code_rounded, "Repositories", 1),
                    _navItem(Icons.psychology_rounded, "Agent Forge", 2),
                    _navItem(Icons.tune_rounded, "Settings", 3),
                    _navItem(Icons.terminal_rounded, "Terminal", 4),
                  ],
                ),
              ),
              // Main Content Area
              Expanded(
                child: _screens[_selectedIndex]
                    .animate(key: ValueKey(_selectedIndex))
                    .fadeIn(duration: 400.ms)
                    .slideX(begin: 0.1, end: 0),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _navItem(IconData icon, String label, int index) {
    final isSelected = _selectedIndex == index;
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: InkWell(
        onTap: () => setState(() => _selectedIndex = index),
        child: AnimatedContainer(
          duration: 300.ms,
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          decoration: BoxDecoration(
            color: isSelected ? Colors.purpleAccent.withOpacity(0.2) : Colors.transparent,
            borderRadius: BorderRadius.circular(12),
            border: Border.all(
              color: isSelected ? Colors.purpleAccent.withOpacity(0.5) : Colors.transparent,
            ),
          ),
          child: Row(
            children: [
              Icon(icon, color: isSelected ? Colors.purpleAccent : Colors.white60),
              const SizedBox(width: 16),
              Text(
                label,
                style: TextStyle(
                  color: isSelected ? Colors.white : Colors.white60,
                  fontWeight: isSelected ? FontWeight.bold : FontWeight.normal,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
