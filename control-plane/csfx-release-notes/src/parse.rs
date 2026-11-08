use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::NotesError;
use crate::model::{Block, Category, Channel, ReleaseNotes};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Frontmatter {
    pub version: String,
    pub channel: Channel,
    pub date: String,
    #[serde(default)]
    pub breaking: Vec<String>,
}

pub fn parse(source: &str) -> Result<ReleaseNotes, NotesError> {
    let (front, body, offset) = split_frontmatter(source)?;
    let meta: Frontmatter = serde_yaml::from_str(front)?;
    validate_frontmatter(&meta)?;
    let notes = ReleaseNotes {
        version: meta.version,
        channel: meta.channel,
        date: meta.date,
        breaking: meta.breaking,
        sections: parse_body(body, offset)?,
    };
    ensure_entries(&notes)?;
    Ok(notes)
}

fn split_frontmatter(source: &str) -> Result<(&str, &str, usize), NotesError> {
    let rest = source
        .strip_prefix("---\n")
        .ok_or(NotesError::MissingFrontmatter)?;
    let (front, body) = rest
        .split_once("\n---\n")
        .ok_or(NotesError::MissingFrontmatter)?;
    Ok((front, body, front.lines().count() + 2))
}

pub(crate) fn validate_frontmatter(meta: &Frontmatter) -> Result<(), NotesError> {
    validate_version(&meta.version)?;
    if Channel::of_version(&meta.version) != meta.channel {
        return Err(NotesError::ChannelMismatch {
            channel: meta.channel.label(),
            version: meta.version.clone(),
        });
    }
    validate_date(&meta.date)?;
    if meta.breaking.iter().any(|entry| entry.trim().is_empty()) {
        return Err(NotesError::EmptyBreakingEntry);
    }
    Ok(())
}

