# Gestalt CLI

[![Git-Core Protocol](https://img.shields.io/badge/Git--Core%20Protocol-v3.5-blueviolet)](AGENTS.md)

**Gestalt** is a context-aware AI assistant for your terminal. It intelligently gathers project context (files, structure, configs) to give LLMs the full picture of your work.

## 🚀 Features

- **🧠 Context Engine**: Automatically detects project type (Rust, Flutter, Node, etc.) and gathers relevant context (directory tree, markdown summaries) to prepend to your prompt.
- **⚙️ Unified Config**: Centralized configuration via `gestalt.toml` or environment variables.
- **🤖 Multi-Model Support**: Works with Gemini, OpenAI, Qwen, and Ollama.
- **⚖️ Consensus Mode** (Optional): Can query multiple models and synthesize the best answer.
- **🔌 MCP Support**: Connects to Model Context Protocol servers.

## 📦 Installation

```bash
cargo install --path gestalt_cli
```

## 🛠️ Configuration

Initialize a default configuration file:

```bash
gestalt config init
# Created config at:
# Linux: ~/.config/gestalt/gestalt.toml
# Windows: %APPDATA%\gestalt\gestalt.toml
# macOS: ~/Library/Application Support/gestalt/gestalt.toml
```

## 🤖 Usage

### Standard (Context-Aware)
The default mode analyzes your current directory and sends relevant context to the model.

```bash
gestalt --prompt "How do I add a new endpoint to this API?"
```

### Consensus Mode
Query multiple models and get a synthesized answer.

```bash
gestalt --consensus --prompt "What are the security risks of this architecture?"
```

### Manage Config
```bash
gestalt config show
```

## 🏗️ Architecture
See [.gitcore/ARCHITECTURE.md](.gitcore/ARCHITECTURE.md) for detailed system design.

