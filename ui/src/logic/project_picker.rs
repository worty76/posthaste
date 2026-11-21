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
    Project { name: String, count: u32 },
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

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str, archived: bool, total: u32) -> Project {
        Project {
            name: name.into(),
            archived,
            open: total,
            total,
            last_at: 100,
        }
    }

    #[test]
    fn set_note_mode_lists_no_project_first() {
        let list = vec![project("work", false, 2), project("home", false, 1)];
        let got = rows(&list, "", PickerMode::SetNote);
        assert_eq!(
            got,
            vec![
                PickerRow::NoProject,
                PickerRow::Project {
                    name: "work".into(),
                    count: 2,
                },
                PickerRow::Project {
                    name: "home".into(),
                    count: 1,
                },
            ]
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
        assert_eq!(
            got,
            vec![
                PickerRow::Project {
                    name: "work".into(),
                    count: 2,
                },
                PickerRow::Project {
                    name: "workshop".into(),
                    count: 1,
                },
            ]
        );
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
        assert_eq!(
            got,
            vec![
                PickerRow::Project {
                    name: "work".into(),
                    count: 2,
                },
                PickerRow::Project {
                    name: "home".into(),
                    count: 1,
                },
            ]
        );
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
                PickerRow::Project {
                    name: "work".into(),
                    count: 2,
                },
                PickerRow::Project {
                    name: "workshop".into(),
                    count: 4,
                },
                PickerRow::Project {
                    name: "homework".into(),
                    count: 1,
                },
                PickerRow::Project {
                    name: "rework".into(),
                    count: 3,
                },
            ]
        );
    }
}
