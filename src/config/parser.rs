use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct IniParser;

impl IniParser {
    pub fn parse_file<P: AsRef<Path>>(
        path: P,
    ) -> Result<HashMap<String, HashMap<String, String>>, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read config file: {}", e))?;
        Self::parse_str(&content)
    }

    pub fn parse_str(content: &str) -> Result<HashMap<String, HashMap<String, String>>, String> {
        let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut current_section = String::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            // Parse section header [section:name]
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                sections
                    .entry(current_section.clone())
                    .or_insert_with(HashMap::new);
            } else if
            // Parse key = value
            let Some((key, value)) = line.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();

                if current_section.is_empty() {
                    return Err(format!(
                        "Key-value pair at line {} outside of section",
                        line_num + 1
                    ));
                }

                if let Some(section) = sections.get_mut(&current_section) {
                    section.insert(key, value);
                } else {
                    return Err(format!(
                        "Internal parser error at line {}: section '{}' missing",
                        line_num + 1,
                        current_section
                    ));
                }
            } else {
                return Err(format!("Invalid syntax at line {}: {}", line_num + 1, line));
            }
        }

        Ok(sections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_parse_basic_ini() {
        let content = r#"
# Comment
[section1]
key1 = value1
key2 = value2

[section2]
key3 = value3
"#;
        let result = IniParser::parse_str(content).unwrap();
        assert_eq!(
            result.get("section1").unwrap().get("key1").unwrap(),
            "value1"
        );
        assert_eq!(
            result.get("section2").unwrap().get("key3").unwrap(),
            "value3"
        );
    }

    #[test]
    fn test_parse_rejects_key_value_outside_section() {
        let content = "key = value";
        let err = IniParser::parse_str(content).expect_err("must fail outside section");
        assert_eq!(err, "Key-value pair at line 1 outside of section");
    }

    #[test]
    fn test_parse_rejects_invalid_syntax_line() {
        let content = "[s]\nnot-valid";
        let err = IniParser::parse_str(content).expect_err("must fail on invalid line");
        assert_eq!(err, "Invalid syntax at line 2: not-valid");
    }

    #[test]
    fn test_parse_file_reports_read_error() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let missing = std::env::temp_dir().join(format!("servex_missing_conf_{nanos}.ini"));

        let err = IniParser::parse_file(&missing).expect_err("missing file must fail");
        assert!(err.starts_with("Failed to read config file: "));
        let _ = fs::remove_file(missing);
    }
}
