// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri_plugin_log::{Target, TargetKind};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Serialize, Deserialize)]
pub struct Product {
    pub name: String,
    pub price: String,
    pub shop: String,
    pub location: String,
    pub photo: String,
    pub link: String,
}

// Note: these functions perform best-effort tasks. In production, more error handling and security checks are required.

#[tauri::command]
async fn ensure_chromedriver() -> Result<String, String> {
    use reqwest::Client;
    use std::env;
    use std::fs;
    use std::io::Cursor;
    use zip::ZipArchive;

    // Determine local app data path
    let local_app_data = env::var("LOCALAPPDATA").map_err(|e| e.to_string())?;
    let driver_dir = PathBuf::from(local_app_data)
        .join("satu-toko")
        .join("chromedriver");
    fs::create_dir_all(&driver_dir).map_err(|e| e.to_string())?;

    // 1) Find installed chrome version (Windows) by reading registry is complex; try using "chrome --version" fallback
    let mut chrome_version = get_chrome_version().map_err(|e| e.to_string())?;
    info!("Versi Chrome terdeteksi : {}", chrome_version);

    let parts: Vec<&str> = chrome_version.split('.').collect();

    // Build a safe major.minor prefix (use up to first two segments). If none found, keep original value.
    let prefix = parts.iter().take(2).map(|s| s.to_string()).collect::<Vec<_>>().join(".");
    if !prefix.is_empty() {
        chrome_version = prefix;
    }

    // 2) Fetch chrome-for-testing JSON
    let client = Client::new();
    let json_url = "https://googlechromelabs.github.io/chrome-for-testing/known-good-versions-with-downloads.json";
    let resp = client
        .get(json_url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;

    // Find matching version entry
    let matching = v["versions"]
        .as_array()
        .ok_or("versions not found in JSON")?
        .iter()
        .find(|entry| {
            let ver = entry["version"].as_str().unwrap_or("");
            ver.starts_with(&chrome_version)
        });

    let chosen = if let Some(m) = matching {
        m
    } else {
        v["versions"]
            .as_array()
            .and_then(|a| a.first())
            .ok_or("no versions in JSON")?
    };

    // Find platform download for windows (windows-x64) and platform name
    let downloads = chosen["downloads"]["chromedriver"]
        .as_array()
        .ok_or("no chromedriver downloads")?;
    // prefer windows-x64
    let mut url: Option<String> = None;
    for d in downloads {
        if let Some(platform) = d["platform"].as_str() {
            if platform.contains("win64") {
                url = d["url"].as_str().map(|s| s.to_string());
                break;
            }
        }
    }
    let url = url.ok_or("no windows chromedriver in JSON")?;

    // Download zip
    let bytes = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    // find chromedriver.exe inside zip
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_string();
        if name.ends_with("chromedriver.exe") {
            let out_path = driver_dir.join("chromedriver.exe");
            let mut out_file = fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut out_file).map_err(|e| e.to_string())?;
            // set executable (best-effort)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&out_path)
                    .map_err(|e| e.to_string())?
                    .permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&out_path, perms).map_err(|e| e.to_string())?;
            }
            return Ok(out_path.to_string_lossy().to_string());
        }
    }
    Err("chromedriver.exe not found in archive".to_string())
}

fn get_chrome_version() -> Result<String, anyhow::Error> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    {
        // Coba dapatkan versi Chrome dari registry Windows
        use std::process::Stdio;

        if let Ok(output) = Command::new("reg")
            // .args(&["query", r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe", "/ve"])
            .args(&[
                "query",
                r"HKEY_CURRENT_USER\Software\Google\Chrome\BLBeacon",
                "/v",
                "version",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
        {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout).to_string();
                // Parse output untuk mendapatkan path Chrome
                for line in output_str.lines() {
                    if line.contains("REG_SZ") {
                        let parts: Vec<&str> = line.split("REG_SZ").collect();
                        if parts.len() > 1 {
                            let chrome_version = parts[1].trim();
                            return Ok(chrome_version.to_string());
                        }
                    }
                }
            }
        }

        // Fallback: coba jalankan chrome --version secara langsung
        if let Ok(version_output) = Command::new("chrome")
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
        {
            if version_output.status.success() {
                let version_str = String::from_utf8_lossy(&version_output.stdout).to_string();
                // Ekstrak versi (contoh: "Google Chrome 140.0.7182.0")
                let version_parts: Vec<&str> = version_str.split_whitespace().collect();
                if version_parts.len() > 2 {
                    let version = version_parts[2];
                    return Ok(version.to_string());
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Try common chrome executable names
        let candidates = [
            "chrome",
            "chrome.exe",
            "google-chrome",
            "google-chrome-stable",
        ];
        for c in &candidates {
            if let Ok(o) = Command::new(c).arg("--version").output() {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).to_string();
                    // format: Google Chrome 116.0.5845.188
                    let parts: Vec<&str> = s.split_whitespace().collect();
                    if let Some(ver) = parts.last() {
                        // return major.minor
                        let v2 = ver.split('.').take(2).collect::<Vec<_>>().join(".");
                        return Ok(v2);
                    }
                }
            }
        }
    }
    // fallback to a broad version prefix
    Ok("116".to_string())
}

