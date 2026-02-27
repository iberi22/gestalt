use git2::{Repository, StatusOptions};

/// Returns a summary of the current git status.
pub fn get_git_status() -> String {
    let repo = match Repository::discover(".") {
        Ok(repo) => repo,
        Err(_) => return String::from("Not a git repository"),
    };

    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);

    let statuses = match repo.statuses(Some(&mut opts)) {
        Ok(s) => s,
        Err(e) => return format!("Error getting git status: {}", e),
    };

    let branch = get_current_branch(&repo);

    if statuses.is_empty() {
        return match branch {
            Some(b) => format!("On branch {}\nnothing to commit, working tree clean", b),
            None => String::from("nothing to commit, working tree clean"),
        };
    }

    let mut result = String::new();
    if let Some(b) = branch {
        result.push_str(&format!("On branch {}\n", b));
    }

    for entry in statuses.iter() {
        let status = entry.status();
        let path = entry.path().unwrap_or("unknown");

        let mut index_char = ' ';
        if status.is_index_new() { index_char = 'A'; }
        else if status.is_index_modified() { index_char = 'M'; }
        else if status.is_index_deleted() { index_char = 'D'; }
        else if status.is_index_renamed() { index_char = 'R'; }
        else if status.is_index_typechange() { index_char = 'T'; }

        let mut wt_char = ' ';
        if status.is_wt_new() { wt_char = '?'; }
        else if status.is_wt_modified() { wt_char = 'M'; }
        else if status.is_wt_deleted() { wt_char = 'D'; }
        else if status.is_wt_renamed() { wt_char = 'R'; }
        else if status.is_wt_typechange() { wt_char = 'T'; }

        let status_code = if wt_char == '?' && index_char == ' ' {
            String::from("??")
        } else {
            format!("{}{}", index_char, wt_char)
        };

        result.push_str(&format!("{} {}\n", status_code, path));
    }

    result
}

fn get_current_branch(repo: &Repository) -> Option<String> {
    match repo.head() {
        Ok(head) => {
            if head.is_branch() {
                head.shorthand().map(|s| s.to_string())
            } else {
                head.target().map(|oid| format!("(detached at {})", &oid.to_string()[..7]))
            }
        }
        Err(_) => {
            // If head() fails, we might be on a new branch with no commits.
            // We can try to get the symbolic reference HEAD points to.
            repo.find_reference("HEAD").ok().and_then(|refs| {
                refs.symbolic_target().map(|s| {
                    s.strip_prefix("refs/heads/").unwrap_or(s).to_string()
                })
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_git_status() {
        let status = get_git_status();
        // Since we are running this in a git repo, it should either be clean or have some changes.
        // At the very least, it shouldn't be "Not a git repository".
        assert!(status != "Not a git repository");
        assert!(status.contains("On branch") || status.contains("nothing to commit"));
    }
}
