#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Integration test for swarm_bridge.py
Tests various modes and flags to ensure all features work together.
"""

import json
import os
import subprocess
import sys
import unittest

class TestSwarmBridge(unittest.TestCase):
    def run_bridge(self, args):
        cmd = [sys.executable, "swarm_bridge.py"] + args
        result = subprocess.run(cmd, capture_output=True, text=True)
        return result

    def test_dry_run(self):
        """Test --dry-run for smart selection."""
        result = self.run_bridge(["--goal", "analyze code", "--dry-run"])
        self.assertEqual(result.returncode, 0)
        self.assertIn("Selected agents", result.stdout)
        self.assertIn("code_analyzer", result.stdout)

    def test_agents_override(self):
        """Test manual agent override."""
        result = self.run_bridge(["--goal", "test", "--agents", "git_status", "--quiet"])
        self.assertEqual(result.returncode, 0)
        self.assertIn("Git Status", result.stdout)
        # Should only have one agent
        self.assertEqual(result.stdout.count("Git Status"), 1)

    def test_json_output(self):
        """Test --json output parsing."""
        result = self.run_bridge(["--goal", "git status", "--agents", "git_status", "--json"])
        self.assertEqual(result.returncode, 0)
        data = json.loads(result.stdout)
        self.assertEqual(data["goal"], "git status")
        self.assertEqual(len(data["agents"]), 1)
        self.assertEqual(data["agents"][0]["id"], "git_status")

    def test_watch_mode(self):
        """Test --watch mode."""
        # Use a temp output file
        output_file = "test_swarm_output.json"
        if os.path.exists(output_file):
            os.remove(output_file)

        result = self.run_bridge([
            "--goal", "git status",
            "--agents", "git_status",
            "--watch",
            "--output", output_file
        ])

        self.assertEqual(result.returncode, 0)
        self.assertIn("Streaming mode enabled", result.stdout)
        self.assertIn("All 1 agents finished", result.stdout)

        # Verify output file exists and is valid JSON
        self.assertTrue(os.path.exists(output_file))
        with open(output_file, "r") as f:
            data = json.load(f)
            self.assertEqual(data["completed_count"], 1)

        os.remove(output_file)

    def test_smart_selection_categories(self):
        """Test smart selection for different categories."""
        goals = {
            "security audit": "security_audit",
            "check dependencies": "dep_check",
            "git history": "git_analyzer",
            "list files": "file_scanner"
        }
        for goal, expected_agent in goals.items():
            result = self.run_bridge(["--goal", goal, "--dry-run"])
            self.assertIn(expected_agent, result.stdout, f"Goal '{goal}' should have selected '{expected_agent}'")

if __name__ == "__main__":
    unittest.main()
