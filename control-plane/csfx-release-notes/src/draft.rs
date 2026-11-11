use std::collections::BTreeMap;

use crate::error::NotesError;
use crate::model::{Category, Channel};
use crate::parse::Frontmatter;

pub struct Commit {
    pub subject: String,
    pub body: String,
}

struct Entry {
    category: Option<Category>,
    breaking: Option<String>,
    description: String,
}

pub fn draft(version: &str, date: &str, commits: &[Commit]) -> Result<String, NotesError> {
    let entries: Vec<Entry> = commits.iter().filter_map(classify).collect();
    let frontmatter = Frontmatter {
        version: version.to_string(),
        channel: Channel::of_version(version),
        date: date.to_string(),
        breaking: entries
            .iter()
            .filter_map(|entry| entry.breaking.clone())
            .collect(),
    };
    let yaml = serde_yaml::to_string(&frontmatter)?;
    Ok(format!("---\n{yaml}---\n\n{}", sections_markdown(&entries)))
}

fn sections_markdown(entries: &[Entry]) -> String {
    let mut grouped: BTreeMap<Category, Vec<&str>> = BTreeMap::new();
    for entry in entries {
        if let Some(category) = entry.category {
            grouped
                .entry(category)
                .or_default()
                .push(&entry.description);
        }
    }
    grouped
        .iter()
        .map(|(category, items)| {
            let lines: String = items.iter().map(|item| format!("- {item}\n")).collect();
            format!("## {}\n\n{lines}\n", category.label())
        })
        .collect::<String>()
        .trim_end()
        .to_string()
        + "\n"
}

fn classify(commit: &Commit) -> Option<Entry> {
    let (head, description) = commit.subject.split_once(": ")?;
    let marked_breaking = head.ends_with('!');
    let head = head.trim_end_matches('!');
    let (kind, scope) = match head.split_once('(') {
        Some((kind, scope)) => (kind, scope.trim_end_matches(')')),
        None => (head, ""),
    };
    let category = category_of(kind, scope);
    let footer = breaking_footer(&commit.body);
    let breaking =
        (marked_breaking || footer.is_some()).then(|| footer.unwrap_or(description).to_string());
    (category.is_some() || breaking.is_some()).then(|| Entry {
        category,
        breaking,
        description: description.to_string(),
    })
}

fn category_of(kind: &str, scope: &str) -> Option<Category> {
    if scope == "security" {
        return Some(Category::Security);
    }
    match kind {
        "feat" => Some(Category::New),
        "fix" => Some(Category::Fixed),
        "refactor" | "perf" | "style" => Some(Category::Improved),
        _ => None,
    }
}

fn breaking_footer(body: &str) -> Option<&str> {
    body.lines()
        .find_map(|line| line.strip_prefix("BREAKING CHANGE:"))
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn commit(subject: &str, body: &str) -> Commit {
        Commit {
            subject: subject.into(),
            body: body.into(),
        }
    }

    #[test]
    fn groups_commits_and_produces_parseable_notes() {
        let commits = [
            commit("feat: add panel", ""),
            commit("fix(security): rotate tokens", ""),
            commit("fix: stop flicker", ""),
            commit(
                "refactor!: move keys",
                "BREAKING CHANGE: run csfx-migrate: first",
            ),
            commit("chore: bump deps", ""),
            commit("not conventional", ""),
        ];
        let markdown = draft("0.3.0", "2026-11-08", &commits).unwrap();
        let notes = parse(&markdown).unwrap();
        assert_eq!(notes.breaking, vec!["run csfx-migrate: first".to_string()]);
        assert_eq!(notes.sections.len(), 4);
        assert!(!markdown.contains("bump deps"));
    }
}
