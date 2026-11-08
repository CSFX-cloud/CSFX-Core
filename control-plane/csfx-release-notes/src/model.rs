use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    New,
    Improved,
    Fixed,
    Security,
}

impl Category {
    pub const ALL: [Category; 4] = [Self::New, Self::Improved, Self::Fixed, Self::Security];

    pub fn label(self) -> &'static str {
        match self {
            Self::New => "New",
            Self::Improved => "Improved",
            Self::Fixed => "Fixed",
            Self::Security => "Security",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|category| category.label() == label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Pre,
    Stable,
}

impl Channel {
    pub fn of_version(version: &str) -> Self {
        if version.contains('-') {
            Self::Pre
        } else {
            Self::Stable
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pre => "Pre-release",
            Self::Stable => "Stable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Block {
    Heading { text: String },
    Paragraph { text: String },
    List { items: Vec<String> },
    Image { src: String, alt: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseNotes {
    pub version: String,
    pub channel: Channel,
    pub date: String,
    pub breaking: Vec<String>,
    pub sections: BTreeMap<Category, Vec<Block>>,
}

impl ReleaseNotes {
    pub fn with_asset_base(mut self, base: &str) -> Self {
        let base = base.trim_end_matches('/');
        for block in self.sections.values_mut().flatten() {
            if let Block::Image { src, .. } = block {
                *src = format!("{base}/{src}");
            }
        }
        self
    }
}
