// typforge-core/src/format.rs

use typstyle_core::{Config, Typstyle};

/// Formats the given Typst document source content using `typstyle`.
pub fn format_document(content: &str, line_width: usize) -> Result<String, String> {
    // 1. Setup configuration
    let mut config = Config::default();
    config.max_width = line_width;

    // 2. Initialize the formatter
    let formatter = Typstyle::new(config);

    // 3. Perform formatting
    // format_text returns a Formatter, we then call render() to get the result
    formatter
        .format_text(content)
        .render()
        .map_err(|e| e.to_string())
}
