//! LLM provider trait and common types.

use serde::{Deserialize, Serialize};

pub mod anthropic;
pub mod ollama;
pub mod openai;

use crate::Result;

/// Result of LLM analysis for a single file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmAnalysisResult {
    /// Whether the file is safe to delete according to LLM.
    pub safe_to_delete: bool,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// Human-readable explanation.
    pub reason: String,
    /// File category determined by LLM.
    pub category: String,
}

/// A chunk of streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamChunk {
    /// A partial text chunk.
    Text(String),
    /// A complete analysis result (parsed from stream).
    Analysis(LlmAnalysisResult),
    /// A progress update.
    Progress { current: usize, total: usize },
    /// Stream completed.
    Done,
    /// Stream errored.
    Error(String),
}

/// Available LLM providers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Ollama,
    Mock,
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderKind::OpenAi => write!(f, "openai"),
            ProviderKind::Anthropic => write!(f, "anthropic"),
            ProviderKind::Ollama => write!(f, "ollama"),
            ProviderKind::Mock => write!(f, "mock"),
        }
    }
}

impl std::str::FromStr for ProviderKind {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(ProviderKind::OpenAi),
            "anthropic" => Ok(ProviderKind::Anthropic),
            "ollama" => Ok(ProviderKind::Ollama),
            "mock" => Ok(ProviderKind::Mock),
            _ => Err(format!("Unknown provider: {}", s)),
        }
    }
}

/// Common prompt templates for file analysis.
pub mod prompts {
    /// System prompt for file analysis.
    pub const FILE_ANALYSIS_SYSTEM: &str =
        "You are a disk cleanup assistant. Analyze files and determine if they are safe to delete. \
         Be conservative - only recommend deletion when confident. \
         Consider file type, location, age, and content patterns.";

    /// User prompt template for a single file.
    pub fn file_analysis_prompt(entry: &crate::types::FileEntry) -> String {
        format!(
            "File: {}\nType: {}\nSize: {}\nLast accessed: {:?}\nLast modified: {:?}\nLocation: {}\nCategory: {}",
            entry.path.display(),
            entry.file_type,
            entry.human_size(),
            entry.last_accessed,
            entry.last_modified,
            entry.path.parent().map(|p| p.display().to_string()).unwrap_or_default(),
            entry.category,
        )
    }

    /// Batch analysis prompt template.
    pub fn batch_analysis_prompt(entries: &[&crate::types::FileEntry]) -> String {
        let mut prompt = String::from("Analyze the following files and determine if each is safe to delete.\n\n");
        for (i, entry) in entries.iter().enumerate() {
            prompt.push_str(&format!("[{}]\n{}\n\n", i + 1, file_analysis_prompt(entry)));
        }
        prompt.push_str("Respond in JSON array format:\n[\n  {{\n    \"safe_to_delete\": boolean,\n    \"confidence\": 0.0-1.0,\n    \"reason\": \"brief explanation\",\n    \"category\": \"temp|cache|log|orphan|unknown\"\n  }}\n]");
        prompt
    }

    /// System prompt for NL query conversion.
    pub const NL_QUERY_SYSTEM: &str =
        "You are a query parser. Convert natural language file search queries into structured scan configurations.";

    /// NL query user prompt template.
    pub fn nl_query_prompt(query: &str) -> String {
        format!(
            "User query: \"{}\"\n\nAvailable filters: [age, size, type, location, name_pattern, category]\n\
             Respond with a JSON object containing the scan configuration.",
            query
        )
    }
}

/// Parse LLM JSON response into analysis results.
pub fn parse_analysis_response(json_str: &str) -> Result<Vec<LlmAnalysisResult>> {
    // Try parsing as JSON array directly
    if let Ok(results) = serde_json::from_str::<Vec<LlmAnalysisResult>>(json_str) {
        return Ok(results);
    }

    // Try extracting JSON from markdown code blocks
    let cleaned = if let Some(start) = json_str.find('[') {
        &json_str[start..]
    } else {
        json_str
    };
    let cleaned = if let Some(end) = cleaned.rfind(']') {
        &cleaned[..=end]
    } else {
        cleaned
    };

    if let Ok(results) = serde_json::from_str::<Vec<LlmAnalysisResult>>(cleaned) {
        return Ok(results);
    }

    // Last resort: try parsing as a single object
    if let Ok(result) = serde_json::from_str::<LlmAnalysisResult>(cleaned) {
        return Ok(vec![result]);
    }

    Err(crate::VulturiusError::Llm(format!(
        "Failed to parse LLM response as analysis results: {}",
        &json_str[..json_str.len().min(200)]
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_analysis_response_array() {
        let json = r#"[
            {"safe_to_delete": true, "confidence": 0.95, "reason": "Temp file", "category": "temp"},
            {"safe_to_delete": false, "confidence": 0.8, "reason": "Active config", "category": "unknown"}
        ]"#;
        let results = parse_analysis_response(json).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].safe_to_delete);
        assert!(!results[1].safe_to_delete);
    }

    #[test]
    fn test_parse_analysis_response_single() {
        let json = r#"{"safe_to_delete": true, "confidence": 0.9, "reason": "Cache file", "category": "cache"}"#;
        let results = parse_analysis_response(json).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].safe_to_delete);
    }

    #[test]
    fn test_parse_analysis_response_markdown() {
        let json = r#"```json
[{"safe_to_delete": true, "confidence": 0.9, "reason": "Temp", "category": "temp"}]
```"#;
        let results = parse_analysis_response(json).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_parse_analysis_response_invalid() {
        let json = "not json at all";
        let result = parse_analysis_response(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_kind_display() {
        assert_eq!(format!("{}", ProviderKind::OpenAi), "openai");
        assert_eq!(format!("{}", ProviderKind::Anthropic), "anthropic");
        assert_eq!(format!("{}", ProviderKind::Ollama), "ollama");
        assert_eq!(format!("{}", ProviderKind::Mock), "mock");
    }

    #[test]
    fn test_provider_kind_from_str() {
        assert_eq!("openai".parse::<ProviderKind>().unwrap(), ProviderKind::OpenAi);
        assert_eq!("anthropic".parse::<ProviderKind>().unwrap(), ProviderKind::Anthropic);
        assert_eq!("ollama".parse::<ProviderKind>().unwrap(), ProviderKind::Ollama);
        assert!("unknown".parse::<ProviderKind>().is_err());
    }
}
