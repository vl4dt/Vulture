//! Streaming response support for real-time LLM analysis.

use tokio::sync::mpsc;

use super::provider::StreamChunk;
use crate::Result;

/// A streaming analysis session.
pub struct StreamingAnalysis {
    /// Receiver for stream chunks.
    rx: mpsc::Receiver<Result<StreamChunk>>,
}

impl StreamingAnalysis {
    /// Create a new streaming analysis from a channel.
    pub fn new(rx: mpsc::Receiver<Result<StreamChunk>>) -> Self {
        Self { rx }
    }

    /// Collect all chunks into a vector.
    pub async fn collect(mut self) -> Vec<Result<StreamChunk>> {
        let mut results = Vec::new();
        while let Some(chunk) = self.rx.recv().await {
            results.push(chunk);
        }
        results
    }

    /// Get only the analysis results (filtering out text/progress).
    pub async fn results(mut self) -> Vec<super::provider::LlmAnalysisResult> {
        let mut results = Vec::new();
        while let Some(chunk) = self.rx.recv().await {
            if let Ok(StreamChunk::Analysis(analysis)) = chunk {
                results.push(analysis);
            }
        }
        results
    }

    /// Iterate over chunks directly.
    pub async fn for_each_chunk<F>(&mut self, mut callback: F)
    where
        F: FnMut(Result<StreamChunk>),
    {
        while let Some(chunk) = self.rx.recv().await {
            callback(chunk);
        }
    }
}

/// Progress tracker for streaming analysis.
#[derive(Debug, Clone)]
pub struct AnalysisProgress {
    /// Current file being analyzed.
    pub current: usize,
    /// Total files to analyze.
    pub total: usize,
    /// Results so far.
    pub results_count: usize,
    /// Percentage complete.
    pub percent: f32,
}

impl AnalysisProgress {
    /// Create from a progress chunk.
    pub fn from_chunk(chunk: &StreamChunk, results_count: usize) -> Self {
        match chunk {
            StreamChunk::Progress { current, total } => Self {
                current: *current,
                total: *total,
                results_count,
                percent: if *total > 0 {
                    (*current as f32 / *total as f32) * 100.0
                } else {
                    0.0
                },
            },
            _ => Self {
                current: 0,
                total: 0,
                results_count,
                percent: 0.0,
            },
        }
    }
}

/// Run a streaming analysis and forward chunks to a callback.
pub async fn run_streaming<F>(mut streaming: StreamingAnalysis, mut callback: F) -> Vec<super::provider::LlmAnalysisResult>
where
    F: FnMut(Result<StreamChunk>),
{
    let mut results = Vec::new();

    while let Some(chunk) = streaming.rx.recv().await {
        match &chunk {
            Ok(StreamChunk::Analysis(analysis)) => {
                results.push(analysis.clone());
            }
            Ok(StreamChunk::Error(e)) => {
                tracing::warn!("Streaming analysis error: {}", e);
            }
            Ok(StreamChunk::Done) => {
                tracing::info!("Streaming analysis complete: {} results", results.len());
                break;
            }
            _ => {}
        }
        callback(chunk);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::provider::LlmAnalysisResult;

    #[test]
    fn test_analysis_progress() {
        let chunk = StreamChunk::Progress {
            current: 5,
            total: 10,
        };
        let progress = AnalysisProgress::from_chunk(&chunk, 3);
        assert_eq!(progress.current, 5);
        assert_eq!(progress.total, 10);
        assert_eq!(progress.results_count, 3);
        assert!((progress.percent - 50.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_streaming_analysis_collect() {
        let (tx, rx) = mpsc::channel(10);

        tx.send(Ok(StreamChunk::Progress {
            current: 0,
            total: 2,
        }))
        .await
        .unwrap();
        tx.send(Ok(StreamChunk::Text("analyzing...".to_string())))
            .await
            .unwrap();
        tx.send(Ok(StreamChunk::Analysis(LlmAnalysisResult {
            safe_to_delete: true,
            confidence: 0.9,
            reason: "temp file".to_string(),
            category: "temp".to_string(),
        })))
        .await
        .unwrap();
        tx.send(Ok(StreamChunk::Done))
            .await
            .unwrap();
        drop(tx);

        let streaming = StreamingAnalysis::new(rx);
        let chunks = streaming.collect().await;
        assert_eq!(chunks.len(), 4);
    }

    #[tokio::test]
    async fn test_streaming_analysis_results() {
        let (tx, rx) = mpsc::channel(10);

        tx.send(Ok(StreamChunk::Progress {
            current: 0,
            total: 1,
        }))
        .await
        .unwrap();
        tx.send(Ok(StreamChunk::Analysis(LlmAnalysisResult {
            safe_to_delete: true,
            confidence: 0.95,
            reason: "cache file".to_string(),
            category: "cache".to_string(),
        })))
        .await
        .unwrap();
        tx.send(Ok(StreamChunk::Analysis(LlmAnalysisResult {
            safe_to_delete: false,
            confidence: 0.8,
            reason: "important".to_string(),
            category: "unknown".to_string(),
        })))
        .await
        .unwrap();
        tx.send(Ok(StreamChunk::Done))
            .await
            .unwrap();
        drop(tx);

        let streaming = StreamingAnalysis::new(rx);
        let results = streaming.results().await;
        assert_eq!(results.len(), 2);
        assert!(results[0].safe_to_delete);
        assert!(!results[1].safe_to_delete);
    }

    #[tokio::test]
    async fn test_run_streaming_callback() {
        let (tx, rx) = mpsc::channel(10);

        tx.send(Ok(StreamChunk::Analysis(LlmAnalysisResult {
            safe_to_delete: true,
            confidence: 0.9,
            reason: "test".to_string(),
            category: "temp".to_string(),
        })))
        .await
        .unwrap();
        tx.send(Ok(StreamChunk::Progress { current: 1, total: 1 }))
            .await
            .unwrap();
        tx.send(Ok(StreamChunk::Done))
            .await
            .unwrap();
        drop(tx);

        let streaming = StreamingAnalysis::new(rx);
        let callback_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let callback_count_clone = callback_count.clone();
        let results = run_streaming(streaming, move |_chunk| {
            callback_count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        })
        .await;

        assert_eq!(results.len(), 1);
        assert!(callback_count.load(std::sync::atomic::Ordering::Relaxed) >= 2);
    }
}
