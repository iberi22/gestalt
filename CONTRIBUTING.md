# Contributing to Gestalt

First off, thank you for considering contributing to Gestalt!

## Getting Started

1. **Fork** the repository on GitHub.
2. **Clone** your fork locally and set the upstream remote:
   ```bash
   git clone https://github.com/<your-username>/gestalt.git
   cd gestalt
   git remote add upstream https://github.com/southwest-ai-labs/gestalt.git
   ```
3. **Create a branch** for your feature or bug fix:
   ```bash
   git checkout -b feature/my-new-feature
   ```

## Making Changes

- Ensure your code follows the existing style and format conventions.
- We use **Conventional Commits**. Please format your commit messages accordingly (e.g., `feat: add new orchestration engine` or `fix: resolve issue with agent vfs`).

## Testing

Before submitting a Pull Request, you must ensure that all tests pass:

```bash
cargo check
cargo test --all-targets
```

## Pull Requests

- Push your branch to your fork.
- Open a Pull Request against the `main` branch of the `southwest-ai-labs/gestalt` repository.
- Ensure the CI workflow passes on your Pull Request.
