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
  const inputRef = useRef(null);
  const listenersRef = useRef([]);

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
      await invoke("open_browser_with_driver", {
        url: "https://shopee.co.id/login",
      });
      alert("Browser opened with ChromeDriver!");
    } catch (e) {
      console.error(e);
      alert("Failed to open browser: " + String(e));
    }
  }

  async function onOpenTokopedia() {
    try {
      await invoke("open_browser_with_driver", {
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

  return (
    <div className="min-h-screen flex items-center justify-center p-4 w-full">
      <div className="card">
        <div className="flex justify-between items-start">
          <h2 className="text-xl font-semibold">Satu Toko — Scraper</h2>
          <div>
            <button
              onClick={onOpenDriver}
              className="px-3 py-1 bg-gray-200 rounded"
            >
              Open Driver
            </button>
          </div>
        </div>

        {/* Driver Info Modal */}
        {showDriverModal && (
          <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
            <div className="bg-white rounded-lg p-6 w-96">
              <div className="flex justify-between items-center mb-4">
                <h3 className="text-lg font-semibold">
                  Chrome Driver Information
                </h3>
                <button
                  onClick={closeModal}
                  className="text-gray-500 hover:text-gray-700"
                >
                  <svg
                    className="w-6 h-6"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth="2"
                      d="M6 18L18 6M6 6l12 12"
                    ></path>
                  </svg>
                </button>
              </div>

              {infoLoading ? (
                <p>Loading...</p>
              ) : (
                <div className="space-y-4">
                  <div>
                    <label className="block text-sm font-medium text-gray-700">
                      Chrome Version
                    </label>
                    <div className="mt-1 p-2 bg-gray-100 rounded">
                      {chromeInfo.chromeVersion || "Not detected"}
                    </div>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700">
                      ChromeDriver Version
                    </label>
                    <div className="mt-1 p-2 bg-gray-100 rounded">
                      {chromeInfo.driverVersion || "Not downloaded"}
                    </div>
                  </div>
                </div>
              )}

              <div className="mt-6 flex justify-between">
                <button
                  onClick={onOpenShopee}
                  disabled={infoLoading}
                  className="px-4 py-2 bg-orange-600 text-white rounded disabled:opacity-50"
                >
                  Open Shopee
                </button>
                <button
                  onClick={onOpenTokopedia}
                  disabled={infoLoading}
                  className="px-4 py-2 bg-blue-600 text-white rounded disabled:opacity-50"
                >
                  Open Tokopedia
                </button>
              </div>
            </div>
          </div>
        )}

        <div className="mt-4">
          <label className="block text-sm text-gray-600">
            Nama produk (bisa lebih dari satu)
          </label>
          <div className="mt-2">
            <div className="flex flex-wrap items-center">
              {tags.map((t, i) => (
                <span key={i} className="tag">
                  {t}
                  <button onClick={() => removeTag(i)} className="ml-2">
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
                placeholder="Ketik lalu tekan Enter atau koma"
                className="flex-1 p-2 outline-none"
              />
            </div>
          </div>

          <div className="mt-4 flex flex-col sm:flex-row gap-4">
            <div>
              <label className="block text-sm text-gray-600">Platform</label>
              <select
                value={selectedPlatform}
                onChange={(e) => setSelectedPlatform(e.target.value)}
                className="mt-1 p-2 border rounded"
              >
                <option value="tokopedia">Tokopedia</option>
                <option value="shopee">Shopee</option>
              </select>
            </div>
            <div>
              <button
                onClick={onSearch}
                className="mt-5 sm:mt-6 px-4 py-2 bg-blue-600 text-white rounded"
              >
                Cari
              </button>
            </div>
          </div>
        </div>

        <div className="mt-6">
          <h3 className="text-lg font-medium">Hasil</h3>
          <div className="mt-3">
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
                      {shop.results ? shop.results.length : 0} queries)
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
    </div>
  );
}

export default App;
