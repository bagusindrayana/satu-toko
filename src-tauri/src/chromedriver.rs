use log::{error, info};
use std::env;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

pub async fn ensure_chromedriver() -> Result<String, String> {
    use reqwest::Client;

    // Determine local app data path (multiplatform)
    let driver_dir = dirs::data_local_dir()
        .ok_or("Could not determine local data directory")?
        .join("satu-toko")
        .join("chromedriver");

    fs::create_dir_all(&driver_dir)
        .map_err(|e| format!("Failed to create driver directory: {}", e))?;

    let os = std::env::consts::OS;
    let chrome_version =
        get_chrome_version().map_err(|e| format!("Failed to get Chrome version: {}", e))?;
    let major_version = chrome_version.split('.').next().unwrap_or("");

    info!("Detected Chrome version: {}", chrome_version);

    let driver_filename = match os {
        "linux" => "chromedriver",
        "macos" => "chromedriver",
        "windows" => "chromedriver.exe",
        _ => return Err("Unsupported OS!".to_string()),
    };

    let driver_path = driver_dir.join(driver_filename);

    // Check if chromedriver exists and is compatible
    if driver_path.exists() {
        if let Ok(existing_version) = get_existing_driver_version(&driver_path).await {
            if existing_version.starts_with(major_version) {
                info!("Compatible chromedriver already exists");
                return Ok(driver_path.to_string_lossy().to_string());
            } else {
                info!(
                    "Existing chromedriver version {} is incompatible with Chrome {}",
                    existing_version, chrome_version
                );
            }
        }
    }

    // Download compatible chromedriver
    info!("Downloading compatible chromedriver...");
    download_chromedriver(&driver_path, major_version, os).await?;

    Ok(driver_path.to_string_lossy().to_string())
}

async fn get_existing_driver_version(driver_path: &Path) -> Result<String, String> {
    use std::process::Command;

    let output = Command::new(driver_path)
        .arg("--version")
        .output()
        .map_err(|e| format!("Failed to execute chromedriver: {}", e))?;

    let version_output = String::from_utf8_lossy(&output.stdout);
    let version = version_output
        .split_whitespace()
        .nth(1)
        .ok_or("Could not parse chromedriver version")?
        .to_string();

    Ok(version)
}

async fn download_chromedriver(
    driver_path: &Path,
    major_version: &str,
    os: &str,
) -> Result<(), String> {
    use reqwest::Client;

    let client = Client::new();

    // Get the latest chromedriver version for this major version
    let version_url = format!(
        "https://googlechromelabs.github.io/chrome-for-testing/LATEST_RELEASE_{}",
        major_version
    );
    println!("Version URL : {}", version_url);
    let response = client
        .get(&version_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch chromedriver version: {}", e))?;

    let driver_version = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?
        .trim()
        .to_string();

    info!("Downloading chromedriver version: {}", driver_version);

    // Determine download URL based on OS
    let platform = match os {
        "linux" => "linux64",
        "macos" => "mac-x64",
        "windows" => "win64",
        _ => return Err("Unsupported OS!".to_string()),
    };

    let download_url = format!(
        "https://storage.googleapis.com/chrome-for-testing-public/{}/{}/chromedriver-{}.zip",
        driver_version, platform, platform
    );

    // Download and extract
    let response = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download chromedriver: {}", e))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read download: {}", e))?;

    let cursor = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("Failed to open zip archive: {}", e))?;

    // Extract chromedriver
    let chromedriver_name = match os {
        "linux" | "macos" => "chromedriver",
        "windows" => "chromedriver.exe",
        _ => return Err("Unsupported OS!".to_string()),
    };

    let mut file = archive
        .by_name(chromedriver_name)
        .map_err(|e| format!("Failed to find chromedriver in archive: {}", e))?;

    let mut contents = Vec::new();
    use std::io::Read;
    file.read_to_end(&mut contents)
        .map_err(|e| format!("Failed to read chromedriver from archive: {}", e))?;

    fs::write(driver_path, contents).map_err(|e| format!("Failed to write chromedriver: {}", e))?;

    // Make executable on Unix systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(driver_path)
            .map_err(|e| format!("Failed to get file metadata: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(driver_path, perms)
            .map_err(|e| format!("Failed to set executable permissions: {}", e))?;
    }

    info!("Chromedriver downloaded successfully to: {:?}", driver_path);
    Ok(())
}

