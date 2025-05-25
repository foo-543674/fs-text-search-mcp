use anyhow::Error;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexWriter, ReloadPolicy, doc};

use super::file::FileLoader;

const SEARCH_FILE_LIMIT: usize = 10;

pub fn search_in_dir(
  file_loader: &dyn FileLoader,
  path: &str,
) -> impl Fn(&str) -> Result<Vec<String>, Error> {
  let mut schema_builder = Schema::builder();
  let file_path_field = schema_builder.add_text_field("file_path", TEXT | STORED);
  let content_field = schema_builder.add_text_field("content", TEXT);
  let schema = schema_builder.build();

  move |keyword: &str| {
    let index = Index::create_in_ram(schema.clone());
    let mut index_writer: IndexWriter = index.writer(50_000_000)?;
    let _ = file_loader.load_directory(path).map(|load_result| {
      load_result.map(|file| {
        index_writer.add_document(doc!(
          file_path_field => file.path.to_string(),
          content_field => file.content,
        ))
      })?
    });
    index_writer.commit()?;

    let reader = index
      .reader_builder()
      .reload_policy(ReloadPolicy::OnCommitWithDelay)
      .try_into()?;
    let searcher = reader.searcher();
    let query_parser = QueryParser::for_index(&index, vec![content_field]);
    let query = query_parser.parse_query(keyword)?;
    let top_docs = searcher.search(&query, &TopDocs::with_limit(SEARCH_FILE_LIMIT))?;

    let results = top_docs
      .iter()
      .map(|(_score, doc_address)| {
        searcher
          .doc(doc_address.clone())
          .map(|doc: TantivyDocument| doc.to_json(&schema))
      })
      .flatten()
      .collect::<Vec<String>>();

    Ok(results)
  }
}
