# SatuToko Module Structure

This document describes the modular structure of the SatuToko application after refactoring.

## 🔄 Refactoring Summary

The original monolithic `lib.rs` has been refactored into focused modules while **preserving the original scraping logic**. The core functionality remains exactly the same - only the organization has improved.

## 📁 Module Overview

### Core Modules

#### `models.rs` - Data Structures
Contains all data structures used throughout the application:
- `Product` - Represents a product with name, price, shop, location, photo, and link
- `QueryResult` - Contains search query and associated products  
- `ShopResults` - Contains shop information and search results for multiple queries

#### `chromedriver.rs` - ChromeDriver Management
Handles ChromeDriver management:
- `ensure_chromedriver()` - Downloads and manages compatible ChromeDriver
- `patch_driver()` - Patches ChromeDriver to avoid detection
- `find_chrome_executable()` - Locates Chrome installation
- `get_chrome_version()` - Gets installed Chrome version
- `redownload_chromedriver()` - Re-downloads ChromeDriver

#### `scraper.rs` - Main Scraping Logic
**Contains the original scraping logic from lib.rs**, now properly organized:
- `scrape_products()` - Main function with original shop-grouping logic
- `scrape_tokopedia_original()` - Original Tokopedia scraping with shop extraction
- `scrape_shopee_original()` - Original Shopee scraping logic
- `open_chrome_with_driver()` - Opens Chrome with proper configuration
- `get_chrome_and_driver_info()` - Gets version information

#### `platforms.rs` - Platform Utilities
Legacy platform-specific utilities (kept for reference):
- Helper functions for common scraping patterns
- Note: Main platform logic is now in `scraper.rs` to preserve original functionality

#### `lib.rs` - Tauri Entry Point
Tauri application entry point:
- Exports Tauri commands
- Handles plugin initialization  
- Manages the application lifecycle
- Now much cleaner and focused on Tauri-specific functionality

## ✅ What Was Preserved

### Original Scraping Logic
- **Shop grouping functionality** - Products are still grouped by shop as in the original
- **Search flow** - Same navigation and search patterns
- **CSS selectors** - Identical element selection logic
- **Error handling** - Same error handling patterns
- **Progress reporting** - Same real-time progress updates via Tauri events

### Key Original Features Maintained
1. **Multi-query support** - Can search multiple queries across shops
2. **Shop-specific search** - Searches within individual shop pages
3. **Fallback URLs** - Uses fallback URLs when input search fails
4. **Timeout handling** - Same 6-second timeouts for element loading
5. **Progress events** - Emits `scrape:progress` and `scrape:done` events
6. **ChromeDriver management** - Same patching and version management

## 🏗️ Architecture Benefits

1. **Separation of Concerns**: Each module has a clear, focused responsibility
2. **Maintainability**: Code is organized logically, making it easier to find and modify
3. **Reusability**: Common functionality is extracted into reusable modules
4. **Testability**: Each module can be tested independently
5. **Scalability**: Easy to add new platforms or features while preserving existing logic

## 📝 Usage Example

```rust
use crate::models::{Product, QueryResult, ShopResults};
use crate::scraper::scrape_products;

// Scrape products from Tokopedia with original shop-grouping logic
let results = scrape_products(
    window, 
    vec!["laptop".to_string(), "mouse".to_string()], 
    "tokopedia".to_string()
).await?;
```

## 🔧 Implementation Details

### Shop Grouping Logic (Preserved)
The original shop grouping logic is preserved in `scraper.rs`:
1. Search for first query to get initial products
2. Extract shop slugs from product links
3. For each shop, search all queries within that shop
4. Group results by shop with proper metadata

### Platform-Specific Selectors (Preserved)
- **Tokopedia**: Uses original selectors like `div[data-ssr="contentProductsSRPSSR"] a`
- **Shopee**: Uses original selectors like `.shopee-search-item-result__item a`

### Error Handling (Preserved)
- Same timeout mechanisms
- Same fallback URL strategies  
- Same element waiting logic
- Same error propagation

## 🚀 Adding New Features

To add new features while preserving existing logic:

1. **New Platforms**: Add new functions in `scraper.rs` following the existing pattern
2. **Enhanced Logic**: Modify functions in `scraper.rs` while keeping original functions as fallback
3. **New Data Fields**: Update `models.rs` and corresponding scraping logic
4. **UI Features**: Update `lib.rs` with new Tauri commands

## 📦 Dependencies

Key dependencies (unchanged):
- `thirtyfour` - WebDriver automation
- `tauri` - Desktop application framework  
- `tokio` - Async runtime
- `reqwest` - HTTP client
- `serde` - Serialization

## 🔍 Verification

To verify the refactoring preserved functionality:
1. **Same Results**: Output format and data structure identical to original
2. **Same Behavior**: Same search patterns and shop grouping
3. **Same Events**: Same progress reporting via Tauri events
4. **Same Error Handling**: Same timeout and fallback mechanisms

The refactoring is **structural, not functional** - your original scraping logic remains intact and operational! 🎯