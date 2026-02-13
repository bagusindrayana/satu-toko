// Module organization for SatuToko

pub mod chromedriver;
pub mod models;
pub mod platforms;
pub mod scraper;

// Re-export commonly used types for convenience
pub use models::{Product, QueryResult, ShopResults};
