pub const MAX_EXCLUSION_BYTES: usize = 4096;
pub const MAX_EXCLUSIONS: usize = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Exclusions {
    entries: Vec<String>,
}

impl Exclusions {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        if text.len() > MAX_EXCLUSION_BYTES {
            return Err("Exclusion list exceeds 4096 bytes.");
        }
        let mut entries = Vec::new();
        for line in text.lines() {
            let value = line
                .trim()
                .trim_matches('"')
                .replace('/', "\\")
                .to_lowercase();
            if value.is_empty() {
                continue;
            }
            let absolute = value.starts_with("\\\\") || value.as_bytes().get(1..3) == Some(b":\\");
            if !value.ends_with(".exe")
                || value
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '*' | '?' | '"' | '<' | '>' | '|'))
                || (value.contains('\\') && !absolute)
                || (value.contains(':') && !absolute)
            {
                return Err("Expected an executable name or absolute executable path.");
            }
            if !entries.contains(&value) {
                if entries.len() == MAX_EXCLUSIONS {
                    return Err("Exclusion list exceeds 32 entries.");
                }
                entries.push(value);
            }
        }
        let encoded_len =
            entries.iter().map(String::len).sum::<usize>() + entries.len().saturating_sub(1) * 2;
        if encoded_len > MAX_EXCLUSION_BYTES {
            return Err("Normalized exclusion list exceeds 4096 bytes.");
        }
        Ok(Self { entries })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn matches(&self, executable_path: &str) -> bool {
        let path = executable_path.replace('/', "\\").to_lowercase();
        let name = path.rsplit('\\').next().unwrap_or(&path);
        self.entries.iter().any(|entry| {
            if entry.contains('\\') {
                entry == &path
            } else {
                entry == name
            }
        })
    }

    pub fn text(&self) -> String {
        self.entries.join("\r\n")
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}
