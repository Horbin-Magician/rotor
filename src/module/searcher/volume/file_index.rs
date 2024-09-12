use std::collections::HashSet;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// use convert_case::{Case, Casing};
use jieba_rs::Jieba; // TODO del
use pinyin::ToPinyin;
use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, INDEXED, STORED};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};

// use crate::utils;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileView {
  pub abs_path: String,
  pub name: String,
  pub created_at: u64,
  pub mod_at: u64,
  pub size: u64,
  pub is_dir: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResult {
  pub file_view: Vec<FileView>,
  pub tokenized: String,
}

pub struct FileIndex {
  pub writer: Arc<Mutex<IndexWriter>>,
  pub reader: IndexReader,
  pub name_field: Field,
  pub path_field: Field,
  pub query_parser: QueryParser,
  pub is_dir_field: Field,
  pub ext_field: Field,
  pub ext_query_parser: QueryParser,
  pub tokenizer: Jieba,
}

impl FileIndex {
  pub fn new(path: &str) -> FileIndex {
    let index_path = std::path::Path::new(path);
    let mut schema_builder = Schema::builder();
    let text_field_indexing = TextFieldIndexing::default().set_index_option(IndexRecordOption::WithFreqs);
    let text_options = TextOptions::default().set_indexing_options(text_field_indexing);
    let name_field = schema_builder.add_text_field("name", text_options.clone());
    let path_field = schema_builder.add_bytes_field("path", INDEXED | STORED); // TODO why not string?
    let is_dir_field = schema_builder.add_bytes_field("is_dir_field", INDEXED); // TODO why not bool?
    let ext_field = schema_builder.add_text_field("ext", text_options);
    let schema = schema_builder.build();

    let index;
    println!("{:?}", index_path);
    if index_path.exists() {
      index = Index::open_in_dir(&index_path).unwrap();
    } else {
      fs::create_dir(&index_path).ok();
      index = Index::create_in_dir(&index_path, schema.clone()).unwrap();
    }

    let writer = Arc::new(Mutex::new(
      index.writer_with_num_threads(2, 140_000_000).unwrap(),
    ));
    let writer_bro = writer.clone();
    std::thread::spawn(move || loop {
      let _ = writer_bro.lock().unwrap().commit();
      std::thread::sleep(Duration::from_secs(5));
    });

    let reader = index
      .reader_builder()
      .reload_policy(ReloadPolicy::OnCommitWithDelay)
      .try_into()
      .unwrap();

    let mut query_parser = QueryParser::for_index(&index, vec![name_field]);
    let ext_query_parser = QueryParser::for_index(&index, vec![ext_field]);
    query_parser.set_field_boost(name_field, 4.0f32);
    let mut jieba = Jieba::new();
    // it's a feature
    jieba.add_word("陈奕迅", None, None);

    FileIndex {
      writer,
      reader,
      name_field,
      path_field,
      ext_field,
      is_dir_field,
      query_parser,
      ext_query_parser,
      tokenizer: jieba,
    }
  }

  pub fn add(&self, name: String, path: String, is_dir: bool, ext: String) {
    let mut ext = ext;
    if is_dir { ext = "".to_string(); }

    let is_dir_bytes = FileIndex::is_dir_bytes(is_dir);
    let _ = self.writer.lock().unwrap().add_document(doc!(
        self.name_field => self.tokenize(name),
        self.path_field=>path.as_bytes(),
        self.is_dir_field=>is_dir_bytes,
        self.ext_field=>ext,
    ));
  }

  pub fn _del(&self, abs_path: String) {
    let term = Term::from_field_bytes(self.path_field, abs_path.as_bytes());
    self.writer.lock().unwrap().delete_term(term);
  }