pub fn patch_driver(driver_path: &Path) -> Result<(), String> {
    use std::fs::File;
    use std::io::{Read, Write};

    info!("Patching chromedriver...");

    let mut file =
        File::open(driver_path).map_err(|e| format!("Failed to open chromedriver: {}", e))?;

    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .map_err(|e| format!("Failed to read chromedriver: {}", e))?;

    // Find and replace the signature
    let signature = b"cdc_ads_client_";
    let replacement = b"abc_ads_client_";

    if let Some(pos) = find_signature(&contents, signature) {
        for i in 0..signature.len() {
            contents[pos + i] = replacement[i];
        }

        let mut file = File::create(driver_path)
            .map_err(|e| format!("Failed to create patched chromedriver: {}", e))?;

        file.write_all(&contents)
            .map_err(|e| format!("Failed to write patched chromedriver: {}", e))?;

        info!("Chromedriver patched successfully");
        Ok(())
    } else {
        Err("Signature not found in chromedriver".to_string())
    }
}

fn find_signature(data: &[u8], signature: &[u8]) -> Option<usize> {
    for i in 0..data.len() - signature.len() {
        if &data[i..i + signature.len()] == signature {
            return Some(i);
        }
    }
    None
}

pub fn find_chrome_executable() -> Result<PathBuf, String> {
    use std::process::Command;

    let os = std::env::consts::OS;

    match os {
        "windows" => {
            let paths = [
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
                r"C:\Users\{}\AppData\Local\Google\Chrome\Application\chrome.exe",
            ];

            for path_template in &paths {
                let path = if path_template.contains("{}") {
                    if let Ok(user) = env::var("USERNAME") {
                        PathBuf::from(path_template.replace("{}", &user))
                    } else {
                        continue;
                    }
                } else {
                    PathBuf::from(path_template)
                };

                if path.exists() {
                    return Ok(path);
                }
            }

            // Try using where command
            if let Ok(output) = Command::new("where").arg("chrome.exe").output() {
                let path_str = String::from_utf8_lossy(&output.stdout);
                if !path_str.trim().is_empty() {
                    return Ok(PathBuf::from(path_str.trim()));
                }
            }

            Err("Chrome executable not found".to_string())
        }
        "macos" => {
            let path =
                PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
            if path.exists() {
                Ok(path)
            } else {
                Err("Chrome executable not found".to_string())
            }
        }
        "linux" => {
            let paths = [
                "/usr/bin/google-chrome",
                "/usr/bin/google-chrome-stable",
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
            ];

            for path in &paths {
                let path = PathBuf::from(path);
                if path.exists() {
                    return Ok(path);
                }
            }

            // Try using which command
            if let Ok(output) = Command::new("which").arg("google-chrome").output() {
                let path_str = String::from_utf8_lossy(&output.stdout);
                if !path_str.trim().is_empty() {
                    return Ok(PathBuf::from(path_str.trim()));
                }
            }

            Err("Chrome executable not found".to_string())
        }
        _ => Err("Unsupported OS!".to_string()),
    }
}

pub fn is_command_available(command: &str) -> bool {
    use std::process::Command;

    Command::new("which")
        .arg(command)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn get_chrome_version() -> Result<String, String> {
    use std::process::Command;

    let chrome_path = find_chrome_executable()?;
    info!("Found Chrome at: {:?}", chrome_path);

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
                            info!("Chrome version output: {}", chrome_version);
                            return Ok(chrome_version.to_string());
                        }
                    }
                }
            }
        }
    }

    let output = Command::new(&chrome_path)
        .arg("--version")
        .output()
        .map_err(|e| format!("Failed to execute Chrome: {}", e))?;

    let version_output = String::from_utf8_lossy(&output.stdout);
    info!("Chrome version output: {}", version_output);

    // Parse version from output like "Google Chrome 120.0.6099.109"
    let version = version_output
        .split_whitespace()
        .last()
        .ok_or("Could not parse Chrome version")?
        .to_string();

    Ok(version)
}

pub async fn redownload_chromedriver() -> Result<String, String> {
    info!("Redownloading chromedriver...");

    // Remove existing chromedriver
    let driver_dir = dirs::data_local_dir()
        .ok_or("Could not determine local data directory")?
        .join("satu-toko")
        .join("chromedriver");

    if driver_dir.exists() {
        fs::remove_dir_all(&driver_dir)
            .map_err(|e| format!("Failed to remove existing chromedriver: {}", e))?;
    }

    // Download fresh chromedriver
    ensure_chromedriver().await
}
