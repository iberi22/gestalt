use super::FileContext;
use ignore::WalkBuilder;
use std::fs;
use std::path::Path;

pub fn scan_markdown_files(root: &Path) -> Vec<FileContext> {
    let mut files = Vec::new();

    // Priority 1: Check for Git-Core Protocol files first (.gitcore/ARCHITECTURE.md)
    let gitcore_path = root.join(".gitcore/ARCHITECTURE.md");
    if gitcore_path.exists() {
        if let Ok(content) = fs::read_to_string(&gitcore_path) {
            files.push(FileContext {
                path: ".gitcore/ARCHITECTURE.md".to_string(),
                content, // Load full content for Architecture, it's critical
            });
        }
    }

    let walker = WalkBuilder::new(root)
        .hidden(false) // Allow hidden files like .github
        .git_ignore(true)
        .build();

    for result in walker {
        match result {
            Ok(entry) => {
                let path = entry.path();
                // Avoid duplicating if we already added it manually
                if path.ends_with(".gitcore/ARCHITECTURE.md") {
                    continue;
                }

                if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                    // Skip target/ and node_modules/ just in case .gitignore misses them
                    if path.to_string_lossy().contains("target")
                        || path.to_string_lossy().contains("node_modules")
                    {
                        continue;
                    }

                    if let Ok(content) = fs::read_to_string(path) {
                        let lines: Vec<&str> = content.lines().collect();
                        let truncated_content = if lines.len() > 100 {
                            let mut c = lines.into_iter().take(100).collect::<Vec<_>>().join("\n");
                            c.push_str("\n... (truncated after 100 lines)");
                            c
                        } else {
                            content
                        };

                        files.push(FileContext {
                            path: path.display().to_string(),
                            content: truncated_content,
                        });
                    }
                }
            }
            Err(err) => eprintln!("Error walking directory: {}", err),
        }
    }
    files
}

pub fn generate_directory_tree(root: &Path, max_depth: usize) -> String {
    let mut tree = String::new();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .max_depth(Some(max_depth))
        .build();

    for entry in walker.flatten() {
        let depth = entry.depth();
        if depth == 0 {
            continue;
        }

        let indent = "  ".repeat(depth - 1);
        let file_name = entry.file_name().to_string_lossy();
        let marker = if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            "/"
        } else {
            ""
        };

        tree.push_str(&format!("{}├── {}{}\n", indent, file_name, marker));
    }
    tree
}
