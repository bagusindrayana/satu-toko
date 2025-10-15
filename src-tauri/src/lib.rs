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

    // Determine local app data path (multiplatform)
    let driver_dir = dirs::data_local_dir()
        .ok_or("Could not determine local data directory")?
        .join("satu-toko")
        .join("chromedriver");
    fs::create_dir_all(&driver_dir).map_err(|e| e.to_string())?;

    // 1) Find installed chrome version
    let mut chrome_version = get_chrome_version().map_err(|e| e.to_string())?;
    info!("Detected Chrome version: {}", chrome_version);

    let parts: Vec<&str> = chrome_version.split('.').collect();
    let prefix = parts.iter().take(2).map(|s| s.to_string()).collect::<Vec<_>>().join(".");
    if !prefix.is_empty() {
        chrome_version = prefix;
    }

    use std::process::Command;

    // 2) If we already have chromedriver, check its version and avoid re-downloading
    let executable_name = if cfg!(target_os = "windows") {
        "chromedriver.exe"
    } else {
        "chromedriver"
    };
    let existing_driver_path = driver_dir.join(executable_name);
    if existing_driver_path.exists() {
        // Try to run `chromedriver --version` and compare major version with Chrome
        if let Ok(output) = Command::new(&existing_driver_path).arg("--version").output() {
            if output.status.success() {
                let out_str = String::from_utf8_lossy(&output.stdout).to_string();
                // expected: "ChromeDriver 116.0.5845.96 (...)" -> take the second token
                if let Some(driver_ver) = out_str.split_whitespace().nth(1) {
                    let driver_major = driver_ver.split('.').next().unwrap_or(driver_ver);
                    let chrome_major = chrome_version.split('.').next().unwrap_or(&chrome_version);
                    if driver_major == chrome_major {
                        info!("Found existing chromedriver with matching major version: {}", driver_ver);
                        return Ok(existing_driver_path.to_string_lossy().to_string());
                    } else {
                        info!("Existing chromedriver major {} != chrome major {} -> will re-download", driver_major, chrome_major);
                        // attempt to remove mismatched driver to ensure fresh install
                        let _ = std::fs::remove_file(&existing_driver_path);
                    }
                }
            } else {
                info!("Existing chromedriver found but --version returned non-zero -> re-download");
                let _ = std::fs::remove_file(&existing_driver_path);
            }
        } else {
            info!("Failed to execute existing chromedriver -> re-download");
            let _ = std::fs::remove_file(&existing_driver_path);
        }
    }

    // 3) Fetch chrome-for-testing JSON
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

    // Find platform download based on OS
    let platform_name = if cfg!(target_os = "windows") {
        "win64"
    } else if cfg!(target_os = "macos") {
        "mac-arm64"
    } else {
        "linux64"
    };

    info!("Platform: {}", platform_name);

    let downloads = chosen["downloads"]["chromedriver"]
        .as_array()
        .ok_or("no chromedriver downloads")?;
    let mut url: Option<String> = None;
    for d in downloads {
        if let Some(platform) = d["platform"].as_str() {
            if platform.contains(platform_name) {
                url = d["url"].as_str().map(|s| s.to_string());
                break;
            }
        }
    }
    let url = url.ok_or(format!("no {} chromedriver in JSON", platform_name))?;
    info!("URL: {}", url);

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

    // Find chromedriver inside zip (platform-specific executable name)
    let executable_name = if cfg!(target_os = "windows") {
        "chromedriver.exe"
    } else {
        "chromedriver"
    };

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_string();
        if name.ends_with(executable_name) {
            let out_path = driver_dir.join(executable_name);
            let mut out_file = fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut out_file).map_err(|e| e.to_string())?;
            
            // Set executable permissions on Unix systems
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
    Err(format!("{} not found in archive", executable_name).to_string())
}

// Helper function to find Chrome executable path
fn find_chrome_executable() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        // Try common Windows Chrome paths
        let paths = [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];
        
        for path in &paths {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }
        
        // Try just "chrome" which might be in PATH
        if is_command_available("chrome") {
            return Some("chrome".to_string());
        }
        
        // Try "chrome.exe" 
        if is_command_available("chrome.exe") {
            return Some("chrome.exe".to_string());
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        // Try the standard macOS Chrome path
        let path = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
        
        // Try "google-chrome" which might be in PATH
        if is_command_available("google-chrome") {
            return Some("google-chrome".to_string());
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        // Try common Linux Chrome/Chromium commands
        let commands = ["google-chrome", "google-chrome-stable", "chromium-browser", "chromium"];
        for cmd in &commands {
            if is_command_available(cmd) {
                return Some(cmd.to_string());
            }
        }
    }
    
    // Universal fallback - try common commands
    let commands = ["chrome", "google-chrome", "chromium-browser", "chromium"];
    for cmd in &commands {
        if is_command_available(cmd) {
            return Some(cmd.to_string());
        }
    }
    
    None
}