fn validate_version(version: &str) -> Result<(), NotesError> {
    let invalid = || NotesError::InvalidVersion(version.to_string());
    let (base, suffix) = match version.split_once('-') {
        Some((base, suffix)) => (base, Some(suffix)),
        None => (version, None),
    };
    let numeric = base.split('.').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    let suffix_valid = suffix.is_none_or(|text| !text.is_empty() && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-'));
    if base.split('.').count() != 3 || !numeric || !suffix_valid {
        return Err(invalid());
    }
    Ok(())
}

fn validate_date(date: &str) -> Result<(), NotesError> {
    let parts: Vec<&str> = date.split('-').collect();
    let number = |part: &str| part.bytes().all(|b| b.is_ascii_digit()).then(|| part.parse::<u32>().ok()).flatten();
    let valid = matches!(parts.as_slice(), [year, month, day]
        if year.len() == 4
            && month.len() == 2
            && day.len() == 2
            && number(year).is_some()
            && number(month).is_some_and(|m| (1..=12).contains(&m))
            && number(day).is_some_and(|d| (1..=31).contains(&d)));
    if valid {
        Ok(())
    } else {
        Err(NotesError::InvalidDate(date.to_string()))
    }
}

fn ensure_entries(notes: &ReleaseNotes) -> Result<(), NotesError> {
    if notes.breaking.is_empty() && notes.sections.values().all(Vec::is_empty) {
        return Err(NotesError::Empty);
    }
    Ok(())
}

fn parse_body(body: &str, offset: usize) -> Result<BTreeMap<Category, Vec<Block>>, NotesError> {
    let mut builder = Builder::default();
    for (index, line) in body.lines().enumerate() {
        builder.feed(offset + index + 1, line.trim_end())?;
    }
    builder.finish(offset + body.lines().count() + 1)
}

#[derive(Default)]
struct Builder {
    sections: BTreeMap<Category, Vec<Block>>,
    current: Option<Category>,
    paragraph: Vec<String>,
    list: Vec<String>,
}

impl Builder {
    fn feed(&mut self, line_no: usize, line: &str) -> Result<(), NotesError> {
        if line.is_empty() {
            return self.flush(line_no);
        }
        if let Some(label) = line.strip_prefix("## ") {
            self.flush(line_no)?;
            self.current = Some(Category::from_label(label.trim()).ok_or_else(|| NotesError::syntax(line_no, "unknown category"))?);
            return Ok(());
        }
        if let Some(text) = line.strip_prefix("### ") {
            self.flush(line_no)?;
            return self.push(line_no, Block::Heading { text: text.trim().to_string() });
        }
        if line.starts_with('#') {
            return Err(NotesError::syntax(line_no, "unsupported heading level"));
        }
        if line.starts_with("![") {
            self.flush(line_no)?;
            return self.push(line_no, parse_image(line_no, line)?);
        }
        self.text_line(line_no, line)
    }

    fn text_line(&mut self, line_no: usize, line: &str) -> Result<(), NotesError> {
        if let Some(item) = line.strip_prefix("- ") {
            self.flush_paragraph(line_no)?;
            self.list.push(item.trim().to_string());
            return Ok(());
        }
        self.flush_list(line_no)?;
        self.paragraph.push(line.trim().to_string());
        Ok(())
    }

    fn flush(&mut self, line_no: usize) -> Result<(), NotesError> {
        self.flush_paragraph(line_no)?;
        self.flush_list(line_no)
    }

    fn flush_paragraph(&mut self, line_no: usize) -> Result<(), NotesError> {
        if self.paragraph.is_empty() {
            return Ok(());
        }
        let text = self.paragraph.drain(..).collect::<Vec<_>>().join(" ");
        self.push(line_no, Block::Paragraph { text })
    }

    fn flush_list(&mut self, line_no: usize) -> Result<(), NotesError> {
        if self.list.is_empty() {
            return Ok(());
        }
        let items = std::mem::take(&mut self.list);
        self.push(line_no, Block::List { items })
    }

    fn push(&mut self, line_no: usize, block: Block) -> Result<(), NotesError> {
        let category = self
            .current
            .ok_or_else(|| NotesError::syntax(line_no, "content before first category heading"))?;
        self.sections.entry(category).or_default().push(block);
        Ok(())
    }

    fn finish(mut self, line_no: usize) -> Result<BTreeMap<Category, Vec<Block>>, NotesError> {
        self.flush(line_no)?;
        Ok(self.sections)
    }
}

fn parse_image(line_no: usize, line: &str) -> Result<Block, NotesError> {
    let parsed = line
        .strip_prefix("![")
        .and_then(|rest| rest.split_once("]("))
        .and_then(|(alt, tail)| tail.strip_suffix(')').map(|src| (alt.trim(), src.trim())))
        .filter(|(alt, src)| !alt.is_empty() && !src.is_empty() && !src.contains('/'));
    let (alt, src) = parsed.ok_or_else(|| NotesError::syntax(line_no, "invalid image, expected ![alt](file-name)"))?;
    Ok(Block::Image {
        src: src.to_string(),
        alt: alt.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "---\nversion: 0.3.0-alpha.1\nchannel: pre\ndate: 2026-11-08\nbreaking:\n  - Run csfx-migrate first\n---\n\n## New\n\n### Panel\nShort text\nsecond line\n\n![Panel](panel.png)\n\n## Fixed\n\n- One\n- Two\n";

    fn replaced(from: &str, to: &str) -> String {
        VALID.replace(from, to)
    }

    #[test]
    fn parses_valid_notes() {
        let notes = parse(VALID).unwrap();
        assert_eq!(notes.channel, Channel::Pre);
        assert_eq!(notes.breaking.len(), 1);
        assert_eq!(notes.sections[&Category::New].len(), 3);
        assert_eq!(
            notes.sections[&Category::Fixed],
            vec![Block::List { items: vec!["One".into(), "Two".into()] }]
        );
        assert_eq!(
            notes.sections[&Category::New][1],
            Block::Paragraph { text: "Short text second line".into() }
        );
    }

    #[test]
    fn rejects_channel_mismatch() {
        assert!(matches!(parse(&replaced("channel: pre", "channel: stable")), Err(NotesError::ChannelMismatch { .. })));
    }

    #[test]
    fn rejects_invalid_version_and_date() {
        assert!(matches!(parse(&replaced("0.3.0-alpha.1", "0.3")), Err(NotesError::InvalidVersion(_))));
        assert!(matches!(parse(&replaced("2026-11-08", "2026-13-08")), Err(NotesError::InvalidDate(_))));
    }

    #[test]
    fn rejects_unknown_category_and_frontmatter_key() {
        assert!(matches!(parse(&replaced("## Fixed", "## Misc")), Err(NotesError::Syntax { .. })));
        assert!(matches!(parse(&replaced("date:", "extra: 1\ndate:")), Err(NotesError::Frontmatter(_))));
    }

    #[test]
    fn rejects_bad_image_and_missing_entries() {
        assert!(matches!(parse(&replaced("panel.png", "dir/panel.png")), Err(NotesError::Syntax { .. })));
        let empty = "---\nversion: 0.3.0\nchannel: stable\ndate: 2026-11-08\n---\n";
        assert!(matches!(parse(empty), Err(NotesError::Empty)));
        assert!(matches!(parse("no frontmatter"), Err(NotesError::MissingFrontmatter)));
    }
}
