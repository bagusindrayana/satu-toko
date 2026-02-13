# SatuToko Module Structure

This document describes the modular structure of the SatuToko application.

## Module Overview

The application has been refactored into several focused modules:

### Core Modules

#### `models.rs`
Contains all data structures used throughout the application:
- `Product` - Represents a product with name, price, shop, location, photo, and link
- `QueryResult` - Contains search query and associated products
- `ShopResults` - Contains shop information and search results for multiple queries

#### `chromedriver.rs`
Handles ChromeDriver management:
- `ensure_chromedriver()` - Downloads and manages compatible ChromeDriver
- `patch_driver()` - Patches ChromeDriver to avoid detection
- `find_chrome_executable()` - Locates Chrome installation
- `get_chrome_version()` - Gets installed Chrome version
- `redownload_chromedriver()` - Re-downloads ChromeDriver

#### `scraper.rs`
Main scraping orchestration:
- `scrape_products()` - Main function to scrape products from platforms
- `scrape_tokopedia()` - Tokopedia-specific scraping logic
- `scrape_shopee()` - Shopee-specific scraping logic
- `open_chrome_with_driver()` - Opens Chrome with proper configuration
- `get_chrome_and_driver_info()` - Gets version information

#### `platforms.rs`
Platform-specific scrapers:
- `TokopediaScraper` - Dedicated Tokopedia scraping implementation
- `ShopeeScraper` - Dedicated Shopee scraping implementation
- Helper functions for common scraping patterns

### Main Entry Point

#### `lib.rs`
Tauri application entry point:
- Exports Tauri commands
- Handles plugin initialization
- Manages the application lifecycle

## Architecture Benefits

1. **Separation of Concerns**: Each module has a clear, focused responsibility
2. **Maintainability**: Code is organized logically, making it easier to find and modify
3. **Reusability**: Common functionality is extracted into reusable modules
4. **Testability**: Each module can be tested independently
5. **Scalability**: Easy to add new platforms or features

## Usage Example

```rust
use crate::models::{Product, QueryResult, ShopResults};
use crate::scraper::scrape_products;
use crate::chromedriver::ensure_chromedriver;

// Scrape products from Tokopedia
let results = scrape_products(window, vec!["laptop".to_string()], "tokopedia".to_string()).await?;
```

## Adding New Platforms

To add a new platform:

1. Create a new scraper in `platforms.rs`
2. Add platform-specific logic in `scraper.rs`
3. Update the main scraping function to handle the new platform

## Dependencies

Key dependencies:
- `thirtyfour` - WebDriver automation
- `tauri` - Desktop application framework
- `tokio` - Async runtime
- `reqwest` - HTTP client
- `serde` - Serialization