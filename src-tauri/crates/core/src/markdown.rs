//! The Markdown export: one file that reads well anywhere and still imports
//! back without loss.
//!
//! Each note is a task-list item at the start of a line. Everything that
//! belongs to it is indented by two spaces: further lines of the note, a
//! hidden `<!-- posthaste {...} -->` comment with its id and exact times, a
//! visible line with its category and dates, then its description. The
//! comment is what makes the round trip exact; without it (a list typed by
//! hand) the checkbox and the section heading decide the status.

use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset};

use crate::error::{CoreError, Result};
use crate::transfer::{ExportNote, EXPORT_VERSION};

const FILE_MARKER: &str = "<!-- posthaste-export version ";
const NOTE_MARKER: &str = "<!-- posthaste ";
const INDENT: &str = "  ";

/// What the hidden comment carries: everything but the text, which is
/// written out visibly.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Meta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    archived_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    later_at: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    ToDo,
    Later,
    Done,
    Archived,
}

impl Section {
    const ALL: [Section; 4] = [
        Section::ToDo,
        Section::Later,
        Section::Done,
        Section::Archived,
    ];

    fn heading(self) -> &'static str {
        match self {
            Section::ToDo => "To do",
            Section::Later => "Later",
            Section::Done => "Done",
            Section::Archived => "Archived",
        }
    }

    fn of(n: &ExportNote) -> Section {
        match n.status.as_deref() {
            Some("completed") => Section::Done,
            Some("archived") => Section::Archived,
            _ if n.later_at.is_some() => Section::Later,
            _ => Section::ToDo,
        }
    }
}

/// Writes the notes as Markdown. Dates shown to the reader use
/// `utc_offset_minutes` (the user's time zone); the hidden comments keep
/// exact UTC times.
pub(crate) fn write(notes: &[ExportNote], exported_at: &str, utc_offset_minutes: i32) -> String {
    let offset = UtcOffset::from_whole_seconds(utc_offset_minutes.clamp(-1080, 1080) * 60)
        .unwrap_or(UtcOffset::UTC);
    let shown = |rfc: &str| readable(rfc, offset);

    let mut out = String::from("# Posthaste notes\n\n");
    let count = match notes.len() {
        1 => "1 note".to_string(),
        n => format!("{n} notes"),
    };
    out.push_str(&format!("Exported {} · {count}\n", shown(exported_at)));
    out.push_str(&format!("{FILE_MARKER}{EXPORT_VERSION} -->\n"));
    if notes.is_empty() {
        out.push_str("\nNo notes.\n");
    }

    for section in Section::ALL {
        let mut items = notes
            .iter()
            .filter(|n| Section::of(n) == section)
            .peekable();
        if items.peek().is_none() {
            continue;
        }
        out.push_str(&format!("\n## {}\n", section.heading()));
        for n in items {
            out.push('\n');
            write_note(&mut out, n, section, &shown);
        }
    }
    out
}

fn write_note(out: &mut String, n: &ExportNote, section: Section, shown: &dyn Fn(&str) -> String) {
    let box_ = if section == Section::Done {
        "[x]"
    } else {
        "[ ]"
    };
    let mut lines = n.content.lines();
    out.push_str(&format!("- {box_} {}\n", lines.next().unwrap_or_default()));
    for line in lines {
        push_indented(out, line);
    }

    let meta = Meta {
        id: n.id.clone(),
        status: n.status.clone(),
        category: n.category.clone(),
        source: n.source.clone(),
        created_at: n.created_at.clone(),
        updated_at: n.updated_at.clone(),
        completed_at: n.completed_at.clone(),
        archived_at: n.archived_at.clone(),
        later_at: n.later_at.clone(),
    };
    // "--" would end the comment early; "-" is the same character in JSON.
    let json = serde_json::to_string(&meta)
        .unwrap_or_else(|_| "{}".into())
        .replace("--", "-\\u002d");
    out.push_str(&format!("{INDENT}{NOTE_MARKER}{json} -->\n"));

    let mut about = Vec::new();
    if let Some(c) = &n.category {
        about.push(format!("`{c}`"));
    }
    if let Some(t) = &n.created_at {
        about.push(format!("Added {}", shown(t)));
    }
    if let Some(t) = n.later_at.as_deref().filter(|_| section == Section::Later) {
        about.push(format!("in Later since {}", shown(t)));
    }
    if let Some(t) = n
        .completed_at
        .as_deref()
        .filter(|_| section == Section::Done)
    {
        about.push(format!("done {}", shown(t)));
    }
    if let Some(t) = n
        .archived_at
        .as_deref()
        .filter(|_| section == Section::Archived)
    {
        about.push(format!("archived {}", shown(t)));
    }
    if let Some(s) = &n.source {
        about.push(format!("from {s}"));
    }
    if about.is_empty() {
        about.push("Note".into());
    }
    out.push_str(&format!("{INDENT}{}\n", about.join(" · ")));

    let details = n.details.trim_matches('\n');
    if !details.trim().is_empty() {
        out.push('\n');
        for line in details.split('\n') {
            push_indented(out, line);
        }
    }
}

