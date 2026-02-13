use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Product {
    pub name: String,
    pub price: String,
    pub shop: String,
    pub location: String,
    pub photo: String,
    pub link: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct QueryResult {
    pub query: String,
    pub products: Vec<Product>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ShopResults {
    pub shop_name: String,
    pub shop_url: String,
    pub platform: String,
    pub results: Vec<QueryResult>,
}
