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

#[tauri::command]
async fn export_to_excel(results: Vec<ShopResults>) -> Result<String, String> {
    use chrono::Local;
    
    // Create CSV content
    let mut csv_content = String::from("Nama Toko,Platform,URL Toko,Query,Nama Produk,Harga,Lokasi,Link Produk\n");
    
    for shop in results {
        for query_result in shop.results {
            for product in query_result.products {
                let row = format!(
                    "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                    shop.shop_name,
                    if shop.platform == "tokopedia" { "Tokopedia" } else { "Shopee" },
                    shop.shop_url,
                    query_result.query,
                    product.name,
                    product.price,
                    product.location,
                    product.link
                );
                csv_content.push_str(&row);
            }
        }
    }
    
    // Get downloads directory
    let downloads_dir = dirs::download_dir()
        .ok_or("Could not determine downloads directory")?;
    
    // Create filename with timestamp
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let filename = format!("hasil_pencarian_{}.csv", timestamp);
    let file_path = downloads_dir.join(&filename);
    
    // Write file
    fs::write(&file_path, csv_content)
        .map_err(|e| format!("Failed to write CSV file: {}", e))?;
    
    Ok(file_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn create_print_html(results: Vec<ShopResults>) -> Result<String, String> {
    use chrono::Local;
    
    let timestamp = Local::now().format("%d/%m/%Y %H:%M:%S");
    
    let mut html = format!(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Hasil Pencarian - Satu Toko</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', sans-serif;
            padding: 20px;
            color: #333;
        }}
        h1 {{
            font-size: 24px;
            margin-bottom: 10px;
        }}
        .timestamp {{
            color: #666;
            font-size: 14px;
            margin-bottom: 30px;
        }}
        .shop-section {{
            margin-bottom: 30px;
            page-break-inside: avoid;
        }}
        .shop-header {{
            background: #f0f0f0;
            padding: 12px;
            border-left: 4px solid #0078d4;
            margin-bottom: 15px;
        }}
        .shop-name {{
            font-size: 18px;
            font-weight: 600;
            margin: 0 0 5px 0;
        }}
        .shop-url {{
            font-size: 12px;
            color: #0078d4;
            word-break: break-all;
        }}
        .query-section {{
            margin-bottom: 20px;
            padding-left: 15px;
        }}
        .query-title {{
            font-size: 14px;
            font-weight: 600;
            color: #666;
            margin-bottom: 10px;
        }}
        .product-table {{
            width: 100%;
            border-collapse: collapse;
            margin-bottom: 15px;
        }}
        .product-table th {{
            background: #f8f8f8;
            border: 1px solid #ddd;
            padding: 8px;
            text-align: left;
            font-size: 12px;
            font-weight: 600;
        }}
        .product-table td {{
            border: 1px solid #ddd;
            padding: 8px;
            font-size: 12px;
        }}
        .product-name {{
            max-width: 300px;
        }}
        .product-price {{
            color: #107c10;
            font-weight: 500;
            white-space: nowrap;
        }}
        .product-link {{
            color: #0078d4;
            font-size: 11px;
            word-break: break-all;
        }}
        .no-results {{
            color: #999;
            font-style: italic;
            padding: 10px;
        }}
        @media print {{
            body {{ padding: 10px; }}
            .shop-section {{ page-break-inside: avoid; }}
        }}
    </style>
</head>
<body>
    <h1>Hasil Pencarian Produk</h1>
    <div class="timestamp">Dicetak pada: {}</div>
"#, timestamp);

    for shop in results {
        html.push_str(&format!(r#"
    <div class="shop-section">
        <div class="shop-header">
            <div class="shop-name">{} - {}</div>
            <div class="shop-url">{}</div>
        </div>
"#, 
            shop.shop_name,
            if shop.platform == "tokopedia" { "Tokopedia" } else { "Shopee" },
            shop.shop_url
        ));

        for query_result in shop.results {
            html.push_str(&format!(r#"
        <div class="query-section">
            <div class="query-title">Query: "{}" ({} produk)</div>
"#, 
                query_result.query,
                query_result.products.len()
            ));

            if !query_result.products.is_empty() {
                html.push_str(r#"
            <table class="product-table">
                <thead>
                    <tr>
                        <th>No</th>
                        <th>Nama Produk</th>
                        <th>Harga</th>
                        <th>Link</th>
                    </tr>
                </thead>
                <tbody>
"#);

                for (index, product) in query_result.products.iter().enumerate() {
                    html.push_str(&format!(r#"
                    <tr>
                        <td>{}</td>
                        <td class="product-name">{}</td>
                        <td class="product-price">{}</td>
                        <td class="product-link">{}</td>
                    </tr>
"#,
                        index + 1,
                        product.name,
                        product.price,
                        product.link
                    ));
                }

                html.push_str(r#"
                </tbody>
            </table>
"#);
            } else {
                html.push_str(r#"<div class="no-results">Tidak ada produk ditemukan</div>"#);
            }

            html.push_str("        </div>\n");
        }

        html.push_str("    </div>\n");
    }

    html.push_str(r#"
</body>
</html>
"#);

    // Save to temp file
    let temp_dir = std::env::temp_dir();
    let timestamp_file = Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("satu_toko_print_{}.html", timestamp_file);
    let file_path = temp_dir.join(&filename);
    
    fs::write(&file_path, html)
        .map_err(|e| format!("Failed to write HTML file: {}", e))?;
    
    Ok(file_path.to_string_lossy().to_string())
}

#[tauri::command]
fn open_file_with_default_app(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/C", "start", "", &path])
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    
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
            set_chrome_profile_path,
            export_to_excel,
            create_print_html,
            open_file_with_default_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
