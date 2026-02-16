use log::info;
use rand::Rng;
use std::path::PathBuf;
use std::fs;
use tauri::Emitter;
use thirtyfour::prelude::*;
use tokio::time::{sleep, Duration};

use crate::chromedriver::ensure_chromedriver;
use crate::models::{Product, QueryResult, ShopResults};
// Legacy platform functions - using original logic

// Helper function to get Chrome profile path from config
fn get_chrome_profile_path() -> String {
    let config_dir = match dirs::config_dir() {
        Some(dir) => dir,
        None => return String::new(),
    };
    let config_file = config_dir.join("satu-toko").join("chrome_profile.txt");
    
    if config_file.exists() {
        fs::read_to_string(config_file).unwrap_or_default()
    } else {
        String::new()
    }
}

pub async fn scrape_products(
    window: tauri::Window,
    queries: Vec<String>,
    platform: String,
    limit: usize,
) -> Result<Vec<ShopResults>, String> {
    // Check chromedriver
    let driver_path = ensure_chromedriver().await.map_err(|e| e.to_string())?;

    // Start chromedriver
    let driver_path_buf = PathBuf::from(driver_path);
    let driver_dir = driver_path_buf.parent().ok_or("invalid driver path")?;

    let os = std::env::consts::OS;

    let chromedriver_executable = match os {
        "linux" => driver_dir.join("chromedriver_PATCHED"),
        "macos" => driver_dir.join("chromedriver_PATCHED"),
        "windows" => driver_dir.join("chromedriver_PATCHED.exe"),
        _ => return Err("Unsupported OS!".to_string()),
    };

    let port: usize = rand::thread_rng().gen_range(5000..9000);

    // Launch chromedriver
    let mut child = std::process::Command::new(chromedriver_executable.as_os_str())
        .arg(format!("--port={}", port))
        .current_dir(driver_dir)
        .spawn()
        .map_err(|e| format!("failed to spawn chromedriver: {}", e))?;

    // Wait a bit for chromedriver to start
    sleep(Duration::from_secs(2)).await;

    // Connect to chromedriver
    let mut caps = DesiredCapabilities::chrome();
    
    // Get Chrome profile path from config
    let profile_path = get_chrome_profile_path();
    if !profile_path.is_empty() {
        caps.add_chrome_arg(&format!("--user-data-dir={}", profile_path))
            .unwrap();
    }
    
    caps.set_no_sandbox().unwrap();
    caps.set_disable_dev_shm_usage().unwrap();
    caps.add_chrome_arg("--disable-blink-features=AutomationControlled")
        .unwrap();
    caps.add_chrome_arg("window-size=1920,1080").unwrap();
    caps.add_chrome_arg("user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/102.0.0.0 Safari/537.36").unwrap();
    caps.add_chrome_arg("disable-infobars").unwrap();
    caps.add_chrome_option("excludeSwitches", ["enable-automation"])
        .unwrap();
    let driver = WebDriver::new(&format!("http://localhost:{}", port), caps)
        .await
        .map_err(|e| format!("failed to connect to chromedriver: {}", e))?;

    let mut all_results = Vec::new();

    // Use platform-specific scrapers
    match platform.as_str() {
        "tokopedia" => {
            let results = crate::platforms::TokopediaScraper::scrape(&driver, &queries, &window, limit).await?;
            all_results.extend(results);
        }
        "shopee" => {
            let results = crate::platforms::ShopeeScraper::scrape(&driver, &queries, &window, limit).await?;
            all_results.extend(results);
        }
        "all" => {
            // Scrape both platforms
            let tokopedia_results = crate::platforms::TokopediaScraper::scrape(&driver, &queries, &window, limit).await?;
            all_results.extend(tokopedia_results);

            let shopee_results = crate::platforms::ShopeeScraper::scrape(&driver, &queries, &window, limit).await?;
            all_results.extend(shopee_results);
        }
        _ => return Err("Unsupported platform".to_string()),
    }

    // Clean up
    driver.quit().await.map_err(|e| e.to_string())?;
    child.kill().map_err(|e| e.to_string())?;

    Ok(all_results)
}


