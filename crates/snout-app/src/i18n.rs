//! The interface's languages: English, French, Spanish and Portuguese (Brazilian).
//!
//! The words are Fluent catalogs, one per language, in `i18n/<language>/snout-app.ftl`,
//! embedded in the program. English (`i18n/en`) is the source: every message is there, and
//! [`t!`] checks at compile time that the message it names is there and that its arguments
//! are the ones the message takes. A message missing from another language falls back to
//! English.
//!
//! The language is the user's choice (Settings ▸ Appearance ▸ Language) or, by default, the
//! system's. [`select`] puts it in place for the whole app; what is drawn after reads it.
//! Nothing here runs in the tests unless a test asks: they see English.

use std::sync::LazyLock;

use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::{DesktopLanguageRequester, LanguageLoader as _};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use unic_langid::LanguageIdentifier;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Catalogs;

/// The words of the language in use, and English under them.
pub static LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    let en = loader.fallback_language().clone();
    loader.load_languages(&Catalogs, &[en]).expect("the English catalog loads");
    // Without Unicode isolation marks around each value put in a message: the text is drawn
    // as it is, and a file name in a sentence stays the file name.
    loader.set_use_isolating(false);
    loader
});

/// A message of the catalog in the language in use: `t!("menu-file")`, or with the values it
/// takes, `t!("tests-passed", count = 3)`. The id and the arguments are checked against the
/// English catalog when the app is compiled.
#[macro_export]
macro_rules! t {
    ($id:literal) => {{ i18n_embed_fl::fl!($crate::i18n::LOADER, $id) }};
    ($id:literal, $($args:tt)*) => {{ i18n_embed_fl::fl!($crate::i18n::LOADER, $id, $($args)*) }};
}

/// Whether `System` follows the system's languages yet.
const FOLLOW_SYSTEM: bool = false;

/// The language the interface speaks: the system's, or one chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Language {
    /// The first of the system's preferred languages SnoutCode speaks, else English.
    #[default]
    System,
    English,
    French,
    Spanish,
    /// Brazilian Portuguese.
    Portuguese,
}

impl Language {
    /// The languages offered, the system's way first.
    pub const ALL: [Self; 5] = [Self::System, Self::English, Self::French, Self::Spanish, Self::Portuguese];

    /// The catalog's language tag; none for the system's way.
    pub fn tag(self) -> Option<&'static str> {
        match self {
            Self::System => None,
            Self::English => Some("en"),
            Self::French => Some("fr"),
            Self::Spanish => Some("es"),
            Self::Portuguese => Some("pt-BR"),
        }
    }

    /// The language's name in itself, as a language menu lists it whatever the language in
    /// use. The system's way is named in the language in use.
    pub fn native_name(self) -> String {
        match self {
            Self::System => t!("language-system", language = self.resolved().native_name()),
            Self::English => "English".into(),
            Self::French => "Français".into(),
            Self::Spanish => "Español".into(),
            Self::Portuguese => "Português (Brasil)".into(),
        }
    }

    /// The language this choice speaks: the system's way resolved to one of the catalogs.
    pub fn resolved(self) -> Self {
        match self {
            // Not until the whole interface is translated: a half-translated window is worse.
            Self::System if !FOLLOW_SYSTEM => Self::English,
            Self::System => from_requested(&DesktopLanguageRequester::requested_languages()),
            other => other,
        }
    }
}

/// The first of `requested` (the system's preferred languages, most preferred first) that has
/// a catalog, by language whatever the region (`pt-PT` gets the Brazilian catalog, `fr-CA` the
/// French); English when none has.
pub fn from_requested(requested: &[LanguageIdentifier]) -> Language {
    requested
        .iter()
        .find_map(|id| match id.language.as_str() {
            "en" => Some(Language::English),
            "fr" => Some(Language::French),
            "es" => Some(Language::Spanish),
            "pt" => Some(Language::Portuguese),
            _ => None,
        })
        .unwrap_or(Language::English)
}

