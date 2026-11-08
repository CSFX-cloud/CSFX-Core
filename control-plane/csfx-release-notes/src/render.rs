use crate::model::{Block, Category, ReleaseNotes};

pub fn to_markdown(notes: &ReleaseNotes, asset_base: &str) -> String {
    let asset_base = asset_base.trim_end_matches('/');
    let mut out = format!("**{}** - {}\n\n", notes.channel.label(), notes.date);
    if !notes.breaking.is_empty() {
        out.push_str("## Breaking changes\n\n");
        out.push_str(&list_markdown(&notes.breaking));
    }
    for category in Category::ALL {
        let Some(blocks) = notes.sections.get(&category) else {
            continue;
        };
        out.push_str(&format!("## {}\n\n", category.label()));
        blocks.iter().for_each(|block| out.push_str(&block_markdown(block, asset_base)));
    }
    out.trim_end().to_string() + "\n"
}

fn block_markdown(block: &Block, asset_base: &str) -> String {
    match block {
        Block::Heading { text } => format!("### {text}\n\n"),
        Block::Paragraph { text } => format!("{text}\n\n"),
        Block::List { items } => list_markdown(items),
        Block::Image { src, alt } => format!("![{alt}]({asset_base}/{src})\n\n"),
    }
}

fn list_markdown(items: &[String]) -> String {
    let lines: String = items.iter().map(|item| format!("- {item}\n")).collect();
    lines + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    #[test]
    fn renders_categories_in_order_with_absolute_images() {
        let source = "---\nversion: 0.3.0\nchannel: stable\ndate: 2026-11-08\nbreaking:\n  - Migrate first\n---\n\n## Fixed\n\n- A\n\n## New\n\n![Shot](shot.png)\n";
        let markdown = to_markdown(&parse(source).unwrap(), "https://example.test/dl/");
        assert_eq!(
            markdown,
            "**Stable** - 2026-11-08\n\n## Breaking changes\n\n- Migrate first\n\n## New\n\n![Shot](https://example.test/dl/shot.png)\n\n## Fixed\n\n- A\n"
        );
    }

    #[test]
    fn rewrites_image_sources_for_api_consumers() {
        let source = "---\nversion: 0.3.0\nchannel: stable\ndate: 2026-11-08\n---\n\n## New\n\n![Shot](shot.png)\n";
        let notes = parse(source).unwrap().with_asset_base("https://mirror.test/dl/");
        assert_eq!(
            notes.sections[&Category::New],
            vec![Block::Image { src: "https://mirror.test/dl/shot.png".into(), alt: "Shot".into() }]
        );
    }
}
