// Legacy platform scrapers - kept for reference but not used in current implementation
// The main scraping logic is now in scraper.rs with original shop grouping functionality

// Helper function to handle common scraping patterns
pub async fn wait_for_elements(
    driver: &thirtyfour::WebDriver,
    selector: &str,
    timeout_secs: u64,
) -> Result<Vec<thirtyfour::WebElement>, String> {
    use thirtyfour::By;
    use tokio::time::{sleep, Duration};

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
pub async fn safe_extract_text(element: &thirtyfour::WebElement, selector: &str) -> String {
    use thirtyfour::By;

    match element.find(By::Css(selector)).await {
        Ok(el) => el.text().await.unwrap_or_default(),
        Err(_) => String::new(),
    }
}

// Helper function to safely extract attribute from elements
pub async fn safe_extract_attr(
    element: &thirtyfour::WebElement,
    selector: &str,
    attr: &str,
) -> String {
    use thirtyfour::By;

    match element.find(By::Css(selector)).await {
        Ok(el) => el.attr(attr).await.unwrap_or_default().unwrap_or_default(),
        Err(_) => String::new(),
    }
}
