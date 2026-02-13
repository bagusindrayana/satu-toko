use log::info;
use thirtyfour::prelude::*;
use tokio::time::{sleep, Duration};

use crate::models::Product;

pub struct TokopediaScraper;
pub struct ShopeeScraper;

impl TokopediaScraper {
    pub async fn search(driver: &WebDriver, query: &str) -> Result<Vec<Product>, String> {
        info!("Starting Tokopedia search for: {}", query);

        // Navigate to Tokopedia search page
        let search_url = format!(
            "https://tokopedia.com/search?st=product&q={}",
            urlencoding::encode(query)
        );
        driver.goto(&search_url).await.map_err(|e| e.to_string())?;

        // Wait for page to load
        sleep(Duration::from_secs(3)).await;

        let mut products = Vec::new();

        // Scroll to load more products
        for _ in 0..2 {
            driver
                .execute(
                    "window.scrollTo(0, document.body.scrollHeight);",
                    Vec::new(),
                )
                .await
                .map_err(|e| e.to_string())?;
            sleep(Duration::from_secs(2)).await;
        }

        // Find product elements - using updated selectors
        let product_elements = driver
            .find_all(By::Css("div[data-testid='divProductWrapper']"))
            .await
            .map_err(|e| e.to_string())?;

        info!("Found {} products on Tokopedia", product_elements.len());

        for element in product_elements.iter().take(10) {
            let name = match element.find(By::Css("a[role='link'] span")).await {
                Ok(el) => match el.text().await {
                    Ok(text) => text,
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            let price = match element.find(By::Css("span[class*='price']")).await {
                Ok(el) => match el.text().await {
                    Ok(text) => text,
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            let shop = match element.find(By::Css("span[class*='shop-name']")).await {
                Ok(el) => match el.text().await {
                    Ok(text) => text,
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            let location = match element.find(By::Css("span[class*='location']")).await {
                Ok(el) => match el.text().await {
                    Ok(text) => text,
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            let photo = match element.find(By::Css("img")).await {
                Ok(el) => match el.attr("src").await {
                    Ok(Some(src)) => src,
                    Ok(None) => String::new(),
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            let link = match element.find(By::Css("a[role='link']")).await {
                Ok(el) => match el.attr("href").await {
                    Ok(Some(href)) => href,
                    Ok(None) => String::new(),
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            if !name.is_empty() && !price.is_empty() {
                products.push(Product {
                    name,
                    price,
                    shop,
                    location,
                    photo,
                    link: format!("https://tokopedia.com{}", link),
                });
            }
        }

        info!(
            "Successfully scraped {} products from Tokopedia",
            products.len()
        );
        Ok(products)
    }
}

impl ShopeeScraper {
    pub async fn search(driver: &WebDriver, query: &str) -> Result<Vec<Product>, String> {
        info!("Starting Shopee search for: {}", query);

        // Navigate to Shopee search page
        let search_url = format!(
            "https://shopee.co.id/search?keyword={}",
            urlencoding::encode(query)
        );
        driver.goto(&search_url).await.map_err(|e| e.to_string())?;

        // Wait for page to load
        sleep(Duration::from_secs(3)).await;

        let mut products = Vec::new();

        // Scroll to load more products
        for _ in 0..2 {
            driver
                .execute(
                    "window.scrollTo(0, document.body.scrollHeight);",
                    Vec::new(),
                )
                .await
                .map_err(|e| e.to_string())?;
            sleep(Duration::from_secs(2)).await;
        }

        // Find product elements - using updated selectors
        let product_elements = driver
            .find_all(By::Css("div[data-sqe='item']"))
            .await
            .map_err(|e| e.to_string())?;

        info!("Found {} products on Shopee", product_elements.len());

        for element in product_elements.iter().take(10) {
            let name = match element.find(By::Css("div[data-sqe='name']")).await {
                Ok(el) => match el.text().await {
                    Ok(text) => text,
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            let price = match element.find(By::Css("div[data-sqe='price']")).await {
                Ok(el) => match el.text().await {
                    Ok(text) => text,
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            let shop = match element.find(By::Css("div[data-sqe='shopname']")).await {
                Ok(el) => match el.text().await {
                    Ok(text) => text,
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            let location = match element.find(By::Css("div[data-sqe='location']")).await {
                Ok(el) => match el.text().await {
                    Ok(text) => text,
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            let photo = match element.find(By::Css("img")).await {
                Ok(el) => match el.attr("src").await {
                    Ok(Some(src)) => src,
                    Ok(None) => String::new(),
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            let link = match element.find(By::Css("a")).await {
                Ok(el) => match el.attr("href").await {
                    Ok(Some(href)) => href,
                    Ok(None) => String::new(),
                    Err(_) => String::new(),
                },
                Err(_) => String::new(),
            };

            if !name.is_empty() && !price.is_empty() {
                products.push(Product {
                    name,
                    price,
                    shop,
                    location,
                    photo,
                    link: format!("https://shopee.co.id{}", link),
                });
            }
        }

        info!(
            "Successfully scraped {} products from Shopee",
            products.len()
        );
        Ok(products)
    }
}

// Helper function to handle common scraping patterns
pub async fn wait_for_elements(
    driver: &WebDriver,
    selector: &str,
    timeout_secs: u64,
) -> Result<Vec<WebElement>, String> {
    use thirtyfour::By;

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    while start.elapsed() < timeout {
        match driver.find_all(By::Css(selector)).await {
            Ok(elements) if !elements.is_empty() => return Ok(elements),
            _ => {
                sleep(Duration::from_millis(500)).await;
                continue;
            }
        }
    }

    Err(format!(
        "Elements not found within {} seconds",
        timeout_secs
    ))
}

// Helper function to safely extract text from elements
pub async fn safe_extract_text(element: &WebElement, selector: &str) -> String {
    match element.find(By::Css(selector)).await {
        Ok(el) => el.text().await.unwrap_or_default(),
        Err(_) => String::new(),
    }
}

// Helper function to safely extract attribute from elements
pub async fn safe_extract_attr(element: &WebElement, selector: &str, attr: &str) -> String {
    match element.find(By::Css(selector)).await {
        Ok(el) => el.attr(attr).await.unwrap_or_default().unwrap_or_default(),
        Err(_) => String::new(),
    }
}
