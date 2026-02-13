use log::info;
use rand::Rng;
use std::path::{Path, PathBuf};
use tauri::Emitter;
use thirtyfour::prelude::*;
use tokio::time::{sleep, Duration};

use crate::chromedriver::ensure_chromedriver;
use crate::models::{Product, QueryResult, ShopResults};
use crate::platforms::{ShopeeScraper, TokopediaScraper};

pub async fn scrape_products(
    window: tauri::Window,
    queries: Vec<String>,
    platform: String,
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
    caps.add_chrome_arg(
        "--user-data-dir=C:\\Users\\bagus\\AppData\\Local\\Google\\Chrome\\User Data\\Profile 1",
    )
    .unwrap();
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

    match platform.as_str() {
        "tokopedia" => {
            let results = scrape_tokopedia(&driver, &queries, &window).await?;
            all_results.push(results);
        }
        "shopee" => {
            let results = scrape_shopee(&driver, &queries, &window).await?;
            all_results.push(results);
        }
        "all" => {
            // Scrape both platforms
            let tokopedia_results = scrape_tokopedia(&driver, &queries, &window).await?;
            all_results.push(tokopedia_results);

            let shopee_results = scrape_shopee(&driver, &queries, &window).await?;
            all_results.push(shopee_results);
        }
        _ => return Err("Unsupported platform".to_string()),
    }

    // Clean up
    driver.quit().await.map_err(|e| e.to_string())?;
    child.kill().map_err(|e| e.to_string())?;

    Ok(all_results)
}

async fn scrape_tokopedia(
    driver: &WebDriver,
    queries: &[String],
    window: &tauri::Window,
) -> Result<ShopResults, String> {
    info!("Starting Tokopedia scraping");

    let mut query_results = Vec::new();

    for query in queries {
        info!("Searching for: {}", query);
        window
            .emit(
                "scraping-status",
                format!("Searching Tokopedia for: {}", query),
            )
            .map_err(|e| format!("Failed to emit status: {}", e))?;

        let products = TokopediaScraper::search(driver, query)
            .await
            .map_err(|e| format!("Tokopedia search failed: {}", e))?;
        query_results.push(QueryResult {
            query: query.clone(),
            products,
        });
    }

    Ok(ShopResults {
        shop_name: "Tokopedia".to_string(),
        shop_url: "https://tokopedia.com".to_string(),
        platform: "tokopedia".to_string(),
        results: query_results,
    })
}

async fn scrape_shopee(
    driver: &WebDriver,
    queries: &[String],
    window: &tauri::Window,
) -> Result<ShopResults, String> {
    info!("Starting Shopee scraping");

    let mut query_results = Vec::new();

    for query in queries {
        info!("Searching for: {}", query);
        window
            .emit(
                "scraping-status",
                format!("Searching Shopee for: {}", query),
            )
            .map_err(|e| format!("Failed to emit status: {}", e))?;

        let products = ShopeeScraper::search(driver, query)
            .await
            .map_err(|e| format!("Shopee search failed: {}", e))?;
        query_results.push(QueryResult {
            query: query.clone(),
            products,
        });
    }

    Ok(ShopResults {
        shop_name: "Shopee".to_string(),
        shop_url: "https://shopee.co.id".to_string(),
        platform: "shopee".to_string(),
        results: query_results,
    })
}

// Legacy function - now using TokopediaScraper::search
#[allow(dead_code)]
async fn perform_tokopedia_search(
    _driver: &WebDriver,
    _query: &str,
) -> Result<Vec<Product>, String> {
    Err("Use TokopediaScraper::search instead".to_string())
}

// Legacy function - now using ShopeeScraper::search
#[allow(dead_code)]
async fn perform_shopee_search(_driver: &WebDriver, _query: &str) -> Result<Vec<Product>, String> {
    Err("Use ShopeeScraper::search instead".to_string())
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
    caps.add_chrome_arg(
        "--user-data-dir=C:\\Users\\bagus\\AppData\\Local\\Google\\Chrome\\User Data\\Profile 1",
    )
    .unwrap();
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
