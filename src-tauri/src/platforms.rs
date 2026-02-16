use log::info;
use std::collections::{HashMap, HashSet};
use tauri::Emitter;
use thirtyfour::prelude::*;
use tokio::time::{sleep, Duration};

use crate::models::{Product, QueryResult, ShopResults};

// Tokopedia scraper implementation
pub struct TokopediaScraper;

impl TokopediaScraper {
    pub async fn scrape(
        driver: &WebDriver,
        queries: &[String],
        window: &tauri::Window,
        limit: usize,
    ) -> Result<Vec<ShopResults>, String> {
        info!("Starting Tokopedia scraping with limit {}", limit);

        if queries.is_empty() {
            return Ok(Vec::new());
        }

        let first_query = &queries[0];

        // Navigate to Tokopedia
        let _ = driver.goto("https://www.tokopedia.com/").await;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Try search with input first
        if Self::perform_site_search(driver, first_query)
            .await
            .is_err()
        {
            let first_url = format!(
                "https://www.tokopedia.com/search?q={}",
                urlencoding::encode(first_query)
            );
            let _ = driver.goto(&first_url).await;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        } else {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        // Scroll and load more for the first result to get enough shops
        let mut first_cards = Vec::new();
        let mut scroll_attempts = 0;
        let max_scroll_attempts = 20; // Prevent infinite loop

        loop {
            // Get current cards
             let current_cards = match driver
                .find_all(By::Css("div[data-ssr=\"contentProductsSRPSSR\"] a"))
                .await
            {
                Ok(v) => v,
                Err(_) => Vec::new(),
            };

            if current_cards.len() >= limit || scroll_attempts >= max_scroll_attempts {
                 first_cards = current_cards;
                 break;
            }

            // Scroll down
             let _ = driver
                .execute("window.scrollTo(0, document.body.scrollHeight);", vec![])
                .await;
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            
            // Check for "Muat Lebih Banyak" button
            // Note: Selector might need adjustment based on actual site
             if let Ok(button) = driver.find(By::XPath("//button[contains(text(), 'Muat Lebih Banyak')]")).await {
                 if button.is_displayed().await.unwrap_or(false) {
                     let _ = button.click().await;
                     tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                 }
             }

            scroll_attempts += 1;
        }
        
        // Take only up to limit
        if first_cards.len() > limit {
             first_cards.truncate(limit);
        }

        let mut shop_slugs: HashSet<String> = HashSet::new();
        let mut shop_names: HashMap<String, String> = HashMap::new();
        let mut first_products_map: HashMap<String, Vec<Product>> = HashMap::new();

        for c in first_cards {
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
                        let spans = match c
                            .find_all(By::Css(
                                "div:nth-child(1) > div:nth-child(2) > div:nth-child(1) span",
                            ))
                            .await
                        {
                            Ok(v) => v,
                            Err(_) => Vec::new(),
                        };
                        
                        let mut name = String::new();
                        for s in spans.into_iter().take(20) {
                            let span_text = s.text().await.unwrap_or_default();
                            // info!("span: {}", span_text);
                            if !span_text.is_empty() && name.is_empty() {
                                name = span_text;
                            }
                        }

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
                            .find(By::Css(
                                "div > div:nth-child(2) > div:nth-child(3) span:nth-child(2)",
                            ))
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
            let shop_display = shop_names
                .get(&slug)
                .cloned()
                .unwrap_or_else(|| slug.clone());

            let mut qresults: Vec<QueryResult> = Vec::new();

            for q in queries.iter() {
                let mut products: Vec<Product> = Vec::new();

                // First query products don't need to be searched again in the shop
                // But if we want to support limit per shop/query, we might need to adjust logic.
                // For now, assuming first query items found in global search are sufficient/starter.
                // If the user wants more items from the shop for the first query, we might need to visit shop page too.
                // CURRENT LOGIC: use what we found in global search for first query.
                if q == first_query {
                    if let Some(v) = first_products_map.get(&slug) {
                        products = v.clone();
                    }
                    // If we need more items for this query from this shop specifically, we should search in shop.
                    // But usually global search is enough to identify shops.
                } else {
                    let shop_page = format!("https://www.tokopedia.com/{}", slug);
                    let _ = driver.goto(&shop_page).await;

                    // Wait for shop name to appear
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
                                info!("Timed out waiting for shopNameHeader to load");
                                break;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    }
                    // Wait for products to load.
                    let mut used_input = false;
                    if Self::perform_site_search(driver, &q).await.is_ok() {
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
                                        info!("Timed out waiting for products to load 1");
                                        break;
                                    }
                                }

                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            }
                        }
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

                    // Failed to search using input, try direct URL (risky redirect)
                    if !used_input {
                        let url = format!(
                            "https://www.tokopedia.com/{}/product?q={}&srp_page_title={}&navsource=shop&srp_component_id=02.01.00.00",
                            slug,
                            urlencoding::encode(&q),
                            urlencoding::encode(&shop_display)
                        );
                        let _ = driver.goto(&url).await;
                        info!("Fallback URL PENCARIAN : {}", url);
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    
                    // Implement scrolling for shop search results if needed?
                    // Usually shop search results are less than global search, but we can try small scroll.
                    let _ = driver
                        .execute("window.scrollTo(0, document.body.scrollHeight);", vec![])
                        .await;
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;


                    let cards = match driver
                        .find_all(By::Css(
                            r#"[data-ssr="shopSSR"] > div:nth-child(2) a[data-theme="default"]"#,
                        ))
                        .await
                    {
                        Ok(v) => v,
                        Err(_) => Vec::new(),
                    };

                    for c in cards.into_iter().take(limit) { // Apply limit here too
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

                        let spans = match c
                            .find_all(By::Css(
                                "div:nth-child(1) > div:nth-child(2) > div:nth-child(1) span",
                            ))
                            .await
                        {
                            Ok(v) => v,
                            Err(_) => Vec::new(),
                        };
                        
                        let mut name = String::new();
                        for s in spans.into_iter().take(20) {
                            let span_text = s.text().await.unwrap_or_default();
                            // info!("span: {}", span_text);
                            if !span_text.is_empty() && name.is_empty() {
                                name = span_text;
                            }
                        }

                        let price = match c
                            .find(By::Css("div > div:nth-child(2) > div:nth-child(2)"))
                            .await
                        {
                            Ok(el) => el.text().await.unwrap_or_default(),
                            Err(_) => String::new(),
                        };
                        let shop = shop_display.clone();
                        let location = match c
                            .find(By::Css(
                                "div > div:nth-child(2) > div:nth-child(3) span:nth-child(2)",
                            ))
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
                platform: "tokopedia".to_string(),
                results: qresults,
            };

            // Emit progress real-time
            let _ = window
                .emit("scrape:progress", shop_result.clone())
                .map_err(|e| e.to_string());

            grouped.push(shop_result);
        }

        // Emit done
        let _ = window.emit("scrape:done", ()).map_err(|e| e.to_string());

        Ok(grouped)
    }

