import pytest
import os
import shutil
import asyncio
from scripts.orchestration.rag_context import RAGContext
from scripts.orchestration.agent_selector import select_agent, AGENT_CAPABILITIES
from scripts.orchestration.orchestration_flow import execute_flow, execute_step

@pytest.fixture
def temp_rag():
    rag = RAGContext(persist_directory=None) # Use in-memory
    return rag

def test_rag_indexing(temp_rag):
    # Create a dummy file to index
    os.makedirs("test_project", exist_ok=True)
    with open("test_project/test.md", "w") as f:
        f.write("# Test Project\nThis is a test document about Rust and Python.")

    temp_rag.index_project("test_project")
    results = temp_rag.retrieve("Rust")

    assert len(results) > 0
    assert "test_project/test.md" in results[0].metadata["source"]

    shutil.rmtree("test_project")

def test_agent_selector():
    # Test bugfix -> codex
    agent, conf = select_agent("Fix a bug in the code", [])
    assert agent == "codex"
    assert conf > 0

    # Test architecture -> gestalt
    agent, conf = select_agent("Design a new Rust architecture", [])
    assert agent == "gestalt"

    # Test explain -> gemini
    agent, conf = select_agent("Explain how this works", [])
    assert agent == "gemini"

@pytest.mark.asyncio
async def test_orchestration_flow(temp_rag):
    steps = [
        {"name": "Step 1", "task": "Analyze code"},
        {"name": "Step 2", "task": "Implement fix"}
    ]

    result = await execute_flow("codex", steps, temp_rag)
    assert "Result of task 'Analyze code'" in result
    assert "Result of task 'Implement fix'" in result

    # Check if context was propagated
    history = temp_rag.retrieve("Analyze code")
    assert len(history) > 0
    assert "codex" in history[0].page_content or "codex" in history[0].metadata.get("agent", "")

@pytest.mark.asyncio
async def test_orchestrate_cli(temp_rag):
    from scripts.orchestration.orchestrate import get_parser, run_orchestrator

    parser = get_parser()
    args = parser.parse_args(["--task", "Fix a Rust bug", "--project", "."])

    result = await run_orchestrator(args, rag=temp_rag)
    assert "Result of task 'Fix a Rust bug'" in result

    # Test with flow
    import json
    flow_file = "test_flow.json"
    with open(flow_file, "w") as f:
        json.dump({"description": "Test flow", "steps": [{"name": "Step 1", "task": "task 1"}]}, f)

    args = parser.parse_args(["--flow", flow_file, "--project", "."])
    result = await run_orchestrator(args, rag=temp_rag)
    assert "Result of task 'task 1'" in result
    os.remove(flow_file)

    # Test error cases
    args = parser.parse_args(["--project", "."])
    result = await run_orchestrator(args, rag=temp_rag)
    assert result is None
