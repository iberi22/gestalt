use super::ProjectType;
use std::path::Path;

pub fn detect_project_type(path: &Path) -> ProjectType {
    if path.join("Cargo.toml").exists() {
        return ProjectType::Rust;
    }
    if path.join("pubspec.yaml").exists() {
        return ProjectType::Flutter;
    }
    if path.join("package.json").exists() {
        return ProjectType::Node;
    }
    if path.join("requirements.txt").exists() || path.join("pyproject.toml").exists() {
        return ProjectType::Python;
    }
    ProjectType::Unknown
}
