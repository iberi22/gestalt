import argparse
import asyncio
import json
import os
import sys
try:
    from .rag_context import RAGContext
    from .agent_selector import select_agent
    from .orchestration_flow import execute_flow
except ImportError:
    from rag_context import RAGContext
    from agent_selector import select_agent
    from orchestration_flow import execute_flow

def get_parser():
    parser = argparse.ArgumentParser(description="Gestalt Agent Orchestrator")
    parser.add_argument("--task", type=str, help="Single task to execute")
    parser.add_argument("--flow", type=str, help="Path to workflow JSON file")
    parser.add_argument("--agents", type=str, help="Comma-separated list of agents to consider")
    parser.add_argument("--project", type=str, default=".", help="Project path to index")
    return parser

async def run_orchestrator(args, rag=None):
    if not args.task and not args.flow:
        print("Error: Either --task or --flow must be provided.")
        return

    # 1. Initialize RAG Context
    if rag is None:
        print("🔍 Initializing RAG Context...")
        rag = RAGContext()

    # 2. Index Project
    print(f"📁 Indexing project at {args.project}...")
    rag.index_project(args.project)

    if args.task:
        # 3. Retrieve context
        docs = rag.retrieve(args.task)

        # 4. Select Agent
        agent, confidence = select_agent(args.task, docs)
        print(f"🤖 Selected agent: {agent} (confidence: {confidence:.2f})")

        # 5. Execute as a single-step flow
        flow = [{"name": "Single Task", "task": args.task}]
        result = await execute_flow(agent, flow, rag)
        print("\n✨ Execution Result:")
        print(result)
        return result

    elif args.flow:
        if not os.path.exists(args.flow):
            print(f"❌ Flow file not found: {args.flow}")
            return

        with open(args.flow, 'r') as f:
            flow_data = json.load(f)

        task = flow_data.get("description", "Complex Workflow")
        steps = flow_data.get("steps", [])

        # Selection for the whole flow (could also be per step)
        docs = rag.retrieve(task)
        agent, confidence = select_agent(task, docs)
        print(f"🤖 Selected agent for flow: {agent} (confidence: {confidence:.2f})")

        result = await execute_flow(agent, steps, rag)
        print("\n✨ Flow Execution Result:")
        print(result)
        return result

async def main():
    parser = get_parser()
    args = parser.parse_args()
    await run_orchestrator(args)

if __name__ == "__main__":
    # Add parent directory to sys.path to allow relative imports
    sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    asyncio.run(main())