// Helper function to check if a command is available
fn is_command_available(command: &str) -> bool {
    use std::process::Command;
    #[cfg(target_os = "windows")]
    {
        Command::new("where")
            .arg(command)
            .output()
            .map(|output| output.status.success() && !output.stdout.is_empty())
            .unwrap_or(false)
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("which")
            .arg(command)
            .output()
            .map(|output| output.status.success() && !output.stdout.is_empty())
            .unwrap_or(false)
    }
}

fn get_chrome_version() -> Result<String, anyhow::Error> {
    use std::process::Command;

    // Platform-specific Chrome version detection
    #[cfg(target_os = "windows")]
    {
        // Try to get Chrome version from Windows registry
        use std::process::Stdio;

        if let Ok(output) = Command::new("reg")
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

        // Fallback to command line approach with multiple options
        if let Some(chrome_path) = find_chrome_executable() {
            if let Ok(version_output) = Command::new(&chrome_path)
                .arg("--version")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
            {
                if version_output.status.success() {
                    let version_str = String::from_utf8_lossy(&version_output.stdout).to_string();
                    let version_parts: Vec<&str> = version_str.split_whitespace().collect();
                    if version_parts.len() > 2 {
                        let version = version_parts[2];
                        return Ok(version.to_string());
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Try macOS-specific locations
        if let Some(chrome_path) = find_chrome_executable() {
            if let Ok(o) = Command::new(&chrome_path).arg("--version").output() {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).to_string();
                    let parts: Vec<&str> = s.split_whitespace().collect();
                    if let Some(ver) = parts.last() {
                        let v2 = ver.split('.').take(2).collect::<Vec<_>>().join(".");
                        return Ok(v2);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(chrome_cmd) = find_chrome_executable() {
            if let Ok(o) = Command::new(&chrome_cmd).arg("--version").output() {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).to_string();
                    let parts: Vec<&str> = s.split_whitespace().collect();
                    if let Some(ver) = parts.get(2) {  // Chrome version is typically the 3rd word
                        let v2 = ver.split('.').take(2).collect::<Vec<_>>().join(".");
                        return Ok(v2);
                    } else if let Some(ver) = parts.last() {  // Fallback for other formats
                        let v2 = ver.split('.').take(2).collect::<Vec<_>>().join(".");
                        return Ok(v2);
                    }
                }
            }
        }
    }

    // Universal fallback
    if let Some(chrome_cmd) = find_chrome_executable() {
        if let Ok(o) = Command::new(&chrome_cmd).arg("--version").output() {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).to_string();
                let parts: Vec<&str> = s.split_whitespace().collect();
                if let Some(ver) = parts.get(2) {  // Chrome version is typically the 3rd word
                    let v2 = ver.split('.').take(2).collect::<Vec<_>>().join(".");
                    return Ok(v2);
                } else if let Some(ver) = parts.last() {  // Fallback for other formats
                    let v2 = ver.split('.').take(2).collect::<Vec<_>>().join(".");
                    return Ok(v2);
                }
            }
        }
    }

    // Default fallback version
    Ok("116".to_string())
}

#[tauri::command]
async fn scrape_products(window: tauri::Window, queries: Vec<String>) -> Result<Vec<ShopResults>, String> {
    use std::env;
    use std::path::PathBuf;
    use thirtyfour::prelude::*;
    use thirtyfour::Key;

    // Check chromedriver
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
                let _ = el.click().await;
                // let _ = el.send_keys(Key::Backspace).await;
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

                    let spans = match c.find_all(By::Css("div:nth-child(1) > div:nth-child(2) > div:nth-child(1) span"))
                        .await
                    {
                        Ok(v) => v,
                        Err(_) => Vec::new(),
                    };
                    info!("CHECK SPAN : {}", slug);
                    let mut name = String::new();
                    for s in spans.into_iter().take(20) {
                        info!("span: {}", s.text().await.unwrap_or_default());
                        if !s.text().await.unwrap_or_default().is_empty() && name.is_empty() {
                            name = s.text().await.unwrap_or_default();
                        }
                    }
                    // let name = match c.find(By::Css("div > div:nth-child(2) span:nth-child(1)")).await {
                    //     Ok(el) => el.text().await.unwrap_or_default(),
                    //     Err(_) => String::new(),
                    // };
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
                    let location = match c
                        .find(By::Css("div > div:nth-child(2) > div:nth-child(3) span:nth-child(2)"))
                        .await
                    {
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
                        location,
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

                    let spans = match c.find_all(By::Css("div:nth-child(1) > div:nth-child(2) > div:nth-child(1) span"))
                        .await
                    {
                        Ok(v) => v,
                        Err(_) => Vec::new(),
                    };
                    info!("CHECK SPAN : {}", slug);
                    let mut name = String::new();
                    for s in spans.into_iter().take(20) {
                        info!("span: {}", s.text().await.unwrap_or_default());
                        if !s.text().await.unwrap_or_default().is_empty() && name.is_empty() {
                            name = s.text().await.unwrap_or_default();
                        }
                    }

                    // let name = match c.find(By::Css("div > div:nth-child(2) span:nth-child(1)")).await {
                    //     Ok(el) => el.text().await.unwrap_or_default(),
                    //     Err(_) => String::new(),
                    // };
                    let price = match c
                        .find(By::Css("div > div:nth-child(2) > div:nth-child(2)"))
                        .await
                    {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
                    let shop = shop_display.clone();
                    let location = match c
                        .find(By::Css("div > div:nth-child(2) > div:nth-child(3) span:nth-child(2)"))
                        .await
                    {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
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
    
    // Determine driver directory (multiplatform)
    let driver_dir = dirs::data_local_dir()
        .ok_or("Could not determine local data directory")?
        .join("satu-toko")
        .join("chromedriver");
    
    // Platform-specific file explorer
    #[cfg(target_os = "windows")]
    {
        let path_str = driver_dir.to_string_lossy().to_string();
        std::process::Command::new("explorer")
            .arg(path_str)
            .spawn()
            .map(|_| ())  // Convert Result<Child, Error> to Result<(), Error>
            .map_err(|e| e.to_string())?;
    }
    
    #[cfg(target_os = "macos")]
    {
        let path_str = driver_dir.to_string_lossy().to_string();
        std::process::Command::new("open")
            .arg(path_str)
            .spawn()
            .map(|_| ())  // Convert Result<Child, Error> to Result<(), Error>
            .map_err(|e| e.to_string())?;
    }
    
    #[cfg(target_os = "linux")]
    {
        let path_str = driver_dir.to_string_lossy().to_string();
        std::process::Command::new("xdg-open")
            .arg(path_str)
            .spawn()
            .map(|_| ())  // Convert Result<Child, Error> to Result<(), Error>
            .map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

// New command to get Chrome and ChromeDriver versions
#[tauri::command]
async fn get_chrome_and_driver_info() -> Result<(String, String), String> {
    // Get Chrome version
    let chrome_version = get_chrome_version().map_err(|e| e.to_string())?;
    
    // Get ChromeDriver version by running chromedriver --version
    use std::env;
    use std::process::Command;
    
    // Determine driver path (multiplatform)
    let driver_dir = dirs::data_local_dir()
        .ok_or("Could not determine local data directory")?
        .join("satu-toko")
        .join("chromedriver");
    
    // Platform-specific executable name
    let executable_name = if cfg!(target_os = "windows") {
        "chromedriver.exe"
    } else {
        "chromedriver"
    };
    
    let driver_path = driver_dir.join(executable_name);
    
    if !driver_path.exists() {
        return Err("ChromeDriver not found".to_string());
    }
    
    let output = Command::new(driver_path)
        .arg("--version")
        .output()
        .map_err(|e| format!("Failed to execute ChromeDriver: {}", e))?;
    
    let driver_version_output = String::from_utf8_lossy(&output.stdout);
    let driver_version = driver_version_output
        .split_whitespace()
        .nth(1)
        .unwrap_or("Unknown")
        .to_string();
    
    Ok((chrome_version, driver_version))
}

// New command to re-download ChromeDriver
#[tauri::command]
async fn redownload_chromedriver() -> Result<String, String> {
    use std::env;
    use std::fs;
    
    // Remove existing ChromeDriver
    let driver_dir = dirs::data_local_dir()
        .ok_or("Could not determine local data directory")?
        .join("satu-toko")
        .join("chromedriver");
    
    // Remove the directory if it exists
    if driver_dir.exists() {
        fs::remove_dir_all(&driver_dir).map_err(|e| format!("Failed to remove existing ChromeDriver: {}", e))?;
    }
    
    // Re-download ChromeDriver
    ensure_chromedriver().await
}

// New command to open browser with ChromeDriver
#[tauri::command]
async fn open_browser_with_driver() -> Result<(), String> {
    use std::env;
    use std::path::PathBuf;
    use thirtyfour::prelude::*;
    use thirtyfour::Key;
    // use std::process::Command;
    // use std::thread;
    // use std::time::Duration;
    

    // Check chromedriver
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

    let caps = DesiredCapabilities::chrome();
    
    // Connect to WebDriver. The ? converts Err(WebDriverError) to Err(String).
    let driver = WebDriver::new("http://127.0.0.1:9515", caps)
        .await
        .map_err(|e| format!("Fatal Error: Driver initialization failed. Details: {}", e.to_string()))?;
    
    let url = "https://www.tokopedia.com/";
    let result_of_goto = driver.goto(url).await;
    
    // 💡 The 'result' variable MUST be a Result<(), String> to be returned.
    let final_result: Result<(), String> = match result_of_goto {
        // SUCCESS: Navigation succeeded. Return Ok(()).
        Ok(()) => {
            let _ = child.kill();
            println!("Success: Navigated to '{}'", url);
            Ok(())
        },
        
        // ERROR: Navigation failed. Format the error string and wrap it in Err().
        Err(e) => {
            let _ = child.kill();
            // Use {:?} to format the WebDriverError for logging/debugging
            Err(format!("Navigation Error: Failed to navigate to '{}'. Details: {:?}", url, e))
        },
    };

    

    // // Clean up the driver session
    // let _ = driver.quit().await;

    // Return the final Result<(), String>
    final_result
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
            open_chrome_with_driver,
            get_chrome_and_driver_info,
            redownload_chromedriver,
            open_browser_with_driver
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}