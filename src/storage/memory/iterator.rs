use crate::array::{DataChunk, DataChunkRef};
use crate::storage::{StorageResult, TxnIterator};
use async_trait::async_trait;
use itertools::Itertools;

/// An iterator over all data in a transaction.
///
/// TODO: Lifetime of the iterator should be bound to the transaction.
/// When the transaction end, accessing items inside iterator is UB.
/// To achieve this, we must enable GAT.
pub struct InMemoryTxnIterator {
    chunks: Vec<DataChunkRef>,
    col_idx: Vec<u32>,
    cnt: usize,
}

impl InMemoryTxnIterator {
    pub(super) fn new(chunks: Vec<DataChunkRef>, col_idx: Vec<u32>) -> Self {
        Self {
            chunks,
            col_idx,
            cnt: 0,
        }
    }
}

#[async_trait]
impl TxnIterator for InMemoryTxnIterator {
    async fn next_batch(&mut self) -> StorageResult<Option<DataChunk>> {
        if self.cnt >= self.chunks.len() {
            Ok(None)
        } else {
            let selected_chunk = &self.chunks[self.cnt];
            // TODO(chi): DataChunk should store Arc to array, so as to reduce costly clones.
            let arrays = self
                .col_idx
                .iter()
                .map(|idx| selected_chunk.array_at(*idx as usize).clone())
                .collect_vec();

            let chunk = DataChunk::builder()
                .cardinality(selected_chunk.cardinality())
                .arrays(arrays.into())
                .build();
            self.cnt += 1;

            Ok(Some(chunk))
        }
    }
}