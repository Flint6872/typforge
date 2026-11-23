use spellbook::Dictionary;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DictionaryManager {
    dict_dir: PathBuf,
    fallback_lang: String,
    custom_ignore: HashSet<String>,
}

impl DictionaryManager {
    pub fn new(dict_dir: impl Into<PathBuf>) -> Self {
        Self {
            dict_dir: dict_dir.into(),
            fallback_lang: "en_US".to_string(),
            custom_ignore: HashSet::new(),
        }
    }

    pub fn add_ignored_word(&mut self, word: impl Into<String>) {
        self.custom_ignore.insert(word.into().to_lowercase()); // Store lowercase
    }

    pub fn add_ignored_words<I: IntoIterator<Item = String>>(&mut self, words: I) {
        self.custom_ignore
            .extend(words.into_iter().map(|w| w.to_lowercase())); // Store lowercase
    }

    pub fn load_ignore_file(&mut self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                self.custom_ignore.insert(trimmed.to_string());
            }
        }
        Ok(())
    }

    pub fn ignored_words(&self) -> &HashSet<String> {
        &self.custom_ignore
    }

    pub fn is_ignored(&self, word: &str) -> bool {
        self.custom_ignore.contains(&word.to_lowercase()) // Check lowercase
    }

    /// Loads `<lang>.aff` + `<lang>.dic` via `spellbook::Dictionary`, falling back to `en_US`.
    pub fn load_dictionary(&self, lang: &str) -> Result<Arc<Dictionary>, String> {
        let normalized = normalize_lang(lang);
        let candidates = [normalized.as_str(), lang, self.fallback_lang.as_str()];

        for code in candidates {
            let aff_path = self.dict_dir.join(format!("{code}.aff"));
            let dic_path = self.dict_dir.join(format!("{code}.dic"));

            if aff_path.exists() && dic_path.exists() {
                // Read the file contents as strings
                let aff_content = fs::read_to_string(&aff_path)
                    .map_err(|e| format!("Failed to read {}: {}", aff_path.display(), e))?;
                let dic_content = fs::read_to_string(&dic_path)
                    .map_err(|e| format!("Failed to read {}: {}", dic_path.display(), e))?;

                // Use Dictionary::new instead of Dictionary::from_files
                let dict = Dictionary::new(&aff_content, &dic_content).map_err(|e| {
                    format!("Failed to parse spellbook dictionary for '{code}': {e:?}")
                })?;

                return Ok(Arc::new(dict));
            }
        }

        Err(format!(
            "No dictionary found for '{lang}' or fallback '{}' in {}",
            self.fallback_lang,
            self.dict_dir.display()
        ))
    }
}

pub fn normalize_lang(lang: &str) -> String {
    match lang.trim().to_lowercase().as_str() {
        "en" | "en-us" => "en_US".to_string(),
        "en-gb" => "en_GB".to_string(),
        "de" | "de-de" => "de_DE".to_string(),
        "fr" | "fr-fr" => "fr_FR".to_string(),
        "es" | "es-es" => "es_ES".to_string(),
        "it" | "it-it" => "it_IT".to_string(),
        "pt" | "pt-br" => "pt_BR".to_string(),
        other => other.replace('-', "_"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_lang_and_ignore_list() {
        assert_eq!(normalize_lang("en"), "en_US");
        assert_eq!(normalize_lang("de-DE"), "de_DE");

        let mut mgr = DictionaryManager::new("/nonexistent");
        mgr.add_ignored_word("Typst");
        assert!(mgr.is_ignored("Typst"));
        assert!(mgr.is_ignored("typst"));
    }
}
