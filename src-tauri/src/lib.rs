// Main library file for SatuToko
// This file contains Tauri commands and re-exports from other modules

use tauri_plugin_log::{Target, TargetKind};
use std::fs;

// Import modules
mod chromedriver;
mod models;
mod platforms;
mod scraper;

// Re-export commonly used types
pub use models::{Product, QueryResult, ShopResults};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn ensure_chromedriver() -> Result<String, String> {
    chromedriver::ensure_chromedriver().await
}

#[tauri::command]
async fn scrape_products(
    window: tauri::Window,
    queries: Vec<String>,
    platform: String,
) -> Result<Vec<ShopResults>, String> {
    scraper::scrape_products(window, queries, platform).await
}

#[tauri::command]
async fn get_chrome_and_driver_info() -> Result<(String, String), String> {
    scraper::get_chrome_and_driver_info().await
}

#[tauri::command]
async fn redownload_chromedriver() -> Result<String, String> {
    chromedriver::redownload_chromedriver().await
}

#[tauri::command]
async fn open_chrome_with_driver(url: String) -> Result<String, String> {
    let (driver, mut child) = scraper::open_chrome_with_driver().await?;

    // Navigate to a test page
    // driver
    // .goto(url)
    // .await
    // .map_err(|e| format!("Failed to navigate: {}", e))?;

    // Keep the browser open for a bit
    //tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Clean up
    //driver.quit().await.map_err(|e| e.to_string())?;
    // child.kill().map_err(|e| e.to_string())?;

    // Ok("Browser opened successfully".to_string())

    let result_of_goto = driver.goto(&url).await;

    // 💡 The 'result' variable MUST be a Result<(), String> to be returned.
    let final_result: Result<String, String> = match result_of_goto {
        // SUCCESS: Navigation succeeded. Return Ok(()).
        Ok(()) => {
            let _ = child.kill();
            println!("Success: Navigated to '{}'", &url);
            Ok("Browser opened successfully".to_string())
        }

        // ERROR: Navigation failed. Format the error string and wrap it in Err().
        Err(e) => {
            let _ = child.kill();
            // Use {:?} to format the WebDriverError for logging/debugging
            Err(format!(
                "Navigation Error: Failed to navigate to '{}'. Details: {:?}",
                url, e
            ))
        }
    };

    // // Clean up the driver session
    // let _ = driver.quit().await;

    // Return the final Result<(), String>
    final_result
}

#[tauri::command]
fn get_chrome_profile_path() -> Result<String, String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not determine config directory")?;
    let config_file = config_dir.join("satu-toko").join("chrome_profile.txt");
    
    if config_file.exists() {
        fs::read_to_string(config_file)
            .map_err(|e| format!("Failed to read config: {}", e))
    } else {
        Ok(String::new())
    }
}

#[tauri::command]
fn set_chrome_profile_path(path: String) -> Result<(), String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not determine config directory")?;
    let satu_toko_dir = config_dir.join("satu-toko");
    
    fs::create_dir_all(&satu_toko_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;
    
    let config_file = satu_toko_dir.join("chrome_profile.txt");
    fs::write(config_file, path)
        .map_err(|e| format!("Failed to write config: {}", e))?;
    
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir {
                        file_name: Some("satu-toko".into()),
                    }),
                    Target::new(TargetKind::Webview),
                ])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            greet,
            ensure_chromedriver,
            scrape_products,
            get_chrome_and_driver_info,
            redownload_chromedriver,
            open_chrome_with_driver,
            get_chrome_profile_path,
            set_chrome_profile_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
