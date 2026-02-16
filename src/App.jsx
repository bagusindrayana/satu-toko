import React, { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./index.css";

function App() {
  const [tags, setTags] = useState([]);
  const [input, setInput] = useState("");
  const [results, setResults] = useState([]);
  const [loading, setLoading] = useState(false);
  const [showDriverModal, setShowDriverModal] = useState(false);
  const [chromeInfo, setChromeInfo] = useState({
    chromeVersion: "",
    driverVersion: "",
  });
  const [infoLoading, setInfoLoading] = useState(false);
  const [expandedShops, setExpandedShops] = useState({}); // Track expanded shops
  const [expandedQueries, setExpandedQueries] = useState({}); // Track expanded queries
  const [selectedPlatform, setSelectedPlatform] = useState("tokopedia"); // Default to tokopedia
  const [showCopyNotification, setShowCopyNotification] = useState(false); // Track copy notification
  const [chromeProfilePath, setChromeProfilePath] = useState(""); // Chrome profile path
  const [profileSaving, setProfileSaving] = useState(false); // Track save state
  const [showHistoryModal, setShowHistoryModal] = useState(false); // History modal state
  const [searchHistory, setSearchHistory] = useState([]); // Search history data
  const inputRef = useRef(null);
  const listenersRef = useRef([]);
  const HISTORY_STORAGE_KEY = "satu-toko-search-history";

  // Load search history from localStorage on mount
  useEffect(() => {
    try {
      const savedHistory = localStorage.getItem(HISTORY_STORAGE_KEY);
      if (savedHistory) {
        setSearchHistory(JSON.parse(savedHistory));
      }
    } catch (e) {
      console.error("Failed to load search history:", e);
    }
  }, []);

  useEffect(() => {
    let unlistenProgress = null;
    let unlistenDone = null;

    (async () => {
      try {
        unlistenProgress = await listen("scrape:progress", (event) => {
          const shop = event.payload;
          setResults((prev) => {
            // replace shop result
            const idx = prev.findIndex((s) => s.shop_url === shop.shop_url);
            if (idx >= 0) {
              const copy = [...prev];
              copy[idx] = shop;
              return copy;
            }
            return [...prev, shop];
          });
        });

        unlistenDone = await listen("scrape:done", () => {
          setLoading(false);
        });
        listenersRef.current.push(unlistenProgress, unlistenDone);
      } catch (e) {
        console.error("Failed to subscribe to scrape events", e);
      }
    })();

    return () => {
      listenersRef.current.forEach((fn) => fn && fn());
      listenersRef.current = [];
    };
  }, []);

  function addTagFromInput() {
    const v = input.trim();
    if (!v) return;
    setTags((t) => Array.from(new Set([...t, v])));
    setInput("");
    inputRef.current && inputRef.current.focus();
  }

  function removeTag(idx) {
    setTags((t) => t.filter((_, i) => i !== idx));
  }

  // Save search history to localStorage
  const saveSearchHistory = (queries, platform, results) => {
    try {
      const totalResults = results.reduce((sum, shop) => {
        return (
          sum +
          (shop.results
            ? shop.results.reduce(
                (s, r) => s + (r.products ? r.products.length : 0),
                0,
              )
            : 0)
        );
      }, 0);

      const newEntry = {
        id: Date.now(),
        timestamp: new Date().toISOString(),
        queries: [...queries],
        platform,
        resultCount: totalResults,
        results: results, // Store full results data
      };

      const updatedHistory = [newEntry, ...searchHistory].slice(0, 20); // Keep last 20 searches (karena data besar)
      setSearchHistory(updatedHistory);
      localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(updatedHistory));
    } catch (e) {
      console.error("Failed to save search history:", e);
    }
  };

  // Load a history entry
  const loadHistoryEntry = (entry) => {
    setTags(entry.queries);
    setSelectedPlatform(entry.platform);
    setResults(entry.results || []); // Load full results
    setExpandedShops({}); // Reset expanded state
    setExpandedQueries({}); // Reset expanded queries
    setShowHistoryModal(false);
  };

  // Delete a history entry
  const deleteHistoryEntry = (id) => {
    try {
      const updatedHistory = searchHistory.filter((entry) => entry.id !== id);
      setSearchHistory(updatedHistory);
      localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(updatedHistory));
    } catch (e) {
      console.error("Failed to delete history entry:", e);
    }
  };

  // Clear all history
  const clearAllHistory = () => {
    try {
      setSearchHistory([]);
      localStorage.removeItem(HISTORY_STORAGE_KEY);
    } catch (e) {
      console.error("Failed to clear history:", e);
    }
  };

  async function onSearch() {
    if (tags.length === 0) return;
    setResults([]);
    setLoading(true);
    try {
      // invoke backend
      const res = await invoke("scrape_products", {
        queries: tags,
        platform: selectedPlatform,
      });
      setResults(res);
      // Save to history after successful search (with full results data)
      saveSearchHistory(tags, selectedPlatform, res);
      setLoading(false);
    } catch (e) {
      console.error(e);
      alert("Error during scraping: " + String(e));
      setLoading(false);
    }
  }

  async function loadChromeInfo() {
    setInfoLoading(true);
    try {
      const [chromeVersion, driverVersion] = await invoke(
        "get_chrome_and_driver_info",
      );
      setChromeInfo({ chromeVersion, driverVersion });
    } catch (e) {
      console.error(e);
      alert("Failed to get Chrome/ChromeDriver info: " + String(e));
    } finally {
      setInfoLoading(false);
    }
  }

  async function onOpenDriver() {
    // Show modal instead of opening folder directly
    setShowDriverModal(true);
    loadChromeInfo();
    loadChromeProfilePath();
  }

  async function loadChromeProfilePath() {
    try {
      const path = await invoke("get_chrome_profile_path");
      setChromeProfilePath(path || "");
    } catch (e) {
      console.error("Failed to load Chrome profile path:", e);
    }
  }

  async function saveChromeProfilePath() {
    try {
      setProfileSaving(true);
      await invoke("set_chrome_profile_path", { path: chromeProfilePath });
      alert("Chrome profile path saved successfully!");
    } catch (e) {
      console.error(e);
      alert("Failed to save Chrome profile path: " + String(e));
    } finally {
      setProfileSaving(false);
    }
  }

  async function onReDownload() {
    try {
      setInfoLoading(true);
      await invoke("redownload_chromedriver");
      alert("ChromeDriver re-downloaded successfully!");
      loadChromeInfo(); // Refresh the version info
    } catch (e) {
      console.error(e);
      alert("Failed to re-download ChromeDriver: " + String(e));
    } finally {
      setInfoLoading(false);
    }
  }

  async function onOpenShopee() {
    try {
      await invoke("open_chrome_with_driver", {
        url: "https://shopee.co.id/buyer/login",
      });
      alert("Browser opened with ChromeDriver!");
    } catch (e) {
      console.error(e);
      alert("Failed to open browser: " + String(e));
    }
  }

  async function onOpenTokopedia() {
    try {
      await invoke("open_chrome_with_driver", {
        url: "https://www.tokopedia.com/login",
      });
      alert("Browser opened with ChromeDriver!");
    } catch (e) {
      console.error(e);
      alert("Failed to open browser: " + String(e));
    }
  }

  function closeModal() {
    setShowDriverModal(false);
  }

  // Function to toggle shop expansion
  const toggleShop = (shopIndex) => {
    setExpandedShops((prev) => ({
      ...prev,
      [shopIndex]: !prev[shopIndex],
    }));
  };

  // Function to toggle query expansion
  const toggleQuery = (shopIndex, queryIndex) => {
    const key = `${shopIndex}-${queryIndex}`;
    setExpandedQueries((prev) => ({
      ...prev,
      [key]: !prev[key],
    }));
  };

  // Function to handle copy to clipboard
  const handleCopyLink = async (e, url) => {
    e.preventDefault();
    try {
      await navigator.clipboard.writeText(url);
      setShowCopyNotification(true);
      setTimeout(() => {
        setShowCopyNotification(false);
      }, 2000); // Hide notification after 2 seconds
    } catch (err) {
      console.error("Failed to copy:", err);
      alert("Gagal menyalin link");
    }
  };

  // Function to handle print
  const handlePrint = async () => {
    try {
      // Call Tauri backend to create HTML file
      const filePath = await invoke("create_print_html", { results });

      // Open the file with default browser
      await invoke("open_file_with_default_app", { path: filePath });

      alert("File HTML untuk print telah dibuka di browser default Anda");
    } catch (e) {
      console.error(e);
      alert("Gagal membuat file print: " + String(e));
    }
  };

  // Function to handle export to Excel
  const handleExportExcel = async () => {
    try {
      // Call Tauri backend to export CSV
      const filePath = await invoke("export_to_excel", { results });

      alert(`File berhasil disimpan di:\n${filePath}`);

      // Open the folder containing the file
      const folderPath = filePath.substring(0, filePath.lastIndexOf("\\"));
      await invoke("open_file_with_default_app", { path: folderPath });
    } catch (e) {
      console.error(e);
      alert("Gagal export ke Excel: " + String(e));
    }
  };

  return (
    <div className="app-container">
      <div className="app-window">
        {/* Title Bar */}
        <div className="title-bar">
          <div className="title-bar-content">
            <h1 className="app-title">Satu Toko — Scraper</h1>
            <div style={{ display: "flex", gap: "8px" }}>
              <button
                onClick={() => setShowHistoryModal(true)}
                className="btn-secondary"
                style={{ fontSize: "14px", padding: "6px 12px" }}
              >
                Riwayat Pencarian
              </button>
              <button
                onClick={onOpenShopee}
                className="btn-primary"
                style={{ fontSize: "14px", padding: "6px 12px" }}
              >
                Open Shopee
              </button>
              <button
                onClick={onOpenTokopedia}
                className="btn-primary"
                style={{ fontSize: "14px", padding: "6px 12px" }}
              >
                Open Tokopedia
              </button>
              <button onClick={onOpenDriver} className="btn-secondary">
                Chromedriver Setting
              </button>
            </div>
          </div>
        </div>

        {/* History Modal */}
        {showHistoryModal && (
          <div className="modal-overlay">
            <div
              className="modal-window"
              style={{ maxWidth: "600px", maxHeight: "80vh" }}
            >
              <div className="modal-header">
                <h3 className="modal-title">📜 Riwayat Pencarian</h3>
                <button
                  onClick={() => setShowHistoryModal(false)}
                  className="btn-close"
                >
                  ×
                </button>
              </div>

              <div
                className="modal-body"
                style={{ maxHeight: "60vh", overflowY: "auto" }}
              >
                {searchHistory.length === 0 ? (
                  <div className="text-center text-gray-500 py-8">
                    <p>Belum ada riwayat pencarian</p>
                  </div>
                ) : (
                  <div className="history-list">
                    {searchHistory.map((entry) => (
                      <div
                        key={entry.id}
                        className="history-item"
                        style={{
                          border: "1px solid #e5e7eb",
                          borderRadius: "8px",
                          padding: "12px",
                          marginBottom: "12px",
                          backgroundColor: "#f9fafb",
                        }}
                      >
                        <div
                          style={{
                            display: "flex",
                            justifyContent: "space-between",
                            alignItems: "flex-start",
                            marginBottom: "8px",
                          }}
                        >
                          <div>
                            <span
                              style={{
                                backgroundColor:
                                  entry.platform === "tokopedia"
                                    ? "#00aa5b"
                                    : "#ee4d2d",
                                color: "white",
                                padding: "2px 8px",
                                borderRadius: "4px",
                                fontSize: "12px",
                                fontWeight: "bold",
                                textTransform: "uppercase",
                              }}
                            >
                              {entry.platform}
                            </span>
                            <span
                              style={{
                                marginLeft: "8px",
                                fontSize: "12px",
                                color: "#6b7280",
                              }}
                            >
                              {new Date(entry.timestamp).toLocaleString(
                                "id-ID",
                              )}
                            </span>
                          </div>
                          <button
                            onClick={() => deleteHistoryEntry(entry.id)}
                            style={{
                              background: "none",
                              border: "none",
                              color: "#ef4444",
                              cursor: "pointer",
                              fontSize: "18px",
                              padding: "0 4px",
                            }}
                            title="Hapus"
                          >
                            ×
                          </button>
                        </div>

                        <div style={{ marginBottom: "8px" }}>
                          <strong>Keywords:</strong>{" "}
                          {entry.queries.map((q, i) => (
                            <span
                              key={i}
                              style={{
                                backgroundColor: "#e5e7eb",
                                padding: "2px 8px",
                                borderRadius: "4px",
                                fontSize: "12px",
                                marginRight: "4px",
                              }}
                            >
                              {q}
                            </span>
                          ))}
                        </div>

                        <div style={{ marginBottom: "8px" }}>
                          <span style={{ fontSize: "13px", color: "#6b7280" }}>
                            {entry.resultCount} produk •{" "}
                            {entry.results?.length || 0} toko
                          </span>
                        </div>

                        {/* Preview shops */}
                        {entry.results && entry.results.length > 0 && (
                          <div
                            style={{ marginBottom: "8px", fontSize: "12px" }}
                          >
                            <strong>Toko:</strong>
                            <div style={{ marginTop: "4px" }}>
                              {entry.results.slice(0, 3).map((shop, idx) => (
                                <span
                                  key={idx}
                                  style={{
                                    display: "inline-block",
                                    backgroundColor: "#dbeafe",
                                    color: "#1e40af",
                                    padding: "1px 6px",
                                    borderRadius: "4px",
                                    fontSize: "11px",
                                    marginRight: "4px",
                                    marginBottom: "4px",
                                  }}
                                >
                                  {shop.shop_name}
                                </span>
                              ))}
                              {entry.results.length > 3 && (
                                <span
                                  style={{
                                    fontSize: "11px",
                                    color: "#6b7280",
                                  }}
                                >
                                  +{entry.results.length - 3} lainnya
                                </span>
                              )}
                            </div>
                          </div>
                        )}

                        <div
                          style={{
                            display: "flex",
                            justifyContent: "flex-end",
                          }}
                        >
                          <button
                            onClick={() => loadHistoryEntry(entry)}
                            className="btn-primary"
                            style={{ fontSize: "13px", padding: "4px 12px" }}
                          >
                            📂 Load Hasil
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}

                {searchHistory.length > 0 && (
                  <div
                    className="modal-actions"
                    // style={{
                    //   borderTop: "1px solid #e5e7eb",
                    //   paddingTop: "16px",
                    //   marginTop: "16px",
                    // }}
                  >
                    <button
                      onClick={clearAllHistory}
                      className="btn-secondary"
                      style={{ width: "100%", color: "#ef4444" }}
                    >
                      🗑️ Hapus Semua Riwayat
                    </button>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Driver Info Modal */}
        {showDriverModal && (
          <div className="modal-overlay">
            <div className="modal-window">
              <div className="modal-header">
                <h3 className="modal-title">Chromedriver Setting</h3>
                <button onClick={closeModal} className="btn-close">
                  ×
                </button>
              </div>

              <div className="modal-body">
                {infoLoading ? (
                  <div className="loading-state">
                    <p>Loading...</p>
                  </div>
                ) : (
                  <div className="info-grid">
                    <div className="info-item">
                      <label className="info-label">Chrome Version</label>
                      <div className="info-value">
                        {chromeInfo.chromeVersion || "Not detected"}
                      </div>
                    </div>
                    <div className="info-item">
                      <label className="info-label">ChromeDriver Version</label>
                      <div className="info-value">
                        {chromeInfo.driverVersion || "Not downloaded"}
                      </div>
                    </div>
                  </div>
                )}

                <div className="info-grid" style={{ marginTop: "20px" }}>
                  <div className="info-item">
                    <label className="info-label">Chrome Profile Path</label>
                    <div style={{ marginBottom: "8px" }}>
                      <input
                        type="text"
                        value={chromeProfilePath}
                        onChange={(e) => setChromeProfilePath(e.target.value)}
                        placeholder="C:\Users\YourName\AppData\Local\Google\Chrome\User Data\Profile 1"
                        className="form-select"
                        style={{ width: "100%", marginBottom: "8px" }}
                      />
                      <button
                        onClick={saveChromeProfilePath}
                        disabled={profileSaving || !chromeProfilePath.trim()}
                        className="btn-primary"
                        style={{ width: "100%" }}
                      >
                        {profileSaving ? "Saving..." : "Save Profile Path"}
                      </button>
                    </div>
                    <div
                      className="warning-box"
                      style={{ marginTop: "12px", fontSize: "13px" }}
                    >
                      <div className="warning-content">
                        <strong>Cara mendapatkan Profile Path:</strong>
                        <ol
                          style={{
                            marginTop: "8px",
                            marginBottom: "0",
                            paddingLeft: "20px",
                          }}
                        >
                          <li>
                            Buka Chrome dan ketik{" "}
                            <code
                              style={{
                                background: "#f0f0f0",
                                padding: "2px 6px",
                                borderRadius: "3px",
                              }}
                            >
                              chrome://version
                            </code>{" "}
                            di address bar
                          </li>
                          <li>
                            Cari baris <strong>"Profile Path"</strong>
                          </li>
                          <li>Copy path tersebut dan paste di input di atas</li>
                          <li>
                            <em>Disarankan:</em> Buat profile Chrome baru khusus
                            untuk scraping
                          </li>
                        </ol>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="modal-actions">
                  <button
                    onClick={onReDownload}
                    disabled={infoLoading}
                    className="btn-secondary"
                    style={{ width: "100%" }}
                  >
                    Re-Download Driver
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}

        <div className="content-section">
          <div className="form-group">
            <label className="form-label">
              Nama produk (bisa lebih dari satu)
            </label>
            <div className="tag-input">
              {tags.map((t, i) => (
                <span key={i} className="tag">
                  {t}
                  <button onClick={() => removeTag(i)} className="tag-remove">
                    ×
                  </button>
                </span>
              ))}
              <input
                ref={inputRef}
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === ",") {
                    e.preventDefault();
                    addTagFromInput();
                  } else if (
                    e.key === "Backspace" &&
                    input === "" &&
                    tags.length > 0
                  ) {
                    removeTag(tags.length - 1);
                  }
                }}
                placeholder="Ketik nama produk lalu tekan Enter atau koma"
                className="tag-input-field"
              />
            </div>
          </div>

          <div className="form-row">
            <div className="form-group">
              <label className="form-label">Platform</label>
              <select
                value={selectedPlatform}
                onChange={(e) => setSelectedPlatform(e.target.value)}
                className="form-select"
              >
                <option value="tokopedia">Tokopedia</option>
                <option value="shopee">Shopee</option>
              </select>
            </div>
            <div className="form-group">
              <button
                onClick={onSearch}
                disabled={tags.length === 0}
                className="btn-primary btn-search"
              >
                Cari Produk
              </button>
            </div>
          </div>

          {/* Shopee Warning */}
          {selectedPlatform === "shopee" && (
            <div className="warning-box">
              <div className="warning-icon">⚠</div>
              <div className="warning-content">
                <strong>Perhatian untuk Platform Shopee</strong>
                <p>
                  Untuk menggunakan Shopee, Anda harus login terlebih dahulu.
                  Klik tombol "Open shopee" di pojok kanan atas, browser chrome
                  akan terbuka dan akan diarahkan ke halaman login shopee.
                </p>
              </div>
            </div>
          )}
        </div>

        <div className="content-section">
          <h3 className="section-title">Hasil Pencarian</h3>

          {/* Action buttons - only show when there are results */}
          {!loading && results.length > 0 && (
            <div style={{ display: "flex", gap: "8px", marginBottom: "16px" }}>
              <button
                onClick={handlePrint}
                className="btn-secondary"
                style={{ fontSize: "14px", padding: "8px 16px" }}
              >
                🖨️ Print
              </button>
              <button
                onClick={handleExportExcel}
                className="btn-secondary"
                style={{ fontSize: "14px", padding: "8px 16px" }}
              >
                📊 Export to Excel
              </button>
            </div>
          )}

          <div>
            {loading && (
              <p className="text-sm text-gray-500">
                Mencari... tunggu sebentar
              </p>
            )}
            {!loading && results.length === 0 && (
              <p className="text-sm text-gray-500">Belum ada hasil</p>
            )}
            {results.map((shop, sIdx) => {
              // determine if this shop has products for every query
              const allFound =
                shop.results &&
                shop.results.length > 0 &&
                shop.results.every((r) => r.products && r.products.length > 0);
              const isShopExpanded = expandedShops[sIdx] || false;

              return (
                <div key={sIdx} className="expandable-container">
                  <div
                    className="expandable-header"
                    onClick={() => toggleShop(sIdx)}
                  >
                    <h4 className="shop-name">
                      {shop.shop_name} -{" "}
                      {shop.platform === "tokopedia" ? "Tokopedia" : "Shopee"} (
                      {shop.results
                        ? shop.results.filter(
                            (r) => r.products && r.products.length > 0,
                          ).length
                        : 0}
                      /{shop.results ? shop.results.length : 0})
                    </h4>
                    <div className="flex items-center gap-3">
                      {allFound && (
                        <div className="badge bg-green-100 text-green-800 text-sm px-2 py-1 rounded">
                          Semua produk ditemukan
                        </div>
                      )}
                      <svg
                        className={`expandable-icon ${isShopExpanded ? "rotated" : ""}`}
                        width="16"
                        height="16"
                        fill="currentColor"
                        viewBox="0 0 16 16"
                        xmlns="http://www.w3.org/2000/svg"
                      >
                        <path
                          d="M6 8L2 8L2 6L8 5.24536e-07L14 6L14 8L10 8L10 16L6 16L6 8Z"
                          fill="#000000"
                        />
                      </svg>
                    </div>
                  </div>

                  <div
                    className={`expandable-content ${isShopExpanded ? "expanded" : "collapsed"}`}
                  >
                    <div className="shop-details">
                      <div className="mb-2">
                        <a
                          href={shop.shop_url}
                          target="_blank"
                          rel="noreferrer"
                          className="text-blue-600 text-sm hover:underline"
                          onClick={(e) => handleCopyLink(e, shop.shop_url)}
                        >
                          {shop.shop_url}
                        </a>
                      </div>

                      {shop.results &&
                        shop.results.map((qr, qIdx) => {
                          const key = `${sIdx}-${qIdx}`;
                          const isQueryExpanded = expandedQueries[key] || false;
                          const hasProducts =
                            qr.products && qr.products.length > 0;

                          return (
                            <div key={qIdx} className="query-item">
                              <div
                                className="query-summary"
                                onClick={() => toggleQuery(sIdx, qIdx)}
                              >
                                <div className="flex items-center gap-2">
                                  <span className="query-pill text-sm">
                                    {qr.query}
                                  </span>
                                  <span className="query-count text-sm text-gray-500">
                                    ({qr.products ? qr.products.length : 0}{" "}
                                    products)
                                  </span>
                                </div>
                                <svg
                                  className={`expandable-icon ${isQueryExpanded ? "rotated" : ""}`}
                                  width="12"
                                  height="12"
                                  fill="currentColor"
                                  viewBox="0 0 16 16"
                                >
                                  <path
                                    d="M6 8L2 8L2 6L8 5.24536e-07L14 6L14 8L10 8L10 16L6 16L6 8Z"
                                    fill="#000000"
                                  />
                                </svg>
                              </div>

                              {isQueryExpanded && (
                                <div className="query-products">
                                  {!hasProducts ? (
                                    <p className="no-results">
                                      Tidak ada hasil
                                    </p>
                                  ) : (
                                    <div className="product-list">
                                      {qr.products.map((p, pIdx) => {
                                        const img =
                                          p.photo ||
                                          p.image ||
                                          p.thumbnail ||
                                          (p.photos && p.photos[0]) ||
                                          null;
                                        return (
                                          <div
                                            key={pIdx}
                                            className="product-item"
                                          >
                                            {img ? (
                                              <img
                                                src={img}
                                                alt={p.name || "product"}
                                                className="product-image-thumb"
                                              />
                                            ) : (
                                              <div className="product-image-thumb bg-gray-200 flex items-center justify-center text-xs text-gray-500">
                                                No Image
                                              </div>
                                            )}
                                            <div className="product-info">
                                              <div className="product-title">
                                                {p.name || p.link}
                                              </div>
                                              {p.price && (
                                                <div className="product-price text-green-600 text-sm">
                                                  {p.price}
                                                </div>
                                              )}
                                              <a
                                                href={p.link}
                                                target="_blank"
                                                rel="noreferrer"
                                                className="product-link-text"
                                                onClick={(e) =>
                                                  handleCopyLink(e, p.link)
                                                }
                                              >
                                                View Product
                                              </a>
                                            </div>
                                          </div>
                                        );
                                      })}
                                    </div>
                                  )}
                                </div>
                              )}
                            </div>
                          );
                        })}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      {/* Copy Notification Popup */}
      {showCopyNotification && (
        <div className="notification">✓ Link berhasil disalin!</div>
      )}
    </div>
  );
}

export default App;