    async fn perform_site_search(driver: &WebDriver, query: &str) -> Result<(), ()> {
        // Cari input
        let sel = r#"input[data-unify="Search"][type="search"]"#;
        if let Ok(el) = driver.find(By::Css(sel)).await {
            if el.is_displayed().await.unwrap_or(false) {
                let _ = el.click().await;
                let _ = el.clear().await;
                let _ = el.send_keys(query).await;
                let _ = el.send_keys("\n").await;
                return Ok(());
            }
        }
        Err(())
    }
}

// Shopee scraper implementation
pub struct ShopeeScraper;

impl ShopeeScraper {
    pub async fn scrape(
        driver: &WebDriver,
        queries: &[String],
        window: &tauri::Window,
        limit: usize,
    ) -> Result<Vec<ShopResults>, String> {
        info!("Starting Shopee scraping with limit {}", limit);

        if queries.is_empty() {
            return Ok(Vec::new());
        }

        let first_query = &queries[0];

        // Navigate to Shopee
        let _ = driver.goto("https://shopee.co.id/").await;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Try search with input first
        if Self::perform_site_search(&driver, first_query)
            .await
            .is_err()
        {
            let first_url = format!(
                "https://shopee.co.id/search?keyword={}",
                urlencoding::encode(first_query)
            );
            let _ = driver.goto(&first_url).await;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        } else {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        let mut collected_items = 0;
        let mut first_cards_collected = Vec::new();
        let mut page = 0;

        loop {
            // Get current page cards
            let current_cards = match driver
                .find_all(By::Css(".shopee-search-item-result__item a"))
                .await
            {
                Ok(v) => v,
                Err(_) => Vec::new(),
            };
            
            // Add to collection
            // Use ElementId to avoid duplicates if possible, or just collect all and process.
            // But we need to process links to avoid duplicates.
            // For simplicitly, let's collect links and process them.
            // But here we need to keep `WebElement` to extract data later?
            // Extracting data here might be safer because elements go stale after navigation.

            // Wait... we need to process cards HERE because once we navigate to next page, elements are gone.
            // But we can just extract the Links and then process them?
            // No, the original logic extracts data from the card element.
            // So we should extract data here and accumulate it.
            
            // Actually, we need to extract Shop Info to group. 
            // The existing structure collects `first_cards` (WebElements) then iterates them.
            // If we navigate, `first_cards` elements become stale.
            // So we MUST extract data from `current_cards` immediately.
            
            // However, the original code logic separates:
            // 1. Collect first_cards (WebElements)
            // 2. Iterate first_cards to extract product data & shop info -> `first_products_map` & `shop_slugs`.
            // 3. Iterate shop_slugs to drill down other queries.

            // So if I want to paginate:
            // I should collect data (Product struct + shop_slug) from each page, then aggregate.
            
            // Let's refactor the loop to collect PRODUCTS directly.
            
            for c in current_cards {
                 if collected_items >= limit {
                    break;
                 }

                 let link = match c.attr("href").await {
                    Ok(opt) => opt.unwrap_or_default(),
                    Err(_) => String::new(),
                };

                // println!("Shopee link found: {}", link);
                if link.starts_with("/") && !link.contains("find_similar_products") {
                     first_cards_collected.push(c); // Store element to extract later? NO. Stale.
                     // We MUST process it now. But wait, `c` is a WebElement on current page.
                     // The loop later `for c in first_cards.into_iter().take(20)` iterates WebElements.
                     // This means the migration to pagination requires a bigger refactor of the code below.
                     // The code below expects `first_cards` to be `Vec<WebElement>`.
                     // BUT if we navigate to page 2, the page 1 elements are invalid.
                     // So we cannot store `Vec<WebElement>` from multiple pages.
                     
                     // We have to extract ALL info here and store in an intermediate struct, NOT WebElement.
                     collected_items += 1;
                }
            }
            
            if collected_items >= limit {
                break;
            }

            // Check next page
             let next_button_xpath = "//a[contains(@class, 'shopee-icon-button--right') and not(contains(@class, 'shopee-icon-button--disabled'))]";
             if let Ok(next_btn) = driver.find(By::XPath(next_button_xpath)).await {
                 if next_btn.is_displayed().await.unwrap_or(false) {
                     let _ = next_btn.click().await;
                     page += 1;
                     // Wait load
                     tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                     continue;
                 }
             }
             
             // Try URL manipulation if button not found or tricky
             // Check if we can just go to next page
             page += 1;
             let next_url = format!(
                "https://shopee.co.id/search?keyword={}&page={}",
                urlencoding::encode(first_query),
                page
            );
             // Verify if we are already at this page or end?
             // Since we count `collected_items`, maybe we just try to go next page.
             let _ = driver.goto(&next_url).await;
             tokio::time::sleep(std::time::Duration::from_secs(3)).await;
             
             // Check if we found products
              if driver.find(By::Css(".shopee-search-item-result__item")).await.is_err() {
                  break; // No more items
              }
        }

        // Wait, if I cannot save WebElement, I must rewrite the processing logic to not use WebElement later.
        // The current structure is:
        // 1. Get `first_cards` (Vec<WebElement>).
        // 2. Loop `first_cards` -> extract `Product`, `shop_slugs`, `first_products_map`.
        // 3. Loop `shop_slugs` -> process other queries.

        // So I need to:
        // 1. Create a loop that visits pages.
        // 2. Inside loop, find elements, extract `Product` immediately.
        // 3. Store `Product` in `all_extracted_products`.
        // 4. After loop, populate `shop_slugs`, `shop_names`, `first_products_map` from `all_extracted_products`.
        
        // Let's restart the logic for this function section.
        
        let mut shop_slugs: HashSet<String> = HashSet::new();
        let mut shop_names: HashMap<String, String> = HashMap::new();
        
        let mut first_products_map: HashMap<String, Vec<Product>> = HashMap::new();
        let mut all_products: Vec<Product> = Vec::new();

        // Reset to page 0 for extraction loop if we moved?
        // Actually, better to do the extraction INSIDE the pagination loop.
        
        // Re-navigate to start to be safe
         let first_url = format!(
            "https://shopee.co.id/search?keyword={}",
            urlencoding::encode(first_query)
        );
        let _ = driver.goto(&first_url).await;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let mut extracted_count = 0;
        let mut current_page = 0;

        loop {
            // Get current cards
             let current_cards = match driver
                .find_all(By::Css(".shopee-search-item-result__item a"))
                .await
            {
                Ok(v) => v,
                Err(_) => Vec::new(),
            };

            for c in current_cards {
                if extracted_count >= limit {
                    break;
                }

                let link = match c.attr("href").await {
                    Ok(opt) => opt.unwrap_or_default(),
                    Err(_) => String::new(),
                };

                 // println!("Shopee link found: {}", link);
                //link bukan /find_similar_products
                if link.starts_with("/") && !link.contains("find_similar_products") {
                    let full_link = format!("https://shopee.co.id{}", link);

                    // Extract product name
                    let name = match c.find(By::Css(".line-clamp-2.break-words")).await {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };

                    // Extract price - Shopee has a specific structure for prices
                    let price = {
                        let price_element = match c
                            .find(By::Css(
                                "[data-testid=\"a11y-label\"] + div .truncate.text-base\\/5.font-medium",
                            ))
                            .await
                        {
                            Ok(el) => el,
                            Err(_) => {
                                // Alternative selector for price
                                match c
                                    .find(By::Css(
                                        ".text-shopee-primary .truncate.text-base\\/5.font-medium",
                                    ))
                                    .await
                                {
                                    Ok(el) => el,
                                    Err(_) => {
                                        // Try another approach
                                        match c.find(By::Css(".flex-shrink.min-w-0.mr-1.truncate.text-shopee-primary .truncate.text-base\\/5.font-medium")).await {
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

                    // Extract location
                    let location = match c
                        .find(By::Css(
                            ".text-shopee-black54.font-extralight.text-sp10 .align-middle",
                        ))
                        .await
                    {
                        Ok(el) => el.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };

                    // Extract photo
                    let photo = match c
                        .find(By::Css(
                            "img.w-full",
                        ))
                        .await
                    {
                        Ok(el) => el.attr("src").await.unwrap_or(None).unwrap_or_default(),
                        Err(_) => {
                            // Try to get the first image in the product card
                            match c.find(By::Css("img")).await {
                                Ok(el) => el.attr("src").await.unwrap_or(None).unwrap_or_default(),
                                Err(_) => String::new(),
                            }
                        }
                    };

                    //contoh link shopee https://shopee.co.id/100ribu-dapat-4pcs-Celana-Pendek-Babytery-Calana-Pria-Resleting-Premium-Celana-Running-4pcs-i.124455053.29705222804 maka angka 124455053 adalah shop id
                    let shop_id = if let Some(rest) = full_link.strip_prefix("https://shopee.co.id/") {
                        // Buang query params
                        let path_only = rest.split('?').next().unwrap_or(rest);

                        if let Some((_, ids_part)) = path_only.rsplit_once("-i.") {
                            if let Some((shop_id_str, _)) = ids_part.split_once('.') {
                                shop_id_str.to_string()
                            } else {
                                "0".to_string()
                            }
                        } else {
                            "0".to_string()
                        }
                    } else {
                        "0".to_string()
                    };


                     // Store shop info
                    if !shop_id.is_empty() {
                        shop_slugs.insert(shop_id.clone());
                        shop_names.insert(shop_id.clone(), shop_id.clone());
                    }

                    let product = Product {
                        name,
                        price: format!("Rp{}", price), // Add currency prefix
                        shop: shop_id.clone(), // Placeholder, actual shop name can be set later
                        location,
                        photo,
                        link: full_link,
                    };
                    
                    // Add to collections
                    all_products.push(product.clone());
                    first_products_map
                        .entry(shop_id.clone())
                        .or_insert_with(Vec::new)
                        .push(product);
                        
                    extracted_count += 1;
                }
            }
            
            if extracted_count >= limit {
                break;
            }
            
            // Go to next page
            current_page += 1;
            let next_url = format!(
                "https://shopee.co.id/search?keyword={}&page={}",
                urlencoding::encode(first_query),
                current_page
            );
            
            // Navigate
             let _ = driver.goto(&next_url).await;
             tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            
            // Check emptiness
            if driver.find(By::Css(".shopee-search-item-result__item")).await.is_err() {
                 break;
            }
        }

        let mut grouped: Vec<ShopResults> = Vec::new();

        for slug in shop_slugs.into_iter() {
            let shop_url = format!("https://shopee.co.id/?shop={}", slug); // This might need adjustment since Shopee shop URLs are structured differently
            let shop_display = shop_names
                .get(&slug)
                .cloned()
                .unwrap_or_else(|| slug.clone());

            let mut qresults: Vec<QueryResult> = Vec::new();

           
            for q in queries.iter() {
                let mut products: Vec<Product> = Vec::new();

                

                // First query products don't need to be searched again in the shop
                if q == first_query {
                    if let Some(v) = first_products_map.get(&slug) {
                        products = v.clone();
                    }
                } else {
                    // For Shopee, we need to go back to the search page and search for the new query
                    let search_url = format!(
                        "https://shopee.co.id/search?keyword={}&shop={}",
                        urlencoding::encode(&q),
                        slug
                    );
                    let _ = driver.goto(&search_url).await;
                    info!("Shopee search URL: {}", search_url);

                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                    // Wait for products to load
                    {
                        use std::time::Duration as StdDuration;
                        let start = std::time::Instant::now();
                        let timeout = StdDuration::from_secs(6);
                        loop {
                            if driver
                                .find(By::Css(".shopee-search-item-result__item"))
                                .await
                                .is_ok()
                            {
                                info!("Shopee products found");
                                break;
                            }

                            if start.elapsed() >= timeout {
                                // timed out waiting
                                info!("Timed out waiting for Shopee products to load");
                                break;
                            }

                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    }

                    // Get Shopee cards
                    let cards = match driver
                        .find_all(By::Css(".shopee-search-item-result__item"))
                        .await
                    {
                        Ok(v) => v,
                        Err(_) => Vec::new(),
                    };

                    for c in cards.into_iter().take(limit) { // limit here too logic
                        let link_element = match c.find(By::Css("a.contents")).await {
                            Ok(el) => el,
                            Err(_) => continue, // Skip if no link found
                        };

                        let link = match link_element.attr("href").await {
                            Ok(opt) => opt.unwrap_or_default(),
                            Err(_) => String::new(),
                        };

                        if link.starts_with("/") && !link.contains("find_similar_products") {
                            let full_link = format!("https://shopee.co.id{}", link);

                            // Extract product name
                            let name = match c.find(By::Css(".line-clamp-2.break-words")).await {
                                Ok(el) => el.text().await.unwrap_or_default(),
                                Err(_) => String::new(),
                            };

                            // Extract price
                            let price = {
                                let price_element = match c.find(By::Css("[data-testid=\"a11y-label\"] + div .truncate.text-base\\/5.font-medium")).await {
                                    Ok(el) => el,
                                    Err(_) => {
                                        // Alternative selector for price
                                        match c.find(By::Css(".text-shopee-primary .truncate.text-base\\/5.font-medium")).await {
                                            Ok(el) => el,
                                            Err(_) => {
                                                // Try another approach
                                                match c.find(By::Css(".flex-shrink.min-w-0.mr-1.truncate.text-shopee-primary .truncate.text-base\\/5.font-medium")).await {
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

                            // Extract location
                            let location = match c
                                .find(By::Css(
                                    ".text-shopee-black54.font-extralight.text-sp10 .align-middle",
                                ))
                                .await
                            {
                                Ok(el) => el.text().await.unwrap_or_default(),
                                Err(_) => String::new(),
                            };

                            // Extract photo
                            let photo = match c
                                .find(By::Css(
                                    "img.w-full",
                                ))
                                .await
                            {
                                Ok(el) => el.attr("src").await.unwrap_or(None).unwrap_or_default(),
                                Err(_) => {
                                    // Try to get the first image in the product card
                                    match c.find(By::Css("img")).await {
                                        Ok(el) => {
                                            el.attr("src").await.unwrap_or(None).unwrap_or_default()
                                        }
                                        Err(_) => String::new(),
                                    }
                                }
                            };

                            // Get shop info from product detail page
                            // let (shop_name, _shop_url) = Self::get_shop_info_from_product(driver, &full_link).await;

                            products.push(Product {
                                name,
                                price: format!("Rp{}", price),
                                shop: slug.clone(),
                                location,
                                photo,
                                link: full_link,
                            });
                        }
                    }
                }

                qresults.push(QueryResult {
                    query: q.clone(),
                    products,
                });
            }

            if !qresults.is_empty() && !qresults[0].products.is_empty() {
                let first_link = qresults[0].products[0].link.clone();
                let (shop_name, new_shop_url) = Self::get_shop_info_from_product(driver, &first_link).await;

                let shop_result = ShopResults {
                    shop_name: shop_name.clone(),
                    shop_url: new_shop_url.clone(),
                    platform: "shopee".to_string(),
                    results: qresults,
                };

                // Emit progress real-time
                let _ = window
                    .emit("scrape:progress", shop_result.clone())
                    .map_err(|e| e.to_string());

                grouped.push(shop_result);
            }
        }

        // Emit done
        let _ = window.emit("scrape:done", ()).map_err(|e| e.to_string());

        Ok(grouped)
    }

    /// Helper method to extract shop info from product detail page
    /// Visits the product page and finds shop info in .page-product__shop element
    async fn get_shop_info_from_product(driver: &WebDriver, product_url: &str) -> (String, String) {
        // Navigate to product detail page
        if let Err(_) = driver.goto(product_url).await {
            info!("Failed to navigate to product page: {}", product_url);
            return (String::new(), String::new());
        }

        // Wait a bit for page to load
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        // Try to find the shop element
        let shop_element = match driver.find(By::Css(".page-product__shop")).await {
            Ok(el) => el,
            Err(_) => {
                info!("Could not find .page-product__shop element on page: {}", product_url);
                return (String::new(), String::new());
            }
        };

        // Find the <a> tag inside the shop element
        let shop_link = match shop_element.find(By::Css("a")).await {
            Ok(el) => el,
            Err(_) => {
                info!("Could not find <a> tag inside .page-product__shop");
                return (String::new(), String::new());
            }
        };

        // Extract shop name (div sibiling tag a -> div -> text)
        let mut shop_name = match shop_element.find(By::XPath(".//a/following-sibling::div//div")).await {
            Ok(el) => el.text().await.unwrap_or_default(),
            Err(_) => String::new(),
        };

        if shop_name.is_empty() {
            let candidates = match shop_element
                .find_all(By::XPath(".//a/following-sibling::div//div"))
                .await
            {
                Ok(list) => list,
                Err(_) => vec![],
            };

            for el in candidates {
                let text = el.text().await.unwrap_or_default();

                if !text.trim().is_empty()
                    && !text.to_lowercase().contains("aktif")
                    && !text.to_lowercase().contains("chat")
                {
                    shop_name = text;
                    break;
                }
            }
        }

        // Extract shop URL (href attribute)
        let shop_url = match shop_link.attr("href").await {
            Ok(Some(href)) => {
                if href.starts_with("/") {
                    format!("https://shopee.co.id{}", href)
                } else if href.starts_with("http") {
                    href
                } else {
                    format!("https://shopee.co.id/{}", href)
                }
            }
            _ => String::new(),
        };

        info!("Extracted shop info - Name: {}, URL: {}", shop_name, shop_url);
        
        (shop_name, shop_url)
    }

    async fn perform_site_search(driver: &WebDriver, query: &str) -> Result<(), ()> {
        // Cari input pada Shopee
        let sel = r#"input[type=\"text\"][class*=\"shopee-search-input__input\"]"#;
        if let Ok(el) = driver.find(By::Css(sel)).await {
            if el.is_displayed().await.unwrap_or(false) {
                let _ = el.click().await;
                let _ = el.clear().await;
                let _ = el.send_keys(query).await;
                // Submit using enter key
                let _ = el.send_keys("\n").await;
                return Ok(());
            }
        }
        Err(())
    }
}
