//! Turns `locales/<code>/*.json` into Rust tables.
//!
//! English is the source of truth: its keys become the constants in
//! `key`, so a mistyped key is a compile error. Every other language is
//! checked against it here. A key English does not have, or a message whose
//! `{placeholders}` differ from the English ones, stops the build. A key a
//! language has not translated yet is allowed: it falls back to English at
//! run time and the `every_language_is_complete` test reports it.
//!
//! A message written as an object of plural categories (`one`, `other`, …)
//! is one plural message; its forms are stored as `key#category`.
//!
//! A new language is a new folder. Nothing in Rust names a language.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

use serde_json::Value;

const SOURCE: &str = "en";

type Messages = BTreeMap<String, String>;

struct Language {
    code: String,
    name: String,
    rtl: bool,
    /// The name of a `PluralRule` variant.
    plural: &'static str,
    messages: Messages,
}

/// The plural categories a message may have a form for. A JSON object made
/// only of these, with `other` among them, is one plural message; any other
/// object is a group of messages.
const CATEGORIES: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

/// Separates a plural message's key from the category of one of its forms.
const FORM: char = '#';

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("../../../locales");
    println!("cargo:rerun-if-changed={}", root.display());

    let mut languages: Vec<Language> = fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", root.display()))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| read_language(&entry.path()))
        .collect();
    // English first, then by code, so the order never depends on the disk.
    languages.sort_by_key(|l| (l.code != SOURCE, l.code.clone()));

    let source = languages
        .first()
        .filter(|l| l.code == SOURCE)
        .unwrap_or_else(|| panic!("locales/{SOURCE} is missing"));
    for language in &languages[1..] {
        check(language, &source.messages);
    }

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("catalogue.rs");
    fs::write(out, render(&languages)).unwrap();
}

fn read_language(dir: &Path) -> Language {
    let code = dir.file_name().unwrap().to_string_lossy().into_owned();
    let mut name = code.clone();
    let mut rtl = false;
    let mut plural = "Other";
    let mut messages = Messages::new();
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        let json: Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{}: not valid JSON: {e}", file.display()));
        let namespace = file.file_stem().unwrap().to_string_lossy().into_owned();
        if namespace == "meta" {
            if let Some(n) = json.get("name").and_then(Value::as_str) {
                name = n.to_owned();
            }
            rtl = json.get("dir").and_then(Value::as_str) == Some("rtl");
            plural = match json.get("plural").and_then(Value::as_str) {
                None | Some("other") => "Other",
                Some("one_other") => "OneOther",
                Some(unknown) => panic!(
                    "{}: unknown plural rule {unknown:?}. Known: \"other\", \"one_other\". \
                     A language with other forms needs its rule added to PluralRule first.",
                    file.display()
                ),
            };
        } else {
            flatten(&namespace, &json, &file, &mut messages);
        }
    }
    Language {
        code,
        name,
        rtl,
        plural,
        messages,
    }
}

fn flatten(prefix: &str, value: &Value, file: &Path, out: &mut Messages) {
    match value {
        Value::String(text) => {
            out.insert(prefix.to_owned(), text.clone());
        }
        Value::Object(map) if is_plural(map) => {
            for (category, form) in map {
                let Value::String(text) = form else {
                    panic!("{}: `{prefix}.{category}` must be text", file.display());
                };
                out.insert(format!("{prefix}{FORM}{category}"), text.clone());
            }
        }
        Value::Object(map) => {
            for (name, child) in map {
                flatten(&format!("{prefix}.{name}"), child, file, out);
            }
        }
        _ => panic!(
            "{}: `{prefix}` must be text or a group of texts",
            file.display()
        ),
    }
}

fn is_plural(map: &serde_json::Map<String, Value>) -> bool {
    map.contains_key("other") && map.keys().all(|k| CATEGORIES.contains(&k.as_str()))
}

/// The `{names}` a message expects to be filled in.
fn placeholders(text: &str) -> BTreeSet<&str> {
    let mut found = BTreeSet::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        rest = &rest[open + 1..];
        match rest.find('}') {
            Some(close) => {
                found.insert(&rest[..close]);
                rest = &rest[close + 1..];
            }
            None => break,
        }
    }
    found
}

fn check(language: &Language, source: &Messages) {
    for (key, text) in &language.messages {
        // A plural form is checked against the English `other` form: a
        // language may have categories English lacks, and the reverse.
        let english = match key.split_once(FORM) {
            Some((base, _)) => source.get(&format!("{base}{FORM}other")),
            None => source.get(key),
        };
        let Some(original) = english else {
            panic!(
                "locales/{}: `{key}` is not in locales/{SOURCE} (or is plural in only one of them). Add it to English first, or remove it.",
                language.code
            );
        };
        if placeholders(text) != placeholders(original) {
            panic!(
                "locales/{}: `{key}` must use the same {{placeholders}} as English ({original:?}), found {text:?}",
                language.code
            );
        }
    }
}

fn render(languages: &[Language]) -> String {
    let mut out = String::new();
    out.push_str(
        "/// One constant per message, named after its key.\npub mod key {\n    use super::{Key, PluralKey};\n",
    );
    for key in languages[0].messages.keys() {
        let constant = |name: &str| name.replace(['.', '-'], "_").to_uppercase();
        match key.split_once(FORM) {
            None => writeln!(out, "    pub const {}: Key = Key({key:?});", constant(key)),
            Some((base, "other")) => writeln!(
                out,
                "    pub const {}: PluralKey = PluralKey({base:?});",
                constant(base)
            ),
            // The other forms share the constant of their `other` form.
            Some(_) => Ok(()),
        }
        .unwrap();
    }
    out.push_str("}\n\npub(crate) static LANGUAGES: &[Language] = &[\n");
    for language in languages {
        writeln!(
            out,
            "    Language {{ code: {:?}, name: {:?}, direction: Direction::{}, plural: PluralRule::{}, messages: &[",
            language.code,
            language.name,
            if language.rtl { "Rtl" } else { "Ltr" },
            language.plural
        )
        .unwrap();
        // BTreeMap order is the order `lookup` binary-searches.
        for (key, text) in &language.messages {
            writeln!(out, "        ({key:?}, {text:?}),").unwrap();
        }
        out.push_str("    ] },\n");
    }
    out.push_str("];\n");
    out
}
