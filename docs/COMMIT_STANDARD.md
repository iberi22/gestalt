# 📝 Commit Standard

We follow **Conventional Commits** with extended context.

## Format
```text
<type>(<scope>): <description> #<issue-number>

[optional body]

[optional footer]
```

## Types
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Formatting, missing semi colons, etc; no code change
- `refactor`: Refactoring production code
- `test`: Adding tests, refactoring test; no production code change
- `chore`: Updating build tasks, package manager configs, etc

## Scopes
- `cli`: gestalt_cli
- `core`: gestalt_core
- `swarm`: gestalt_swarm
- `timeline`: gestalt_timeline
- `synapse`: synapse-agentic
- `docs`: documentation
- `build`: build system / CI

## Example
```text
feat(cli): add --context flag to main command #42

Implemented the context engine integration in the main CLI loop.
Now reads from gestalt_core::context.

Closes #42
```
