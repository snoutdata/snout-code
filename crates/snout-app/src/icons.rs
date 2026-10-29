//! File-type icons for the file tree, the editor tabs and the breadcrumbs: a small
//! monogram in a wash of the type's colour (J for Java, K for Kotlin, G for Gradle and
//! Groovy, …), or a plain page for everything else. Folders use the house folder drawing.

use std::path::Path;

use gpui::{AnyElement, FontWeight, Hsla, IntoElement, ParentElement as _, Styled as _, canvas, div, point, px};

use crate::look;
use crate::theme::{Palette, mix};

/// What a file is, for its icon and the status bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Java,
    Kotlin,
    Groovy,
    Yaml,
    Properties,
    Xml,
    Json,
    Markdown,
    Rust,
    Shell,
    Sql,
    Other,
}

impl FileKind {
    pub fn of(path: &Path) -> Self {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        match ext.as_str() {
            "java" => Self::Java,
            "kt" | "kts" => Self::Kotlin,
            "groovy" | "gradle" | "gvy" => Self::Groovy,
            "yml" | "yaml" => Self::Yaml,
            "properties" => Self::Properties,
            "xml" | "pom" | "xsd" | "html" => Self::Xml,
            "json" => Self::Json,
            "md" | "markdown" => Self::Markdown,
            "rs" => Self::Rust,
            "sh" | "bash" | "zsh" => Self::Shell,
            "sql" => Self::Sql,
            _ if name == "Jenkinsfile" => Self::Groovy,
            _ if name == "gradlew" || name == "mvnw" => Self::Shell,
            _ => Self::Other,
        }
    }

    /// The name the status bar gives the file's language.
    pub fn language(self) -> &'static str {
        match self {
            Self::Java => "Java",
            Self::Kotlin => "Kotlin",
            Self::Groovy => "Groovy",
            Self::Yaml => "YAML",
            Self::Properties => "Properties",
            Self::Xml => "XML",
            Self::Json => "JSON",
            Self::Markdown => "Markdown",
            Self::Rust => "Rust",
            Self::Shell => "Shell",
            Self::Sql => "SQL",
            Self::Other => "Plain Text",
        }
    }

    fn monogram(self) -> Option<&'static str> {
        Some(match self {
            Self::Java => "J",
            Self::Kotlin => "K",
            Self::Groovy => "G",
            Self::Yaml => "Y",
            Self::Properties => "P",
            Self::Xml => "<>",
            Self::Json => "{}",
            Self::Markdown => "M",
            Self::Rust => "R",
            Self::Shell => "$",
            Self::Sql => "Q",
            Self::Other => return None,
        })
    }

    fn color(self, p: &Palette) -> Hsla {
        if p.tinted {
            let i = match self {
                Self::Java => 2,
                Self::Kotlin => 4,
                Self::Groovy => 5,
                Self::Shell => 1,
                Self::Yaml | Self::Json | Self::Sql => 6,
                Self::Xml => 7,
                Self::Markdown => 0,
                Self::Rust => 3,
                Self::Properties | Self::Other => return p.fg_dim,
            };
            return p.hues[i];
        }
        let [green, warm, blue, clay, violet, dim] = p.lanes();
        match self {
            Self::Java | Self::Rust => clay,
            Self::Kotlin => violet,
            Self::Groovy | Self::Shell => green,
            Self::Yaml | Self::Json | Self::Sql => warm,
            Self::Xml | Self::Markdown => blue,
            Self::Properties | Self::Other => dim,
        }
    }
}

/// The icon of the file at `path`, `size` square.
pub fn file_icon(path: &Path, size: f32, p: &Palette) -> AnyElement {
    let kind = FileKind::of(path);
    let Some(text) = kind.monogram() else { return page(size, mix(p.fg, 0.55, p.panel)).into_any_element() };
    let color = kind.color(p);
    if p.tinted {
        // A solid tile of the type's colour, the letter cut out of it.
        let (tile, ink) = p.tile(color);
        return div()
            .flex_none()
            .size(px(size))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.5))
            .bg(tile)
            .text_color(ink)
            .font_family(look::MONO)
            .font_weight(FontWeight::BOLD)
            .text_size(px(if text.len() > 1 { size * 0.5 } else { size * 0.66 }))
            .line_height(px(size))
            .child(text)
            .into_any_element();
    }
    div()
        .flex_none()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.5))
        .bg(mix(color, if p.dark { 0.3 } else { 0.22 }, p.panel))
        .border_1()
        .border_color(mix(color, 0.45, p.panel))
        .text_color(color)
        .font_family(look::MONO)
        .font_weight(FontWeight::BOLD)
        .text_size(px(if text.len() > 1 { size * 0.5 } else { size * 0.66 }))
        .line_height(px(size))
        .child(text)
        .into_any_element()
}

/// A page with a folded corner, drawn in one line like the house drawings.
fn page(size: f32, ink: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let o = b.origin;
            let s = f32::from(b.size.width) / 16.;
            let at = |x: f32, y: f32| point(o.x + px(x * s), o.y + px(y * s));
            let mut path = gpui::PathBuilder::stroke(px(1.2));
            path.move_to(at(3.5, 1.5));
            path.line_to(at(9.5, 1.5));
            path.line_to(at(13., 5.));
            path.line_to(at(13., 14.5));
            path.line_to(at(3.5, 14.5));
            path.close();
            let mut fold = gpui::PathBuilder::stroke(px(1.2));
            fold.move_to(at(9.5, 1.5));
            fold.line_to(at(9.5, 5.));
            fold.line_to(at(13., 5.));
            for path in [path, fold] {
                if let Ok(path) = path.build() {
                    window.paint_path(path, ink);
                }
            }
        },
    )
    .flex_none()
    .size(px(size))
}
