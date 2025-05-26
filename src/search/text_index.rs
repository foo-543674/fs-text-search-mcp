use anyhow::Error;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexWriter, ReloadPolicy, doc};

use super::file::FileLoader;

const SEARCH_FILE_LIMIT: usize = 10;

pub struct TextIndex {
  index: Index,
  file_path_field: Field,
  content_field: Field,
  schema: Schema,
  writer: IndexWriter,
  pending_operations: usize,
}

impl TextIndex {
  pub fn new() -> Result<Self, Error> {
    let mut schema_builder = Schema::builder();
    let file_path_field = schema_builder.add_text_field("file_path", TEXT | STORED);
    let content_field = schema_builder.add_text_field("content", TEXT);
    let schema = schema_builder.build();

    let index = Index::create_in_ram(schema.clone());
    let index_writer = index.writer(50_000_000)?;

    Ok(TextIndex {
      index,
      file_path_field,
      content_field,
      schema,
      writer: index_writer,
      pending_operations: 0,
    })
  }

  pub fn initialize_index(
    &mut self,
    file_loader: &dyn FileLoader,
    dir_path: &str,
  ) -> Result<(), Error> {
    let loaded_results = file_loader.load_directory(dir_path);
    for result in loaded_results {
      match result {
        Ok(file) => {
          self.add_doc(&file.path, &file.content)?;
        }
        Err(e) => {
          tracing::error!("Failed to load file: {}", e);
        }
      }
    }
    self.commit()
  }

  pub fn add_doc(&mut self, file_path: &str, content: &str) -> Result<(), Error> {
    self.writer.add_document(doc!(
      self.file_path_field => file_path,
      self.content_field => content,
    ))?;
    self.pending_operations += 1;
    Ok(())
  }

  pub fn replace_doc(&mut self, file_path: &str, new_content: &str) -> Result<(), Error> {
    let term = Term::from_field_text(self.file_path_field, file_path);
    self.writer.delete_term(term);

    self.writer.add_document(doc!(
      self.file_path_field => file_path,
      self.content_field => new_content,
    ))?;
    self.pending_operations += 1;
    Ok(())
  }

  pub fn delete_doc(&mut self, file_path: &str) -> Result<(), Error> {
    let term = Term::from_field_text(self.file_path_field, file_path);
    self.writer.delete_term(term);
    self.pending_operations += 1;
    Ok(())
  }

  pub fn commit(&mut self) -> Result<(), Error> {
    self.writer.commit()?;
    self.pending_operations = 0;
    Ok(())
  }

  pub fn search(&self, keyword: &str) -> Result<Vec<String>, Error> {
    let reader = self
      .index
      .reader_builder()
      .reload_policy(ReloadPolicy::OnCommitWithDelay)
      .try_into()?;
    let searcher = reader.searcher();
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