  pub fn tokenize(&self, hans: String) -> String {
    if hans.is_ascii() { return self.ascii_tokenize(hans);}

    let space = " ";
    let hans = hans.replace("-", space).replace("_", space);

    let words = self.tokenizer.cut(&hans, false);

    let mut token_text: HashSet<String> = vec![].into_iter().collect();

    for word in words {
      let raw = word;
      let mut first = String::new();
      let mut all = String::new();
      token_text.insert(raw.to_string());
      for pinyin in raw.to_pinyin() {
        if let Some(pinyin) = pinyin {
          first = format!("{}{}", first, pinyin.first_letter());
          all = format!("{}{}", all, pinyin.plain());
        }
      }
      if !first.is_empty() {
        token_text.insert(first);
      }
      if !all.is_empty() {
        token_text.insert(all);
      }
    }
    for pinyin in hans.as_str().to_pinyin() {
      if let Some(full) = pinyin {
        token_text.insert(full.first_letter().to_string());
        token_text.insert(full.plain().to_string());
      }
    }
    token_text.insert(hans.clone());
    token_text.into_iter().collect::<Vec<String>>().join(" ")
  }

  fn ascii_tokenize(&self, asc: String) -> String {
    let lowercase = asc.to_lowercase();
    return lowercase;
  }

  pub fn search_tokenize(&self, hans: String) -> String {
    let space = " ";
    let hans = hans
      .replace("-", space)
      .replace("+", space)
      .replace(",", space)
      .replace(".", space)
      .replace(":", space)
      .replace("/", space)
      .replace("\\", space)
      .replace("_", space); // TODO del

    if hans.is_ascii() { return self.ascii_tokenize(hans); }

    let words = self.tokenizer.cut(&hans, false);

    let mut token_text: HashSet<String> = vec![].into_iter().collect();

    for word in words {
      token_text.insert(word.to_string());
    }
    token_text.insert(hans.clone());

    token_text.into_iter().collect::<Vec<String>>().join(" ")
  }

  fn search_paths(&self, kw: String, limit: usize) -> Vec<String> {
    let searcher = self.reader.searcher();

    let query = self
      .query_parser
      .parse_query(&self.search_tokenize(kw))
      .ok()
      .unwrap();
    let top_docs = searcher
      .search(&query, &TopDocs::with_limit(limit))
      .ok()
      .unwrap();

    let mut paths = Vec::new();
    for (_score, doc_address) in top_docs {
      let retrieved_doc: TantivyDocument = searcher.doc(doc_address).unwrap();

      let path = retrieved_doc
        .get_first(self.path_field)
        .unwrap()
        .as_bytes()
        .map(|x| std::str::from_utf8(x))
        .unwrap()
        .unwrap();

      paths.push(path.to_string());
    }
    paths
  }

  pub fn search(&self, kw: String, limit: usize) -> Vec<FileView> {
    let paths = self.search_paths(self.search_tokenize(kw.clone()), limit);
    println!("{:?}", paths);
    let file_views = self.parse_file_views(paths);

    file_views
  }

  // pub fn search_with_filter(
  //   &self,
  //   kw: String,
  //   limit: usize,
  //   is_dir_opt: Option<bool>,
  //   ext_opt: Option<String>,
  // ) -> SearchResult {
  //   let searcher = self.reader.searcher();

  //   let tokens = self.search_tokenize(kw.clone());
  //   let kw_query = self.query_parser.parse_query(&tokens).ok().unwrap();
  //   let mut subqueries = vec![(Occur::Must, kw_query)];

  //   if let Some(is_dir) = is_dir_opt {
  //     let is_dir_bytes = IdxStore::is_dir_bytes(is_dir);
  //     subqueries.push((
  //       Occur::Must,
  //       Box::new(TermQuery::new(
  //         Term::from_field_bytes(self.is_dir_field, is_dir_bytes),
  //         IndexRecordOption::Basic,
  //       )),
  //     ));
  //   }

  //   if let Some(ext) = ext_opt {
  //     let ext_query = self
  //       .ext_query_parser
  //       .parse_query(ext.as_str().to_lowercase().as_str())
  //       .ok()
  //       .unwrap();

  //     subqueries.push((Occur::Must, ext_query));
  //   }

  //   let q = BooleanQuery::new(subqueries);

  //   let top_docs = searcher
  //     .search(&q, &TopDocs::with_limit(limit))
  //     .ok()
  //     .unwrap();

