use anyhow::Error;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::{schema::*, IndexReader};
use tantivy::{Index, IndexWriter, ReloadPolicy, TantivyDocument, Term, doc};

use super::file::File;

const SEARCH_FILE_LIMIT: usize = 10;

pub struct TextIndex {
  index: Index,
  file_path_field: Field,
  content_field: Field,
  schema: Schema,
  writer: IndexWriter,
  reader: IndexReader,
  pending_operations: usize,
}

impl TextIndex {
  pub fn new() -> Result<Self, Error> {
    let mut schema_builder = Schema::builder();
    let file_path_field = schema_builder.add_text_field("file_path", STRING | STORED);
    let content_field = schema_builder.add_text_field("content", TEXT);
    let schema = schema_builder.build();

    let index = Index::create_in_ram(schema.clone());
    let index_writer = index.writer(50_000_000)?;

    let index_reader = index
      .reader_builder()
      .reload_policy(ReloadPolicy::OnCommitWithDelay)
      .try_into()?;

    Ok(TextIndex {
      index,
      file_path_field,
      content_field,
      schema,
      writer: index_writer,
      reader: index_reader,
      pending_operations: 0,
    })
  }

  pub fn new_with_directory<P: AsRef<Path>>(index_dir: P) -> Result<Self, Error> {
    let mut schema_builder = Schema::builder();
    let file_path_field = schema_builder.add_text_field("file_path", STRING | STORED);
    let content_field = schema_builder.add_text_field("content", TEXT);
    let schema = schema_builder.build();

    std::fs::create_dir_all(&index_dir)?;

    let index = if index_dir.as_ref().join("meta.json").exists() {
      tracing::info!("Opening existing index at {:?}", index_dir.as_ref());
      Index::open_in_dir(&index_dir)?
    } else {
      tracing::info!("Creating new index at {:?}", index_dir.as_ref());
      Index::create_in_dir(&index_dir, schema.clone())?
    };

    let index_writer = index.writer(50_000_000)?;

    let index_reader = index
      .reader_builder()
      .reload_policy(ReloadPolicy::OnCommitWithDelay)
      .try_into()?;

    Ok(TextIndex {
      index,
      file_path_field,
      content_field,
      schema,
      writer: index_writer,
      reader: index_reader,
      pending_operations: 0,
    })
  }

  pub fn add_doc(&mut self, file: &File) -> Result<(), Error> {
    self.writer.add_document(doc!(
      self.file_path_field => file.path,
      self.content_field => file.content,
    ))?;
    self.pending_operations += 1;
    tracing::debug!("Added document for file: {}", file.path);
    Ok(())
  }

  pub fn replace_doc(&mut self, file: &File) -> Result<(), Error> {
    let term = Term::from_field_text(self.file_path_field, &file.path);
    self.writer.delete_term(term);

    self.writer.add_document(doc!(
      self.file_path_field => file.path,
      self.content_field => file.content,
    ))?;
    self.pending_operations += 1;
    tracing::debug!("Replaced document for file: {}", file.path);
    Ok(())
  }

  pub fn delete_doc(&mut self, file_path: &str) -> Result<(), Error> {
    let term = Term::from_field_text(self.file_path_field, file_path);
    self.writer.delete_term(term);
    self.pending_operations += 1;
    tracing::debug!("Deleted document for file: {}", file_path);
    Ok(())
  }

  pub fn delete_docs_by_path_prefix(&mut self, path_prefix: &str) -> Result<usize, Error> {
    let reader = self.index.reader()?;
    let searcher = reader.searcher();

    let mut deleted_count = 0;

    for segment_reader in searcher.segment_readers() {
      let inverted_index = segment_reader.inverted_index(self.file_path_field)?;
      let term_dict = inverted_index.terms();

      let mut term_stream = term_dict.stream()?;

      while term_stream.advance() {
        let term_bytes = term_stream.key();
        if let Ok(term_str) = std::str::from_utf8(term_bytes) {
          if term_str.starts_with(path_prefix) {
            let term = Term::from_field_text(self.file_path_field, term_str);
            self.writer.delete_term(term);
            deleted_count += 1;
            tracing::debug!("Deleted document for file: {}", term_str);
          }
        }
      }
    }

    if deleted_count > 0 {
      self.pending_operations += deleted_count;
    }

    Ok(deleted_count)
  }

