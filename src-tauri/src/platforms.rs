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
    ) -> Result<Vec<ShopResults>, String> {
        info!("Starting Tokopedia scraping with original logic");

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

        // Get first cards
        let first_cards = match driver
            .find_all(By::Css("div[data-ssr=\"contentProductsSRPSSR\"] a"))
            .await
        {
            Ok(v) => v,
            Err(_) => Vec::new(),
        };

        let mut shop_slugs: HashSet<String> = HashSet::new();
        let mut shop_names: HashMap<String, String> = HashMap::new();
        let mut first_products_map: HashMap<String, Vec<Product>> = HashMap::new();

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
                        let spans = match c
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
                if q == first_query {
                    if let Some(v) = first_products_map.get(&slug) {
                        products = v.clone();
                    }
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

                    let cards = match driver
                        .find_all(By::Css(
                            r#"[data-ssr="shopSSR"] > div:nth-child(2) a[data-theme="default"]"#,
                        ))
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

                        let spans = match c
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
    ) -> Result<Vec<ShopResults>, String> {
        info!("Starting Shopee scraping with original logic");

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

        // Get first cards
        let first_cards = match driver
            .find_all(By::Css(".shopee-search-item-result__item a"))
            .await
        {
            Ok(v) => v,
            Err(_) => Vec::new(),
        };

        let mut shop_slugs: HashSet<String> = HashSet::new();
        let mut shop_names: HashMap<String, String> = HashMap::new();
        
        let mut first_products_map: HashMap<String, Vec<Product>> = HashMap::new();

        let mut all_products: Vec<Product> = Vec::new();

        for c in first_cards.into_iter().take(20) {
            let link = match c.attr("href").await {
                Ok(opt) => opt.unwrap_or_default(),
                Err(_) => String::new(),
            };

            println!("Shopee link found: {}", link);
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

                // //contoh link shopee ada Celana-Running-4pcs-i.124455053.29705222804 maka angka 124455053 adalah shop id
                // let shop_slug = if let Some(rest) = link.strip_prefix("/shop/") {
                //     if let Some((slug, _)) = rest.split_once('/') {
                //         slug.to_string()
                //     } else {
                //         "shopee".to_string()
                //     }
                // } else if let Some(rest) = link.strip_prefix("/product/") {
                //     if let Some((shop_id_part, _)) = rest.split_once('.') {
                //         shop_id_part.to_string()
                //     } else {
                //         "shopee".to_string()
                //     }
                // } else {
                //     "shopee".to_string()
                // };

                // // Get shop info from product detail page
                // let (shop_name, shop_url) = Self::get_shop_info_from_product(driver, &full_link).await;
                
                // // Extract shop slug from shop URL or use shop name as fallback
                // let shop_slug = if !shop_url.is_empty() {
                //     // Extract slug from URL like https://shopee.co.id/shop/12345678
                //     shop_url
                //         .trim_end_matches('/')
                //         .split('/')
                //         .last()
                //         .unwrap_or("shopee")
                //         .to_string()
                // } else {
                //     // Use sanitized shop name as slug
                //     shop_name
                //         .to_lowercase()
                //         .replace(" ", "_")
                //         .chars()
                //         .filter(|c| c.is_alphanumeric() || *c == '_')
                //         .collect::<String>()
                // };

                // // Store shop info
                // if !shop_slug.is_empty() && !shop_name.is_empty() {
                //     shop_slugs.insert(shop_slug.clone());
                //     shop_names.insert(shop_slug.clone(), shop_name.clone());
                // }

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
                all_products.push(product.clone());

                first_products_map
                    .entry(shop_id.clone())
                    .or_insert_with(Vec::new)
                    .push(product);
            }
        }


        // //find shop slugs from all_products link
        // for product in all_products.iter() {
        //     // Get shop info from product detail page
        //     let (shop_name, shop_url) = Self::get_shop_info_from_product(driver, &product.link).await;
            
        //     // Extract shop slug from shop URL or use shop name as fallback
        //     let shop_slug = if !shop_url.is_empty() {
        //         // Extract slug from URL like https://shopee.co.id/shop/12345678
        //         shop_url
        //             .trim_end_matches('/')
        //             .split('/')
        //             .last()
        //             .unwrap_or("shopee")
        //             .to_string()
        //     } else {
        //         // Use sanitized shop name as slug
        //         shop_name
        //             .to_lowercase()
        //             .replace(" ", "_")
        //             .chars()
        //             .filter(|c| c.is_alphanumeric() || *c == '_')
        //             .collect::<String>()
        //     };

        //     //contoh link shopee https://shopee.co.id/100ribu-dapat-4pcs-Celana-Pendek-Babytery-Calana-Pria-Resleting-Premium-Celana-Running-4pcs-i.124455053.29705222804 maka angka 124455053 adalah shop id
        //     let shop_id = if let Some(rest) = shop_url.strip_prefix("https://shopee.co.id/") {
        //         if let Some((_, shop_id_part)) = rest.rsplit_once('.') {
        //             if let Some((shop_id_str, _)) = shop_id_part.split_once('.') {
        //                 shop_id_str.to_string()
        //             } else {
        //                 "0".to_string()
        //             }
        //         } else {
        //             "0".to_string()
        //         }
        //     } else {
        //         "0".to_string()
        //     };

        //     // Store shop info
        //     if !shop_slug.is_empty() && !shop_name.is_empty() {
        //         shop_slugs.insert(shop_slug.clone());
        //         shop_names.insert(shop_slug.clone(), shop_name.clone());
        //     }

        //     let new_shop_slug = format!("{}?shop_id={}", shop_slug, shop_id);

        //     first_products_map
        //         .entry(new_shop_slug)
        //         .or_insert_with(Vec::new)
        //         .push(product.clone());
        // }

        let mut grouped: Vec<ShopResults> = Vec::new();

        for slug in shop_slugs.into_iter() {
            let shop_url = format!("https://shopee.co.id/?shop={}", slug); // This might need adjustment since Shopee shop URLs are structured differently
            let shop_display = shop_names
                .get(&slug)
                .cloned()
                .unwrap_or_else(|| slug.clone());

            let mut qresults: Vec<QueryResult> = Vec::new();

            // let shop_id = if let Some(pos) = slug.find("?shop_id=") {
            //     &slug[pos + 9..]
            // } else {
            //     "0"
            // };
            

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

                    for c in cards.into_iter().take(20) {
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
