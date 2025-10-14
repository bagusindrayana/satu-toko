use tauri_plugin_log::{Target, TargetKind};
use log::{error, info};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri::Emitter;
use std::path::PathBuf;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

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
    pub results: Vec<QueryResult>,
}


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

    // 1) Find installed chrome version (Windows)
    let mut chrome_version = get_chrome_version().map_err(|e| e.to_string())?;
    info!("Versi Chrome terdeteksi : {}", chrome_version);

    let parts: Vec<&str> = chrome_version.split('.').collect();

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
    // default
    Ok("116".to_string())
}

#[tauri::command]
async fn scrape_products(window: tauri::Window, queries: Vec<String>) -> Result<Vec<ShopResults>, String> {
    use std::env;
    use std::path::PathBuf;
    use thirtyfour::prelude::*;

    // Cek chromedriver
    let driver_path = ensure_chromedriver().await.map_err(|e| e.to_string())?;

    // Start chromedriver
    let driver_path_buf = PathBuf::from(driver_path);
    let driver_dir = driver_path_buf.parent().ok_or("invalid driver path")?;

    // Launch chromedriver
    let mut child = std::process::Command::new(driver_path_buf.as_os_str())
        .arg("--port=9515")
        .current_dir(driver_dir)
        .spawn()
        .map_err(|e| format!("failed to spawn chromedriver: {}", e))?;


    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Connect with thirtyfour
    let caps = DesiredCapabilities::chrome();
    let driver = WebDriver::new("http://127.0.0.1:9515", caps)
        .await
        .map_err(|e| e.to_string())?;

    if queries.is_empty() {
        let _ = child.kill();
        return Ok(Vec::new());
    }

    let first_query = &queries[0];


    async fn perform_site_search(driver: &WebDriver, query: &str) -> Result<(), ()> {
        // Cari input
        let sel = r#"input[data-unify="Search"][type="search"]"#;
        if let Ok(el) = driver.find(By::Css(sel)).await {
            if el.is_displayed().await.unwrap_or(false) {
                let _ = el.clear().await;
                let _ = el.send_keys(query).await;
                let _ = el.send_keys("\n").await;
                return Ok(());
            }
        }
        Err(())
    }

    let _ = driver.goto("https://www.tokopedia.com/").await;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // coba cari dengan input
    if perform_site_search(&driver, first_query).await.is_err() {
        let first_url = format!(
            "https://www.tokopedia.com/search?q={}",
            urlencoding::encode(first_query)
        );
        let _ = driver.goto(&first_url).await;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    } else {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    let first_cards = match driver
        .find_all(By::Css("div[data-ssr=\"contentProductsSRPSSR\"] a"))
        .await
    {
        Ok(v) => v,
        Err(_) => Vec::new(),
    };

    use std::collections::HashSet;
    let mut shop_slugs: HashSet<String> = HashSet::new();
    let mut shop_names: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut first_products_map: std::collections::HashMap<String, Vec<Product>> = std::collections::HashMap::new();

    for c in first_cards.into_iter().take(20) {
        let link = match c.attr("href").await {
            Ok(opt) => opt.unwrap_or_default(),
            Err(_) => String::new(),
        };

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
                    let name = match c.find(By::Css("div > div:nth-child(2) > div > span")).await {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
                    let price = match c
                        .find(By::Css("div > div:nth-child(2) > div:nth-child(2)"))
                        .await
                    {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
                    let shop_display = match c.find(By::Css("span.flip")).await {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
                    let photo = match c.find(By::Css("img[alt=\"product-image\"]")).await {
                        Ok(el) => el.attr("src").await.unwrap_or(None).unwrap_or_default(),
                        Err(_) => String::new(),
                    };

                    shop_slugs.insert(slug.to_string());
                    if !shop_display.is_empty() {
                        shop_names.insert(slug.to_string(), shop_display.clone());
                    }

                    let prod = Product {
                        name,
                        price,
                        shop: shop_display.clone(),
                        location: String::new(),
                        photo,
                        link: link.clone(),
                    };
                    first_products_map
                        .entry(slug.to_string())
                        .or_insert_with(Vec::new)
                        .push(prod);
                }
            }
        }
    }

    let mut grouped: Vec<ShopResults> = Vec::new();

    for slug in shop_slugs.into_iter() {
        let shop_url = format!("https://www.tokopedia.com/{}", slug);
        let shop_display = shop_names.get(&slug).cloned().unwrap_or_else(|| slug.clone());

        let mut qresults: Vec<QueryResult> = Vec::new();

        for q in &queries {
            let mut products: Vec<Product> = Vec::new();

            //produk pertama yang didapat gak perlu dicari di tokonya lagi
            if q == first_query {
                if let Some(v) = first_products_map.get(&slug) {
                    products = v.clone();
                }
            } else {

                let shop_page = format!("https://www.tokopedia.com/{}", slug);
                let _ = driver.goto(&shop_page).await;
                
              
                // tunggu nama tokonya muncul
                {
                    use std::time::Duration as StdDuration;
                    let start = std::time::Instant::now();
                    let timeout = StdDuration::from_secs(6);
                    loop {
                        if driver
                            .find(By::Css("h1[data-testid=\"shopNameHeader\"]"))
                            .await
                            .is_ok()
                        {
                            break;
                        }

                        if start.elapsed() >= timeout {
                            // timed out waiting
                            info!("Timed out waiting for shopNameHeader to load");
                            break;
                        }

                        
                        
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }

                // wait products to load.
                let mut used_input = false;
                if perform_site_search(&driver, q).await.is_ok() {
                    used_input = true;
                    info!("Performed search via input for shop {} query {}", slug, q);
                    {
                        use std::time::Duration as StdDuration;
                        let start = std::time::Instant::now();
                        let timeout = StdDuration::from_secs(6);
                        loop {
                            if driver
                                .find(By::Css("img[alt=\"product-image\"]"))
                                .await
                                .is_ok()
                            {   
                                info!("Found products");
                                break;
                            }

                            if driver
                                .find(By::Css("div[class=\"unf-emptystate-img\"]"))
                                .await
                                .is_ok()
                            {   
                                info!("emptystate");
                                break;
                            } else {
                                if start.elapsed() >= timeout {
                                    // timed out waiting
                                    info!("Timed out waiting for products to load 1");
                                    break;
                                }
                            }
                            
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    }
                }

                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

                // gagal cari pakai input, coba cari langsung dengan url (rawan redirect)
                if !used_input {
                    let url = format!(
                        "https://www.tokopedia.com/{}/product?q={}&srp_page_title={}&navsource=shop&srp_component_id=02.01.00.00",
                        slug,
                        urlencoding::encode(q),
                        urlencoding::encode(&shop_display)
                    );
                    let _ = driver.goto(&url).await;
                    info!("Fallback URL PENCARIAN : {}", url);
                }

          
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                let cards = match driver
                    .find_all(By::Css(r#"[data-ssr="shopSSR"] > div:nth-child(2) a[data-theme="default"]"#))
                    .await
                {
                    Ok(v) => v,
                    Err(_) => Vec::new(),
                };

                for c in cards.into_iter().take(20) {
                    let link = match c.attr("href").await {
                        Ok(opt) => opt.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
                    if link.starts_with(&format!("/{}/product?perpage", slug)) {
                        continue;
                    }
                    let link = if link.starts_with("//") {
                        format!("https:{}", link)
                    } else if link.starts_with('/') {
                        format!("https://www.tokopedia.com{}", link)
                    } else {
                        link
                    };

                    

                    let name = match c.find(By::Css("div > div:nth-child(2) > div > span")).await {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
                    let price = match c
                        .find(By::Css("div > div:nth-child(2) > div:nth-child(2)"))
                        .await
                    {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
                    let shop = shop_display.clone();
                    let location = String::new();
                    let photo = match c.find(By::Css("img[alt=\"product-image\"]")).await {
                        Ok(el) => el.attr("src").await.unwrap_or(None).unwrap_or_default(),
                        Err(_) => String::new(),
                    };

                    products.push(Product {
                        name,
                        price,
                        shop: shop.clone(),
                        location,
                        photo,
                        link,
                    });
                }
            }

            qresults.push(QueryResult {
                query: q.clone(),
                products,
            });
        }

        let shop_result = ShopResults {
            shop_name: shop_display.clone(),
            shop_url: shop_url.clone(),
            results: qresults,
        };

        // Emit progress real-time
        let _ = window.emit("scrape:progress", shop_result.clone()).map_err(|e| e.to_string());

        grouped.push(shop_result);
    }

    // Cleanup: kill chromedriver
    let _ = child.kill();

    // emit done
    let _ = window.emit("scrape:done", ()).map_err(|e| e.to_string());

    Ok(grouped)
}

#[tauri::command]
async fn open_chrome_with_driver() -> Result<(), String> {
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
