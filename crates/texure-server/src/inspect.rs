/// Embedded single-page Inspector assets.
pub const INSPECT_HTML: &str = include_str!("../assets/inspect.html");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspect_html_non_empty_and_valid() {
        assert!(!INSPECT_HTML.is_empty(), "INSPECT_HTML must not be empty");
        assert!(
            INSPECT_HTML.contains("<!DOCTYPE html>"),
            "Must have DOCTYPE declaration"
        );
        assert!(INSPECT_HTML.contains("<html"), "Must contain <html> tag");
        assert!(INSPECT_HTML.contains("<head>"), "Must contain <head> tag");
        assert!(INSPECT_HTML.contains("<body>"), "Must contain <body> tag");
        assert!(INSPECT_HTML.contains("</html>"), "Must close </html>");

        // UI components required by ticket #75
        assert!(
            INSPECT_HTML.contains("screen-canvas") || INSPECT_HTML.contains("canvas"),
            "Must have screen canvas"
        );
        assert!(
            INSPECT_HTML.contains("Node detail") || INSPECT_HTML.contains("node-detail"),
            "Must contain node detail drawer"
        );
        assert!(
            INSPECT_HTML.contains("Copy Tool Call") || INSPECT_HTML.contains("copy-tool-call"),
            "Must have Copy Tool Call action preview"
        );
        assert!(
            INSPECT_HTML.contains("MCP JSON") || INSPECT_HTML.contains("mcp"),
            "Must have MCP JSON preview"
        );
        assert!(
            INSPECT_HTML.contains("Python") || INSPECT_HTML.contains("python"),
            "Must have Python preview"
        );
        assert!(
            INSPECT_HTML.contains("Bash") || INSPECT_HTML.contains("bash"),
            "Must have Bash preview"
        );

        // Required node attributes in detail drawer
        let required_attrs = [
            "index",
            "resource-id",
            "tag",
            "text",
            "cd",
            "bounds",
            "clickable",
            "editable",
            "semantic hash",
        ];
        for attr in required_attrs {
            assert!(
                INSPECT_HTML.to_lowercase().contains(&attr.to_lowercase()),
                "Detail drawer must mention '{attr}'"
            );
        }
    }

    #[test]
    fn test_inspect_html_no_external_resources() {
        // Zero external dependencies / no network calls
        assert!(
            !INSPECT_HTML.contains("http://"),
            "INSPECT_HTML must not contain unencrypted external http:// links"
        );
        assert!(
            !INSPECT_HTML.contains("https://"),
            "INSPECT_HTML must not contain external https:// dependencies"
        );
        assert!(
            !INSPECT_HTML.contains("<link rel=\"stylesheet\" href=\"//"),
            "No protocol-relative stylesheets"
        );
        assert!(
            !INSPECT_HTML.contains("<script src=\"//"),
            "No protocol-relative scripts"
        );
    }
}