/// Speak `choice` from now on: the next things drawn read it. Returns the language put in
/// place.
pub fn select(choice: Language) -> Language {
    let language = choice.resolved();
    let tag: LanguageIdentifier = language.tag().unwrap_or("en").parse().expect("a catalog's tag parses");
    let en = LOADER.fallback_language().clone();
    let languages = if tag == en { vec![en] } else { vec![tag, en] };
    if LOADER.current_languages() != languages {
        if let Err(e) = LOADER.load_languages(&Catalogs, &languages) {
            log::warn!("the {} catalog does not load: {e}", languages[0]);
        }
        LOADER.set_use_isolating(false);
    }
    language
}

/// The language in use.
pub fn current() -> Language {
    from_requested(&[LOADER.current_language()])
}

/// The catalogs embedded, each by its tag, as text: for the tests that hold them to English.
pub fn catalogs() -> Vec<(String, String)> {
    Catalogs::iter()
        .filter_map(|path| {
            let tag = path.split('/').next()?.to_owned();
            let file = Catalogs::get(&path)?;
            Some((tag, String::from_utf8_lossy(&file.data).into_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    /// Each message of a catalog, with the arguments it takes.
    fn messages(text: &str) -> BTreeMap<String, BTreeSet<String>> {
        let arg = regex::Regex::new(r"\{ *\$([a-z_-]+)").unwrap();
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with(' '))
            .filter_map(|l| l.split_once(" = "))
            .map(|(id, value)| (id.to_owned(), arg.captures_iter(value).map(|c| c[1].to_owned()).collect()))
            .collect()
    }

    fn catalog(tag: &str) -> String {
        catalogs().into_iter().find(|(t, _)| t == tag).unwrap_or_else(|| panic!("no {tag} catalog")).1
    }

    #[test]
    fn every_language_has_every_message_with_its_arguments() {
        let en = messages(&catalog("en"));
        assert!(en.len() > 200, "{} messages", en.len());
        for tag in ["fr", "es", "pt-BR"] {
            assert_eq!(messages(&catalog(tag)), en, "the {tag} catalog");
        }
    }

    #[test]
    fn every_catalog_parses_and_says_its_own_words() {
        for (tag, save) in [("en", "Save"), ("fr", "Enregistrer"), ("es", "Guardar"), ("pt-BR", "Salvar")] {
            let loader: FluentLanguageLoader = fluent_language_loader!();
            let id: LanguageIdentifier = tag.parse().unwrap();
            loader.load_languages(&Catalogs, &[id]).unwrap_or_else(|e| panic!("{tag}: {e}"));
            assert_eq!(loader.get("cmd-save"), save, "{tag}");
        }
    }

    #[test]
    fn no_catalog_uses_an_em_dash() {
        for (tag, text) in catalogs() {
            assert!(!text.contains('\u{2014}'), "{tag} has an em dash");
        }
    }

    #[test]
    fn the_system_language_goes_by_language_whatever_the_region() {
        let ids = |tags: &[&str]| tags.iter().map(|t| t.parse().unwrap()).collect::<Vec<LanguageIdentifier>>();
        assert_eq!(from_requested(&ids(&["fr-CA"])), Language::French);
        assert_eq!(from_requested(&ids(&["pt-PT"])), Language::Portuguese);
        assert_eq!(from_requested(&ids(&["es-MX", "en-US"])), Language::Spanish);
        // The first one there is a catalog for.
        assert_eq!(from_requested(&ids(&["de-DE", "pt-BR", "fr"])), Language::Portuguese);
        assert_eq!(from_requested(&ids(&["de-DE"])), Language::English);
        assert_eq!(from_requested(&[]), Language::English);
    }

    #[test]
    fn the_tests_speak_english() {
        assert_eq!(t!("cmd-save"), "Save");
        assert_eq!(t!("cmd-open-recent-project", name = "web"), "Open Recent: web");
    }
}
