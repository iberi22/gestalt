import asyncio
from typing import List, Dict, Any
try:
    from .rag_context import RAGContext
except ImportError:
    from rag_context import RAGContext

async def execute_step(agent: str, step: Dict[str, Any], context: RAGContext) -> str:
    """
    Simulates the execution of a step by an agent.
    In a real system, this would call the actual agent API.
    """
    print(f"[{agent}] Executing step: {step.get('name', 'unnamed')}")
    task = step.get("task", "")

    # Retrieve context for the step
    relevant_docs = context.retrieve(task, k=3)
    context_str = "\n".join([doc.page_content for doc in relevant_docs])

    # Simulate execution result
    result = f"Result of task '{task}' executed by {agent}. Context used: {len(relevant_docs)} docs."

    # Propagate result back to context
    context.add_result(result, metadata={"agent": agent, "step": step.get("name")})

    return result

async def execute_flow(agent: str, steps: List[Dict[str, Any]], context: RAGContext) -> str:
    """
    Executes a sequence of steps using the selected agent.
    """
    final_result = ""
    for step in steps:
        result = await execute_step(agent, step, context)
        final_result += result + "\n"

    return final_result
