/// Escape a string for safe XML inclusion.
/// Matches Python protectXml() exactly.
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "&#x0a;")
}