// Original Tokopedia scraping logic from lib.rs - Single query version
async fn perform_tokopedia_search(driver: &WebDriver, query: &str) -> Result<Vec<Product>, String> {
    // Navigate to Tokopedia search page
    let search_url = format!(
        "https://www.tokopedia.com/search?q={}",
        urlencoding::encode(query)
    );
    driver.goto(&search_url).await.map_err(|e| e.to_string())?;

    // Wait for page to load
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let mut products = Vec::new();

    // Find product elements - using original selectors from lib.rs
    let product_elements = driver
        .find_all(By::Css("div[data-ssr=\"contentProductsSRPSSR\"] a"))
        .await
        .map_err(|e| e.to_string())?;

    info!("Found {} products on Tokopedia", product_elements.len());

    for element in product_elements.iter().take(20) {
        let link = match element.attr("href").await {
            Ok(opt) => opt.unwrap_or_default(),
            Err(_) => String::new(),
        };

        if link.contains("/product?perpage=") {
            continue;
        }

        let link = if link.starts_with("//") {
            format!("https:{}", link)
        } else if link.starts_with('/') {
            format!("https://www.tokopedia.com{}", link)
        } else {
            link
        };

        let marker = "https://www.tokopedia.com/";
        if let Some(rest) = link.strip_prefix(marker) {
            if let Some((slug, _)) = rest.split_once('/') {
                if !slug.is_empty() {
                    let spans = match element
                        .find_all(By::Css(
                            "div:nth-child(1) > div:nth-child(2) > div:nth-child(1) span",
                        ))
                        .await
                    {
                        Ok(v) => v,
                        Err(_) => Vec::new(),
                    };

                    info!("CHECK SPAN : {}", slug);
                    let mut name = String::new();
                    for s in spans.into_iter().take(20) {
                        let span_text = s.text().await.unwrap_or_default();
                        info!("span: {}", span_text);
                        if !span_text.is_empty() && name.is_empty() {
                            name = span_text;
                        }
                    }

                    let price = match element
                        .find(By::Css("div > div:nth-child(2) > div:nth-child(2)"))
                        .await
                    {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };

                    let shop_display = match element.find(By::Css("span.flip")).await {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };

                    let location = match element
                        .find(By::Css(
                            "div > div:nth-child(2) > div:nth-child(3) span:nth-child(2)",
                        ))
                        .await
                    {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };

                    let photo = match element.find(By::Css("img[alt=\"product-image\"]")).await {
                        Ok(el) => el.attr("src").await.unwrap_or(None).unwrap_or_default(),
                        Err(_) => String::new(),
                    };

                    if !name.is_empty() && !price.is_empty() {
                        products.push(Product {
                            name,
                            price,
                            shop: shop_display.clone(),
                            location,
                            photo,
                            link: link.clone(),
                        });
                    }
                }
            }
        }
    }

    Ok(products)
}

