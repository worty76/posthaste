//! Filtering and row ordering for the shared project picker.

use posthaste_model::Project;

/// Longest project name allowed, matching core's `clean_category`.
const MAX_NAME_CHARS: usize = 40;

/// Why the picker was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerMode {
    /// Set or clear a note's project (`P` in inbox/Today, or Details chip).
    SetNote,
    /// Jump to an active project (`#` in the main window).
    Jump,
}

/// One selectable row in the project picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerRow {
    NoProject,
    Project {
        name: String,
        color: Option<String>,
        count: u32,
    },
    Create(String),
    Unarchive(String),
}

/// Builds the picker rows for `projects` and the user's `typed` text.
pub fn rows(projects: &[Project], typed: &str, mode: PickerMode) -> Vec<PickerRow> {
    let cleaned = typed.trim();
    let cleaned = cleaned.strip_prefix('#').unwrap_or(cleaned).trim();
    let needle = cleaned.to_lowercase();

    let mut out = Vec::new();
    if mode == PickerMode::SetNote && cleaned.is_empty() {
        out.push(PickerRow::NoProject);
    }

    let active = projects.iter().filter(|p| !p.archived);
    if needle.is_empty() {
        for project in active {
            out.push(PickerRow::Project {
                name: project.name.clone(),
                color: project.color.clone(),
                count: project.total,
            });
        }
    } else {
        let mut prefix = Vec::new();
        let mut substring = Vec::new();
        for project in active {
            let lower = project.name.to_lowercase();
            if lower.starts_with(&needle) {
                prefix.push(project);
            } else if lower.contains(&needle) {
                substring.push(project);
            }
        }
        for project in prefix.into_iter().chain(substring) {
            out.push(PickerRow::Project {
                name: project.name.clone(),
                color: project.color.clone(),
                count: project.total,
            });
        }
    }

    if mode == PickerMode::SetNote
        && !cleaned.is_empty()
        && cleaned.chars().count() <= MAX_NAME_CHARS
    {
        let exact_active = projects.iter().any(|p| !p.archived && p.name == cleaned);
        if !exact_active {
            if let Some(archived) = projects.iter().find(|p| p.archived && p.name == cleaned) {
                out.push(PickerRow::Unarchive(archived.name.clone()));
            } else {
                out.push(PickerRow::Create(cleaned.to_owned()));
            }
        }
    }

    out
}

/// Decision when renaming a project from `from` to `to`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameDecision {
    /// Blank, invalid, or unchanged.
    NoOp,
    /// Target name does not exist yet: rename directly.
    Rename(String),
    /// Target name already exists: ask before merging `count` notes into `target`.
    ConfirmMerge { target: String, count: u32 },
}

/// Decides whether renaming `from` to `to` needs a merge confirmation.
pub fn rename_decision(from: &str, to: &str, projects: &[Project]) -> RenameDecision {
    let cleaned = to.trim();
    let cleaned = cleaned.strip_prefix('#').unwrap_or(cleaned).trim();
    if cleaned.is_empty() || cleaned.chars().count() > MAX_NAME_CHARS || cleaned == from {
        return RenameDecision::NoOp;
    }
    if projects.iter().any(|p| p.name == cleaned) {
        let count = projects
            .iter()
            .find(|p| p.name == from)
            .map_or(0, |p| p.total);
        RenameDecision::ConfirmMerge {
            target: cleaned.to_owned(),
            count,
        }
    } else {
        RenameDecision::Rename(cleaned.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str, archived: bool, total: u32) -> Project {
        Project {
            name: name.into(),
            color: None,
            archived,
            open: total,
            total,
            last_at: 100,
        }
    }

    fn row(name: &str, count: u32) -> PickerRow {
        PickerRow::Project {
            name: name.into(),
            color: None,
            count,
        }
    }

    #[test]
    fn set_note_mode_lists_no_project_first() {
        let list = vec![project("work", false, 2), project("home", false, 1)];
        let got = rows(&list, "", PickerMode::SetNote);
        assert_eq!(
            got,
            vec![PickerRow::NoProject, row("work", 2), row("home", 1)]
        );
    }

    #[test]
    fn typing_a_new_name_offers_create() {
        let list = vec![project("work", false, 2)];
        let got = rows(&list, "taxes", PickerMode::SetNote);
        assert_eq!(got, vec![PickerRow::Create("taxes".into())]);
    }

    #[test]
    fn an_exact_match_hides_create() {
        let list = vec![project("work", false, 2), project("workshop", false, 1)];
        let got = rows(&list, "work", PickerMode::SetNote);
        assert_eq!(got, vec![row("work", 2), row("workshop", 1)]);
    }

    #[test]
    fn an_archived_exact_match_offers_unarchive_not_create() {
        let list = vec![project("taxes", true, 3), project("work", false, 1)];
        let got = rows(&list, "taxes", PickerMode::SetNote);
        assert_eq!(got, vec![PickerRow::Unarchive("taxes".into())]);
    }

    #[test]
    fn jump_mode_lists_only_active_projects() {
        let list = vec![
            project("work", false, 2),
            project("taxes", true, 3),
            project("home", false, 1),
        ];
        let got = rows(&list, "", PickerMode::Jump);
        assert_eq!(got, vec![row("work", 2), row("home", 1)]);
        assert_eq!(rows(&list, "newname", PickerMode::Jump), vec![]);
    }

    #[test]
    fn prefix_matches_rank_above_substring_matches() {
        let list = vec![
            project("homework", false, 1),
            project("work", false, 2),
            project("rework", false, 3),
            project("workshop", false, 4),
        ];
        let got = rows(&list, "Work", PickerMode::Jump);
        assert_eq!(
            got,
            vec![
                row("work", 2),
                row("workshop", 4),
                row("homework", 1),
                row("rework", 3),
            ]
        );
    }

    #[test]
    fn rename_onto_an_existing_name_asks_before_merging() {
        let list = vec![project("work", false, 3), project("taxes", false, 2)];
        assert_eq!(
            rename_decision("work", "taxes", &list),
            RenameDecision::ConfirmMerge {
                target: "taxes".into(),
                count: 3,
            }
        );
        assert_eq!(
            rename_decision("work", "office", &list),
            RenameDecision::Rename("office".into())
        );
        assert_eq!(rename_decision("work", "work", &list), RenameDecision::NoOp);
        assert_eq!(rename_decision("work", "   ", &list), RenameDecision::NoOp);
    }
}