  //   let mut paths = Vec::new();
  //   for (_score, doc_address) in top_docs {
  //     let retrieved_doc = searcher.doc(doc_address).ok().unwrap();

  //     let path = retrieved_doc
  //       .get_first(self.path_field)
  //       .unwrap()
  //       .as_bytes()
  //       .map(|x| std::str::from_utf8(x))
  //       .unwrap()
  //       .unwrap();

  //     paths.push(path.to_string());
  //   }

  //   // if paths.is_empty() {
  //   //   paths = self.suggest_path(kw, limit);
  //   // }
  //   let file_views = self.parse_file_views(paths);

  //   SearchResult {
  //     file_view: file_views,
  //     tokenized: self.search_tokenize(kw),
  //   }
  // }

  fn parse_file_views(&self, paths: Vec<String>) -> Vec<FileView> {
    let mut file_views = Vec::new();

    let mut uniques: HashSet<String> = HashSet::new();
    for path0 in paths {
      let path =  str::replace(&path0, "\\", "/");
      match fs::metadata(path.clone()) {
        Ok(meta) => {
          if !uniques.contains(&path) {
            uniques.insert(path.clone());
          } else {
            continue;
          }
          #[cfg(windows)]
          let size = meta.file_size();
          #[cfg(unix)]
          let size = meta.size();

          file_views.push(FileView {
            abs_path: str::replace(&path, "\\", "/"),
            name: str::replace(&path, "\\", "/").split("/").into_iter().last().unwrap_or("").to_string(),
            created_at: meta
              .created()
              .unwrap_or(SystemTime::now())
              .duration_since(UNIX_EPOCH)
              .unwrap()
              .as_secs(),
            mod_at: meta
              .modified()
              .unwrap_or(SystemTime::now())
              .duration_since(UNIX_EPOCH)
              .unwrap()
              .as_secs(),
            size: size,
            is_dir: meta.is_dir(),
          });
        }
        Err(_) => {
          self._del(path0);
        }
      }
    }

    file_views
  }

  // pub fn suggest(&self, kw: String, limit: usize) -> Vec<FileView> {
  //   let mut paths = self.search_paths(self.search_tokenize(kw.clone()), limit);
  //   if paths.is_empty() {
  //     paths = self.suggest_path(kw, limit);
  //   }
  //   let file_views = paths
  //     .into_iter()
  //     .map(|x| {
  //       return FileView {
  //         abs_path: "".to_string(),
  //         name: utils::path2name(x),
  //         created_at: 0,
  //         mod_at: 0,
  //         size: 0,
  //         is_dir: false,
  //       };
  //     })
  //     .collect::<Vec<FileView>>();
  //   file_views
  // }

  // fn suggest_path(&self, kw: String, limit: usize) -> Vec<String> {
  //   let searcher = self.reader.searcher();
  //   let term = Term::from_field_text(self.name_field, &kw);
  //   let query = FuzzyTermQuery::new_prefix(term, 1, false);
  //   let top_docs = searcher
  //     .search(&query, &TopDocs::with_limit(limit))
  //     .unwrap();
  //   let mut paths = Vec::new();
  //   for (_score, doc_address) in top_docs {
  //     let retrieved_doc = searcher.doc(doc_address).ok().unwrap();

  //     let path = retrieved_doc
  //       .get_first(self.path_field)
  //       .unwrap()
  //       .as_bytes()
  //       .map(|x| std::str::from_utf8(x))
  //       .unwrap()
  //       .unwrap();

  //     paths.push(path.to_string());
  //   }
  //   paths
  // }

  // bool to bytes
  fn is_dir_bytes(is_dir: bool) -> &'static [u8] {
    let is_dir_bytes = if is_dir {
      "1".as_bytes()
    } else {
      "0".as_bytes()
    };
    is_dir_bytes
  }

  // commit
  pub fn commit(&self) {
    let _ = self.writer.lock().unwrap().commit();
  }

  // return the docs count
  pub fn num_docs(&self) -> u64 {
    self.reader.searcher().num_docs()
  }
}