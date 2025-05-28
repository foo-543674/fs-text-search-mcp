use anyhow::Result;
use notify_debouncer_full::{
  DebounceEventResult, DebouncedEvent, FileIdMap, new_debouncer,
  notify::{
    EventKind, RecursiveMode, Watcher,
    event::{ModifyKind, RemoveKind, RenameMode},
  },
};
use std::{
  path::{Component, Path, PathBuf},
  sync::mpsc::{Receiver, channel},
  thread,
  time::Duration,
};

use crate::search::file::{FileOperation, FileOperationHandler, FileWatcher};

pub struct NotifyFileWatcher {
  watcher: Option<
    notify_debouncer_full::Debouncer<notify_debouncer_full::notify::RecommendedWatcher, FileIdMap>,
  >,
  stop_tx: Option<std::sync::mpsc::Sender<()>>,
  watch_target: Option<String>,
  thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl NotifyFileWatcher {
  pub fn new() -> Self {
    Self {
      watcher: None,
      stop_tx: None,
      watch_target: None,
      thread_handle: None,
    }
  }
}

impl FileWatcher for NotifyFileWatcher {
  fn watch_directory(&mut self, path: &str, handler: Box<FileOperationHandler>) -> Result<()> {
    let (tx, rx) = channel::<DebounceEventResult>();
    let (stop_tx, stop_rx) = channel::<()>();

    let mut debouncer = new_debouncer(Duration::from_secs(1), None, tx)?;

    debouncer
      .watcher()
      .watch(Path::new(path), RecursiveMode::Recursive)?;

    let thread_handle = thread::Builder::new()
      .name("debounced-file-watcher".to_string())
      .spawn(move || {
        event_loop(rx, stop_rx, handler);
      })?;

    self.watcher = Some(debouncer);
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
        let _ = watcher.watcher().unwatch(Path::new(path));
      }
    }

    Ok(())
  }
}

fn event_loop(
  rx: Receiver<DebounceEventResult>,
  stop_rx: Receiver<()>,
  handler: Box<FileOperationHandler>,
) {
  loop {
    if stop_rx.try_recv().is_ok() {
      break;
    }

    match rx.recv_timeout(Duration::from_millis(100)) {
      Result::Ok(result) => match result {
        Result::Ok(events) => {
          let _ = process_events(events, &handler).map_err(|e| {
            tracing::error!("Error processing file events: {}", e);
            e
          });
        }
        Result::Err(errors) => {
          for error in errors {
            tracing::error!("File watcher error: {}", error);
          }
        }
      },
      Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
        // タイムアウトは正常
      }
      Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
        break;
      }
    }
  }
}

fn process_paths(
  paths: &Vec<PathBuf>,
  f: impl Fn(&Path) -> Result<FileOperation>,
) -> Vec<Result<FileOperation>> {
  paths.into_iter().map(|path| f(&path)).collect()
}

pub fn normalize_notify_path(path: &Path) -> PathBuf {
  /* notify sends path includes unnecessary "./" */
  path
    .components()
    .filter(|component| !matches!(component, Component::CurDir))
    .collect::<PathBuf>()
}

fn process_events(events: Vec<DebouncedEvent>, handler: &Box<FileOperationHandler>) -> Result<()> {
  events
    .into_iter()
    .flat_map(|event| {
      tracing::debug!("Processing event: {:?} for path {:?}", event, event.paths);
      match event.kind {
        EventKind::Create(_) => process_paths(&event.paths, |path: &Path| {
          Ok(FileOperation::FileCreated(
            normalize_notify_path(path).to_string_lossy().to_string(),
          ))
        }),
        EventKind::Modify(modify_kind) => match modify_kind {
          ModifyKind::Data(_) => process_paths(&event.paths, |path: &Path| {
            Ok(FileOperation::FileModified(
              normalize_notify_path(path).to_string_lossy().to_string(),
            ))
          }),
          ModifyKind::Name(RenameMode::Both) => {
            const OLD_PATH_INDEX: usize = 0;
            const NEW_PATH_INDEX: usize = 1;
            let old_path = event.paths.get(OLD_PATH_INDEX);
            let new_path = event.paths.get(NEW_PATH_INDEX);
            if let (Some(old), Some(new)) = (old_path, new_path) {
              if new.is_file() {
                return vec![Ok(FileOperation::FileRenamed {
                  old_path: normalize_notify_path(old).to_string_lossy().to_string(),
                  new_path: normalize_notify_path(new).to_string_lossy().to_string(),
                })];
              } else {
                return vec![Ok(FileOperation::DirectoryRenamed {
                  old_path: normalize_notify_path(old).to_string_lossy().to_string(),
                  new_path: normalize_notify_path(new).to_string_lossy().to_string(),
                })];
              }
            } else {
              vec![Err(anyhow::anyhow!("Rename event missing paths"))]
            }
          }
          _ => vec![], // 他の ModifyKind は無視
        },
        EventKind::Remove(RemoveKind::File) => process_paths(&event.paths, |path: &Path| {
          Ok(FileOperation::FileDeleted(
            normalize_notify_path(path).to_string_lossy().to_string(),
          ))
        }),
        EventKind::Remove(RemoveKind::Folder) => process_paths(&event.paths, |path: &Path| {
          Ok(FileOperation::DirectoryDeleted(
            normalize_notify_path(path).to_string_lossy().to_string(),
          ))
        }),
        _ => vec![], // その他のイベントは無視
      }
    })
    .collect::<Result<Vec<_>>>()
    .and_then(|ops| {
      for op in ops {
        handler(&op)?;
      }
      Ok(())
    })
    .map(|_| ())
}

impl Drop for NotifyFileWatcher {
  fn drop(&mut self) {
    if let Err(e) = self.stop_watching() {
      tracing::error!("Error stopping file watcher in Drop: {}", e);
    }
  }
}