  pub fn commit(&mut self) -> Result<(), Error> {
    if self.pending_operations > 0 {
      self.writer.commit()?;
      self.pending_operations = 0;
      self.reader.reload()?;
    }
    Ok(())
  }

  pub fn get_pending_operations(&self) -> usize {
    self.pending_operations
  }

  pub fn search(&self, keyword: &str) -> Result<Vec<String>, Error> {
    let searcher = self.reader.searcher();
    let query_parser = QueryParser::for_index(&self.index, vec![self.content_field]);
    let query = query_parser.parse_query(keyword)?;
    let top_docs = searcher.search(&query, &TopDocs::with_limit(SEARCH_FILE_LIMIT))?;

    let results = top_docs
      .iter()
      .map(|(_score, doc_address)| {
        searcher
          .doc(*doc_address)
          .map(|doc: TantivyDocument| doc.to_json(&self.schema))
      })
      .collect::<Result<Vec<String>, _>>()?;
    Ok(results)
  }
}

impl Drop for TextIndex {
  fn drop(&mut self) {
    if self.pending_operations > 0 {
      if let Err(e) = self.commit() {
        tracing::error!("Failed to commit pending operations: {}", e);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn text_index_should_able_to_search_and_get_file_name_by_keyword() {
    use super::*;
    use crate::search::file::File;

    let mut index = TextIndex::new().unwrap();
    let file = File {
      path: "test.txt".to_string(),
      content: "This is a test file for indexing.".to_string(),
    };
    index.add_doc(&file).unwrap();
    index.commit().unwrap();
    let results = index.search("test").unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].contains("test.txt"));
  }

  #[test]
  fn text_index_should_able_to_replace_file_content() {
    use super::*;
    use crate::search::file::File;

    let mut index = TextIndex::new().unwrap();
    let file = File {
      path: "test.txt".to_string(),
      content: "This is a test file for indexing.".to_string(),
    };
    index.add_doc(&file).unwrap();
    index.commit().unwrap();
    let updated_file = File {
      path: "test.txt".to_string(),
      content: "This is an updated test file for indexing.".to_string(),
    };
    index.replace_doc(&updated_file).unwrap();
    index.commit().unwrap();
    let results = index.search("updated").unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].contains("test.txt"));
    let results = index.search("test").unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].contains("test.txt"));
  }

  #[test]
  fn text_index_should_able_to_delete_file() {
    use super::*;
    use crate::search::file::File;

    let mut index = TextIndex::new().unwrap();
    let file = File {
      path: "test.txt".to_string(),
      content: "This is a test file for indexing.".to_string(),
    };
    index.add_doc(&file).unwrap();
    index.commit().unwrap();
    index.delete_doc("test.txt").unwrap();
    index.commit().unwrap();
    let results = index.search("test").unwrap();
    assert_eq!(results.len(), 0);
  }

  #[test]
  fn text_index_should_able_to_delete_files_by_path_prefix() {
    use super::*;
    use crate::search::file::File;

    let mut index = TextIndex::new().unwrap();
    let file1 = File {
      path: "/foo/test1.txt".to_string(),
      content: "This is a test file 1 for indexing.".to_string(),
    };
    let file2 = File {
      path: "/foo/test2.txt".to_string(),
      content: "This is a test file 2 for indexing.".to_string(),
    };
    index.add_doc(&file1).unwrap();
    index.add_doc(&file2).unwrap();
    index.commit().unwrap();
    let deleted_count = index.delete_docs_by_path_prefix("/foo").unwrap();
    index.commit().unwrap();
    assert_eq!(deleted_count, 2);
    let results = index.search("test").unwrap();
    assert_eq!(results.len(), 0);
  }
}