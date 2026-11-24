//! News retrieval from Qdrant. Everything above this crate depends on the
//! `NewsRepository` trait, not on Qdrant.

mod news_article;
mod news_error;
mod news_query;
mod news_repository;
mod qdrant;

pub use news_article::NewsArticle;
pub use news_error::NewsError;
pub use news_query::NewsQuery;
pub use news_repository::NewsRepository;
pub use qdrant::{QdrantConfig, QdrantNewsRepository};