fn push_indented(out: &mut String, line: &str) {
    if !line.is_empty() {
        out.push_str(INDENT);
        out.push_str(line);
    }
    out.push('\n');
}

/// "14 Nov 2026 09:12" in the given offset; the input as-is if unreadable.
fn readable(rfc: &str, offset: UtcOffset) -> String {
    let Ok(t) = OffsetDateTime::parse(rfc, &time::format_description::well_known::Rfc3339) else {
        return rfc.to_string();
    };
    let t = t.to_offset(offset);
    let month = t.month().to_string();
    format!(
        "{} {} {} {:02}:{:02}",
        t.day(),
        &month[..3],
        t.year(),
        t.hour(),
        t.minute()
    )
}

/// Reads notes back from Markdown. Fails only when the file is not a
/// Posthaste export and holds no task-list items at all.
pub(crate) fn read(text: &str) -> Result<Vec<ExportNote>> {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut ours = false;
    let mut heading = String::new();
    let mut notes = Vec::new();
    let mut lines = text.split('\n').peekable();

    while let Some(line) = lines.next() {
        if let Some(rest) = line.trim().strip_prefix(FILE_MARKER) {
            ours = true;
            let version: u32 = rest.trim_end_matches("-->").trim().parse().unwrap_or(0);
            if version > EXPORT_VERSION {
                return Err(CoreError::InvalidImport(format!(
                    "export version {version} is newer than this version of Posthaste supports"
                )));
            }
            continue;
        }
        if let Some(h) = line.strip_prefix("## ") {
            heading = h.trim().to_lowercase();
            continue;
        }
        let Some((checked, first)) = task_item(line) else {
            continue;
        };
        let mut body = Vec::new();
        while let Some(next) = lines.peek() {
            if !next.trim().is_empty() && !next.starts_with(INDENT) {
                break;
            }
            let next = lines.next().unwrap_or_default();
            body.push(next.strip_prefix(INDENT).unwrap_or(""));
        }
        while body.last().is_some_and(|l| l.trim().is_empty()) {
            body.pop();
        }
        notes.push(note_from(checked, first, &body, &heading));
    }

    if notes.is_empty() && !ours {
        return Err(CoreError::InvalidImport(
            "no notes found; expected a Posthaste export or a Markdown task list".into(),
        ));
    }
    Ok(notes)
}

/// `- [ ] text` or `* [x] text` at the start of a line.
fn task_item(line: &str) -> Option<(bool, &str)> {
    let rest = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))?;
    let checked = match rest.get(..3)? {
        "[ ]" => false,
        "[x]" | "[X]" => true,
        _ => return None,
    };
    let text = &rest[3..];
    let text = text.strip_prefix(' ').unwrap_or(text);
    Some((checked, text))
}

fn note_from(checked: bool, first: &str, body: &[&str], heading: &str) -> ExportNote {
    let marker = body
        .iter()
        .position(|l| l.starts_with(NOTE_MARKER) && parse_meta(l).is_some());
    let (content_lines, meta, rest): (&[&str], Meta, &[&str]) = match marker {
        Some(i) => {
            let meta = parse_meta(body[i]).unwrap_or_default();
            // The visible line after the comment is for reading only.
            let skip = if body.get(i + 1).is_some_and(|l| !l.trim().is_empty()) {
                2
            } else {
                1
            };
            (&body[..i], meta, body.get(i + skip..).unwrap_or(&[]))
        }
        None => {
            let end = body
                .iter()
                .position(|l| l.trim().is_empty())
                .unwrap_or(body.len());
            (&body[..end], Meta::default(), &body[end..])
        }
    };

    let mut content = first.to_string();
    for l in content_lines {
        content.push('\n');
        content.push_str(l);
    }
    let details = rest
        .iter()
        .skip_while(|l| l.trim().is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("\n");

    // The checkbox says whether it is done, so ticking one in another app
    // counts. An archived note stays archived; a hand-typed list goes by
    // its heading.
    let status = match meta.status.as_deref() {
        Some("archived") => "archived",
        Some(_) if checked => "completed",
        Some(_) => "inbox",
        None if heading == "archived" => "archived",
        None if checked || heading == "done" || heading == "completed" => "completed",
        None => "inbox",
    };
    let marked_status = meta.status.as_deref();
    ExportNote {
        id: meta.id,
        content,
        status: Some(status.into()),
        category: meta.category,
        source: meta.source,
        created_at: meta.created_at,
        updated_at: meta.updated_at,
        completed_at: meta.completed_at.filter(|_| status == "completed"),
        archived_at: meta.archived_at.filter(|_| status == "archived"),
        later_at: meta
            .later_at
            .filter(|_| status == "inbox" && marked_status != Some("completed")),
        details,
    }
}

fn parse_meta(line: &str) -> Option<Meta> {
    let json = line
        .strip_prefix(NOTE_MARKER)?
        .trim_end()
        .strip_suffix("-->")?;
    serde_json::from_str(json.trim()).ok()
}