// Original Shopee scraping logic from lib.rs - Single query version
async fn perform_shopee_search(driver: &WebDriver, query: &str) -> Result<Vec<Product>, String> {
    // Navigate to Shopee search page
    let search_url = format!(
        "https://shopee.co.id/search?keyword={}",
        urlencoding::encode(query)
    );
    driver.goto(&search_url).await.map_err(|e| e.to_string())?;

    // Wait for page to load
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let mut products = Vec::new();

    // Find product elements - using original selectors from lib.rs
    let product_elements = driver
        .find_all(By::Css(".shopee-search-item-result__item"))
        .await
        .map_err(|e| e.to_string())?;

    info!("Found {} products on Shopee", product_elements.len());

    for element in product_elements.iter().take(20) {
        let link = match element.attr("href").await {
            Ok(opt) => opt.unwrap_or_default(),
            Err(_) => String::new(),
        };

        if link.starts_with("/") {
            let full_link = format!("https://shopee.co.id{}", link);

            // Extract product name - using original logic
            let name = match element.find(By::Css(".line-clamp-2.break-words")).await {
                Ok(el) => el.text().await.unwrap_or_default(),
                Err(_) => String::new(),
            };

            // Extract price - using original logic with fallback selectors
            let price = {
                let price_element = match element
                    .find(By::Css(
                        "[data-testid=\"a11y-label\"] + div .truncate.text-base\\/5.font-medium",
                    ))
                    .await
                {
                    Ok(el) => el,
                    Err(_) => {
                        // Alternative selector for price
                        match element
                            .find(By::Css(
                                ".text-shopee-primary .truncate.text-base\\/5.font-medium",
                            ))
                            .await
                        {
                            Ok(el) => el,
                            Err(_) => {
                                // Try another approach
                                match element.find(By::Css(".flex-shrink.min-w-0.mr-1.truncate.text-shopee-primary .truncate.text-base\\/5.font-medium")).await {
                                    Ok(el) => el,
                                    Err(_) => {
                                        // If all else fails, return empty string
                                        continue; // Skip this item
                                    }
                                }
                            }
                        }
                    }
                };
                price_element.text().await.unwrap_or_default()
            };

            // Extract shop name - using original logic
            let shop = "Shopee Official Store".to_string();

            // Extract location - using original logic
            let location = match element
                .find(By::Css(
                    ".text-shopee-black54.font-extralight.text-sp10 .align-middle",
                ))
                .await
            {
                Ok(el) => el.text().await.unwrap_or_default(),
                Err(_) => String::new(),
            };

            // Extract photo - using original logic with fallback selectors
            let photo = match element
                .find(By::Css(
                    "img[alt=\"product-image\"], img[src*='simg'], img[src*='shopee']",
                ))
                .await
            {
                Ok(el) => el.attr("src").await.unwrap_or(None).unwrap_or_default(),
                Err(_) => {
                    // Try to get the first image in the product card
                    match element.find(By::Css("img")).await {
                        Ok(el) => el.attr("src").await.unwrap_or(None).unwrap_or_default(),
                        Err(_) => String::new(),
                    }
                }
            };

            if !name.is_empty() && !price.is_empty() {
                products.push(Product {
                    name,
                    price: format!("Rp{}", price), // Add currency prefix as in original
                    shop,
                    location,
                    photo,
                    link: full_link,
                });
            }
        }
    }

    Ok(products)
}

// Function to group products by shop using original logic
// Legacy functions - now using original logic
#[allow(dead_code)]
async fn scrape_tokopedia_with_grouping(
    _driver: &WebDriver,
    _queries: &[String],
    _window: &tauri::Window,
) -> Result<Vec<ShopResults>, String> {
    Err("Use scrape_tokopedia instead".to_string())
}

#[allow(dead_code)]
async fn scrape_shopee_with_grouping(
    _driver: &WebDriver,
    _queries: &[String],
    _window: &tauri::Window,
) -> Result<Vec<ShopResults>, String> {
    Err("Use scrape_shopee instead".to_string())
}

#[allow(dead_code)]
fn group_products_by_shop(
    query_results: Vec<QueryResult>,
    platform: String,
) -> Result<Vec<ShopResults>, String> {
    use std::collections::{HashMap, HashSet};

    let mut shop_slugs: HashSet<String> = HashSet::new();
    let mut shop_names: HashMap<String, String> = HashMap::new();
    let mut first_products_map: HashMap<String, Vec<Product>> = HashMap::new();

    // Process first query results to extract shop information
    if let Some(first_result) = query_results.first() {
        for product in &first_result.products {
            // Extract shop slug from product link
            if let Some(shop_slug) = extract_shop_slug(&product.link, &platform) {
                shop_slugs.insert(shop_slug.clone());
                if !product.shop.is_empty() {
                    shop_names.insert(shop_slug.clone(), product.shop.clone());
                }
                first_products_map
                    .entry(shop_slug)
                    .or_insert_with(Vec::new)
                    .push(product.clone());
            }
        }
    }

    let mut grouped: Vec<ShopResults> = Vec::new();

    for slug in shop_slugs.into_iter() {
        let shop_url = if platform == "shopee" {
            format!("https://shopee.co.id/{}", slug)
        } else {
            format!("https://www.tokopedia.com/{}", slug)
        };
        let shop_display = shop_names
            .get(&slug)
            .cloned()
            .unwrap_or_else(|| slug.clone());

        let mut qresults: Vec<QueryResult> = Vec::new();

        // For each query, create results
        for qresult in &query_results {
            qresults.push(QueryResult {
                query: qresult.query.clone(),
                products: qresult.products.clone(),
            });
        }

        let shop_result = ShopResults {
            shop_name: shop_display.clone(),
            shop_url: shop_url.clone(),
            platform: platform.clone(),
            results: qresults,
        };

        grouped.push(shop_result);
    }

    Ok(grouped)
}

