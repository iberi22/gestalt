from typing import List, Tuple
from langchain_core.documents import Document

AGENT_CAPABILITIES = {
    "codex": {"languages": ["*"], "best_for": ["edit", "refactor", "bugfix"]},
    "gemini": {"languages": ["*"], "best_for": ["analyze", "explain", "generate"]},
    "qwen": {"languages": ["*"], "best_for": ["translate", "multilingual"]},
    "gestalt": {"languages": ["rust", "python"], "best_for": ["architecture", "design"]},
}

def select_agent(task: str, context: List[Document]) -> Tuple[str, float]:
    """
    Selects the best agent for the task based on capabilities and context.
    Returns a tuple of (agent_name, confidence).
    """
    task_lower = task.lower()
    context_text = "\n".join([doc.page_content.lower() for doc in context])
    scores = {agent: 0.0 for agent in AGENT_CAPABILITIES}

    # Heuristic-based selection using task and retrieved context
    for agent, caps in AGENT_CAPABILITIES.items():
        # Check 'best_for' keywords in task
        for keyword in caps["best_for"]:
            if keyword in task_lower:
                scores[agent] += 0.4
            if keyword in context_text:
                scores[agent] += 0.2

        # Language detection (simple)
        languages = caps["languages"]
        if "rust" in languages or "*" in languages:
            if "rust" in task_lower: scores[agent] += 0.3
            if "rust" in context_text: scores[agent] += 0.1
        if "python" in languages or "*" in languages:
            if "python" in task_lower: scores[agent] += 0.3
            if "python" in context_text: scores[agent] += 0.1

        if "*" in languages:
            scores[agent] += 0.1

    # In a real implementation, we'd use an LLM to analyze the task and context
    # and provide a selection with reasoning.

    best_agent = max(scores, key=scores.get)
    confidence = min(scores[best_agent], 1.0)

    # Ensure a minimum confidence
    if confidence == 0:
        best_agent = "gemini" # Default
        confidence = 0.5

    return best_agent, confidence
