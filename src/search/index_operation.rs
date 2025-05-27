use std::{sync::{mpsc, Arc, Mutex}, thread, time::Duration};
use anyhow::Result;

use super::{file::File, text_index::TextIndex};

#[derive(Debug)]
pub enum IndexOperation {
  Create(File),
  Modify(File),
  Delete(String),
}

pub struct IndexUpdateQueue {
  sender: mpsc::Sender<IndexOperation>,
  _worker_handle: thread::JoinHandle<()>,
}

impl IndexUpdateQueue {
  pub fn new(text_index: Arc<Mutex<TextIndex>>) -> Self {
    let (sender, receiver) = mpsc::channel::<IndexOperation>();

    let worker_handle = thread::Builder::new()
      .name("index-update-worker".to_string())
      .spawn(move || {
        let mut operations = Vec::new();
        const MAX_BULK_OPERATION_SIZE: usize = 10;
        const WAIT_TIME_FOR_NEXT_UPDATE_IN_BULK: Duration = Duration::from_millis(500);

        loop {
          let timeout = if operations.is_empty() {
            None
          } else {
            Some(WAIT_TIME_FOR_NEXT_UPDATE_IN_BULK)
          };

          match Self::receive_with_timeout(&receiver, timeout) {
            Ok(op) => {
              operations.push(op);

              if operations.len() >= MAX_BULK_OPERATION_SIZE {
                Self::process_queue(&text_index, &mut operations);
              }
            }
            Err(_timeout_or_disconnect) => {
              if !operations.is_empty() {
                Self::process_queue(&text_index, &mut operations);
              };

              if receiver.try_recv().is_err() {
                break; // チャンネルが閉じられた
              }
            }
          }
        }
      })
      .expect("Failed to spawn index update worker");

    IndexUpdateQueue {
      sender,
      _worker_handle: worker_handle,
    }
  }

  fn receive_with_timeout(
    receiver: &mpsc::Receiver<IndexOperation>,
    timeout: Option<Duration>,
  ) -> Result<IndexOperation, mpsc::RecvTimeoutError> {
    match timeout {
      Some(t) => receiver.recv_timeout(t),
      None => receiver
        .recv()
        .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
    }
  }

  fn process_queue(text_index: &Arc<Mutex<TextIndex>>, operations: &mut Vec<IndexOperation>) {
    let result = if let Ok(mut index) = text_index.lock() {
      // バッチ処理でパフォーマンス向上
      let result = operations.into_iter().map(|op| {
        match op {
          IndexOperation::Create(file) => index.add_doc(&file.path, &file.content),
          IndexOperation::Modify(file) => index.replace_doc(&file.path, &file.content),
          IndexOperation::Delete(path) => index.delete_doc(&path),
        }
      }).collect::<Result<(), _>>();
      operations.clear();
      result
    } else {
      tracing::error!("Failed to lock text index for processing operations");
      Err(anyhow::anyhow!("Failed to lock text index"))
    };

    if let Err(e) = result {
      tracing::error!("Failed to process index operations: {}", e);
    }
  }

  pub fn queue_create(&self, file: File) {
    if let Err(e) = self.sender.send(IndexOperation::Create(file)) {
      tracing::error!("Failed to queue create operation: {}", e);
    }
  }

  pub fn queue_modify(&self, file: File) {
    if let Err(e) = self.sender.send(IndexOperation::Modify(file)) {
      tracing::error!("Failed to queue modify operation: {}", e);
    }
  }

  pub fn queue_delete(&self, path: String) {
    if let Err(e) = self.sender.send(IndexOperation::Delete(path)) {
      tracing::error!("Failed to queue delete operation: {}", e);
    }
  }
}