// Helper function to extract shop slug from product link
fn extract_shop_slug(link: &str, platform: &str) -> Option<String> {
    match platform {
        "tokopedia" => {
            // Extract from tokopedia link format
            if link.contains("tokopedia.com") {
                if let Some(rest) = link.strip_prefix("https://www.tokopedia.com/") {
                    if let Some((slug, _)) = rest.split_once('/') {
                        return Some(slug.to_string());
                    }
                }
            }
        }
        "shopee" => {
            // For shopee, we'll use a generic identifier since shop extraction is more complex
            return Some("shopee".to_string());
        }
        _ => {}
    }
    None
}

pub async fn open_chrome_with_driver() -> Result<(WebDriver, std::process::Child), String> {
    let driver_path = ensure_chromedriver().await.map_err(|e| e.to_string())?;

    let driver_path_buf = PathBuf::from(driver_path);
    let driver_dir = driver_path_buf.parent().ok_or("invalid driver path")?;

    let os = std::env::consts::OS;
    let chromedriver_executable = match os {
        "linux" => driver_dir.join("chromedriver_PATCHED"),
        "macos" => driver_dir.join("chromedriver_PATCHED"),
        "windows" => driver_dir.join("chromedriver_PATCHED.exe"),
        _ => return Err("Unsupported OS!".to_string()),
    };

    let port: usize = rand::thread_rng().gen_range(5000..9000);

    // Launch chromedriver
    let child = std::process::Command::new(chromedriver_executable.as_os_str())
        .arg(format!("--port={}", port))
        .current_dir(driver_dir)
        .spawn()
        .map_err(|e| format!("failed to spawn chromedriver: {}", e))?;

    // Wait a bit for chromedriver to start
    sleep(Duration::from_secs(2)).await;

    // Connect to chromedriver
    let mut caps = DesiredCapabilities::chrome();
    
    // Get Chrome profile path from config
    let profile_path = get_chrome_profile_path();
    if !profile_path.is_empty() {
        caps.add_chrome_arg(&format!("--user-data-dir={}", profile_path))
            .unwrap();
    }
    
    caps.set_no_sandbox().unwrap();
    caps.set_disable_dev_shm_usage().unwrap();
    caps.add_chrome_arg("--disable-blink-features=AutomationControlled")
        .unwrap();
    caps.add_chrome_arg("window-size=1920,1080").unwrap();
    caps.add_chrome_arg("user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/102.0.0.0 Safari/537.36").unwrap();
    caps.add_chrome_arg("disable-infobars").unwrap();
    caps.add_chrome_option("excludeSwitches", ["enable-automation"])
        .unwrap();
    let driver = WebDriver::new(&format!("http://localhost:{}", port), caps)
        .await
        .map_err(|e| format!("failed to connect to chromedriver: {}", e))?;

    Ok((driver, child))
}

pub async fn get_chrome_and_driver_info() -> Result<(String, String), String> {
    let chrome_version = crate::chromedriver::get_chrome_version()
        .map_err(|e| format!("Failed to get Chrome version: {}", e))?;

    let driver_path = ensure_chromedriver()
        .await
        .map_err(|e| format!("Failed to ensure chromedriver: {}", e))?;

    println!("Driver Path : {}", driver_path);

    // Get driver version using the path we just ensured
    let driver_version = {
        use std::process::Command;
        let output = Command::new(&driver_path)
            .arg("--version")
            .output()
            .map_err(|e| format!("Failed to execute chromedriver: {}", e))?;

        let version_output = String::from_utf8_lossy(&output.stdout);
        version_output
            .split_whitespace()
            .nth(1)
            .ok_or("Could not parse chromedriver version")?
            .to_string()
    };

    Ok((chrome_version, driver_version))
}
