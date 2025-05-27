use anyhow::{Context, Result};
use notify::{Event, RecursiveMode, Watcher, event::*, recommended_watcher};
use std::{
  path::Path,
  sync::{
    Arc,
    mpsc::{Receiver, channel},
  },
  thread,
  time::Duration,
};

use crate::search::file::File;

use super::{file_filter::FileFilter, read_file::path_to_file};

pub type FileCreatedCallback = dyn Fn(&File) + Send + Sync;
pub type FileModifiedCallback = dyn Fn(&File) + Send + Sync;
pub type FileDeletedCallback = dyn Fn(&str) + Send + Sync;

pub trait FileWatcher {
  fn watch_directory(
    &mut self,
    path: &str,
    created: Box<FileCreatedCallback>,
    modified: Box<FileModifiedCallback>,
    deleted: Box<FileDeletedCallback>,
  ) -> Result<()>;
  fn stop_watching(&mut self) -> Result<()>;
}

pub struct NotifyFileWatcher {
  watcher: Option<notify::RecommendedWatcher>,
  stop_tx: Option<std::sync::mpsc::Sender<()>>,
  watch_target: Option<String>,
  thread_handle: Option<std::thread::JoinHandle<()>>,
  target_filter: Arc<dyn FileFilter + Send + Sync>,
}

fn event_loop(
  rx: Receiver<notify::Result<Event>>,
  stop_rx: Receiver<()>,
  target_filter: Arc<dyn FileFilter + Send + Sync>,
  created: Box<FileCreatedCallback>,
  modified: Box<FileModifiedCallback>,
  deleted: Box<FileDeletedCallback>,
) {
  loop {
    if stop_rx.try_recv().is_ok() {
      break;
    }

    match rx.recv_timeout(Duration::from_millis(100)) {
      Ok(Ok(event)) => {
        tracing::debug!("Received event: {:?}", event);
        let _ = process_event(event, &target_filter, &created, &modified, &deleted)
          .map_err(|e| {
            tracing::error!("Error processing event: {}", e);
          });
      }
      Ok(Err(e)) => {
        tracing::error!("Error receiving event: {}", e);
      }
      Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
        continue;
      }
      Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
        tracing::error!("Watcher channel disconnected");
        break;
      }
    }
  }
}

fn process_event(
  event: Event,
  target_filter: &Arc<dyn FileFilter + Send + Sync>,
  created: &FileCreatedCallback,
  modified: &FileModifiedCallback,
  deleted: &FileDeletedCallback,
) -> Result<()> {
  match event.kind {
    EventKind::Create(CreateKind::File) | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
      for path in event.paths {
        if !target_filter.is_target(&path) {
          continue;
        }
        if let Some(_) = path.to_str() {
          let file = path_to_file(path.as_path())?;
          created(&file);
        }
      }
    }
    EventKind::Modify(ModifyKind::Data(DataChange::Content)) => {
      for path in event.paths {
        if !target_filter.is_target(&path) {
          continue;
        }
        if let Some(_) = path.to_str() {
          let file = path_to_file(path.as_path())?;
          modified(&file);
        }
      }
    }
    EventKind::Remove(RemoveKind::File) | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
      for path in event.paths {
        if !target_filter.is_target(&path) {
          continue;
        }
        if let Some(path_str) = path.to_str() {
          deleted(&path_str);
        }
      }
    }
    _ => {} // Ignore other event kinds,
  }
  Ok(())
}

impl NotifyFileWatcher {
  pub fn new(target_filter: Arc<dyn FileFilter + Send + Sync>) -> Self {
    NotifyFileWatcher {
      watcher: None,
      stop_tx: None,
      watch_target: None,
      thread_handle: None,
      target_filter,
    }
  }
}

impl FileWatcher for NotifyFileWatcher {
  fn watch_directory(
    &mut self,
    path: &str,
    created: Box<FileCreatedCallback>,
    modified: Box<FileModifiedCallback>,
    deleted: Box<FileDeletedCallback>,
  ) -> Result<()> {
    let (tx, rx) = channel::<notify::Result<Event>>();
    let (stop_tx, stop_rx) = channel::<()>();
    let mut watcher = recommended_watcher(tx)?;
    watcher.watch(Path::new(path), RecursiveMode::Recursive)?;

    let target_filter = self.target_filter.clone();
    let thread_handle = thread::Builder::new()
      .name("file-watcher".to_string())
      .spawn(move || {
        event_loop(rx, stop_rx, target_filter, created, modified, deleted);
      })
      .context("Failed to spawn file watcher thread")?;

    self.watcher = Some(watcher);
    self.stop_tx = Some(stop_tx);
    self.watch_target = Some(path.to_string());
    self.thread_handle = Some(thread_handle);

    Ok(())
  }

  fn stop_watching(&mut self) -> Result<()> {
    if let Some(stop_tx) = self.stop_tx.take() {
      let _ = stop_tx.send(());
    }

    if let Some(handle) = self.thread_handle.take() {
      handle
        .join()
        .map_err(|_| anyhow::anyhow!("Failed to join watcher thread"))?;
    }

    if let Some(mut watcher) = self.watcher.take() {
      if let Some(path) = &self.watch_target {
        let _ = watcher.unwatch(Path::new(path));
      }
    }

    Ok(())
  }
}

impl Drop for NotifyFileWatcher {
  fn drop(&mut self) {
    self.stop_watching().unwrap_or_else(|e| {
      tracing::error!("Failed to stop file watcher: {}", e);
    });
  }
}
