//! Limits shared by server and desktop provider callers.
pub fn embedding_batch_limit(model: &str) -> usize {
    if model.trim() == "text-embedding-v4" {
        10
    } else {
        64
    }
}