#[tauri::command]
async fn scrape_products(queries: Vec<String>) -> Result<Vec<Product>, String> {
    use std::env;
    use std::path::PathBuf;
    use thirtyfour::prelude::*;

    // Ensure chromedriver is present
    let driver_path = ensure_chromedriver().await.map_err(|e| e.to_string())?;

    // Start chromedriver using the provided path as a child process
    let driver_path_buf = PathBuf::from(driver_path);
    let driver_dir = driver_path_buf.parent().ok_or("invalid driver path")?;

    // Launch chromedriver in background (std::process)
    let mut child = std::process::Command::new(driver_path_buf.as_os_str())
        .arg("--port=9515")
        .current_dir(driver_dir)
        .spawn()
        .map_err(|e| format!("failed to spawn chromedriver: {}", e))?;

    // give it a moment
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Connect with thirtyfour
    let caps = DesiredCapabilities::chrome();
    let driver = WebDriver::new("http://127.0.0.1:9515", caps)
        .await
        .map_err(|e| e.to_string())?;

    let mut results: Vec<Product> = Vec::new();
    for q in queries {
        let url = format!(
            "https://www.tokopedia.com/search?q={}",
            urlencoding::encode(&q)
        );
        // navigate
        driver.goto(&url).await.map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Attempt to find product items - updated selectors per request
        // Find anchors inside div[data-ssr="contentProductsSRPSSR"]
        let cards = match driver
            .find_all(By::Css("div[data-ssr=\"contentProductsSRPSSR\"] a"))
            .await
        {
            Ok(v) => v,
            Err(_) => Vec::new(),
        };

        for c in cards.into_iter().take(10) {
            // name: first span inside the anchor
            let name = match c.find(By::Css("div > div:nth-child(2) > div > span")).await {
                Ok(el) => el.text().await.unwrap_or_default(),
                Err(_) => String::new(),
            };

            // price: div > div:nth-child(2) > div:nth-child(2)
            let price = match c
                .find(By::Css("div > div:nth-child(2) > div:nth-child(2)"))
                .await
            {
                Ok(el) => el.text().await.unwrap_or_default(),
                Err(_) => String::new(),
            };

            // shop: span with class "flip"
            let shop = match c.find(By::Css("span.flip")).await {
                Ok(el) => el.text().await.unwrap_or_default(),
                Err(_) => String::new(),
            };

            // location: not provided in new spec, keep empty
            let location = String::new();

            // photo: img with alt="product-image"
            let photo = match c.find(By::Css("img[alt=\"product-image\"]")).await {
                Ok(el) => el.attr("src").await.unwrap_or(None).unwrap_or_default(),
                Err(_) => String::new(),
            };

            // link: href attribute of the anchor
            let link = match c.attr("href").await {
                Ok(opt) => opt.unwrap_or_default(),
                Err(_) => String::new(),
            };

            // Normalize scheme-relative or root-relative URLs to absolute
            let link = if link.starts_with("//") {
                format!("https:{}", link)
            } else if link.starts_with('/') {
                format!("https://www.tokopedia.com{}", link)
            } else {
                link
            };

            results.push(Product {
                name,
                price,
                shop,
                location,
                photo,
                link,
            });
        }
    }

    // Cleanup: try to kill chromedriver
    let _ = child.kill();

    Ok(results)
}

#[tauri::command]
async fn open_chrome_with_driver() -> Result<(), String> {
    // Opens the chromedriver executable directory in file explorer so user can inspect browser binary
    use std::env;
    let local_app_data = env::var("LOCALAPPDATA").map_err(|e| e.to_string())?;
    let driver_dir = PathBuf::from(local_app_data)
        .join("satu-toko")
        .join("chromedriver");
    #[cfg(target_os = "windows")]
    {
        let path_str = driver_dir.to_string_lossy().to_string();
        std::process::Command::new("explorer")
            .arg(path_str)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            ensure_chromedriver,
            scrape_products,
            open_chrome_with_driver
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
