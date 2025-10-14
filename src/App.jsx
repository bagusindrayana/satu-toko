import React, { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./index.css";

function App() {
  const [tags, setTags] = useState([]);
  const [input, setInput] = useState("");
  const [results, setResults] = useState([]);
  const [loading, setLoading] = useState(false);
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
      const res = await invoke("scrape_products", { queries: tags });
      setResults(res);
      setLoading(false);
    } catch (e) {
      console.error(e);
      alert("Error during scraping: " + String(e));
      setLoading(false);
    }
  }

  async function onOpenDriver() {
    try {
      await invoke("open_chrome_with_driver");
    } catch (e) {
      console.error(e);
      alert("Failed to open driver folder: " + String(e));
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center p-4 w-full">
      <div className="card">
        <div className="flex justify-between items-start">
          <h2 className="text-xl font-semibold">Satu Toko — Scraper</h2>
          <div>
            <button onClick={onOpenDriver} className="px-3 py-1 bg-gray-200 rounded">Open Driver</button>
          </div>
        </div>

        <div className="mt-4">
          <label className="block text-sm text-gray-600">Nama produk (bisa lebih dari satu)</label>
          <div className="mt-2">
            <div className="flex flex-wrap items-center">
              {tags.map((t, i) => (
                <span key={i} className="tag">
                  {t}
                  <button onClick={() => removeTag(i)} className="ml-2">×</button>
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
                  } else if (e.key === "Backspace" && input === "" && tags.length > 0) {
                    removeTag(tags.length - 1);
                  }
                }}
                placeholder="Ketik lalu tekan Enter atau koma"
                className="flex-1 p-2 outline-none"
              />
            </div>
          </div>

          <div className="mt-4">
            <button onClick={onSearch} className="px-4 py-2 bg-blue-600 text-white rounded">Cari</button>
          </div>
        </div>

        <div className="mt-6">
          <h3 className="text-lg font-medium">Hasil</h3>
          <div className="mt-3">
            {loading && <p className="text-sm text-gray-500">Mencari... tunggu sebentar</p>}
            {!loading && results.length === 0 && <p className="text-sm text-gray-500">Belum ada hasil</p>}
            {results.map((shop, sIdx) => {
              // determine if this shop has products for every query
              const allFound = shop.results && shop.results.length > 0 && shop.results.every(r => (r.products && r.products.length > 0));
              return (
                <div key={sIdx} className={"shop-block " + (allFound ? "shop-highlight" : "")}>
                  <div className="shop-header flex items-center justify-between">
                    <div>
                      <a href={shop.shop_url} target="_blank" rel="noreferrer" className="shop-name">{shop.shop_name}</a>
                      <div className="shop-subtitle text-sm text-gray-500">{shop.shop_url}</div>
                    </div>
                    <div className="flex items-center gap-3">
                      <div className="shop-count text-sm text-gray-600">{shop.results ? shop.results.length : 0} query</div>
                      {allFound && <div className="badge bg-green-100 text-green-800 text-sm px-2 py-1 rounded">Semua produk ditemukan</div>}
                    </div>
                  </div>
                  <div className="shop-queries mt-2">
                    {shop.results.map((qr, qIdx) => (
                      <div key={qIdx} className="query-block">
                        <div className="query-title flex items-center gap-3">
                          <div className="query-label text-sm text-gray-600">Hasil untuk:</div>
                          <div className="query-pill text-sm font-medium bg-gray-100 px-2 py-1 rounded">{qr.query}</div>
                          <div className="query-count text-sm text-gray-500">({qr.products ? qr.products.length : 0})</div>
                        </div>
                        <div className="mt-2">
                          {(!qr.products || qr.products.length === 0) && <div className="text-sm text-gray-500">Tidak ada hasil</div>}
                          {qr.products && qr.products.length > 0 && (
                            <div className="products-grid">
                              {qr.products.map((p, pIdx) => {
                                const img = p.photo || p.image || p.thumbnail || (p.photos && p.photos[0]) || null;
                                return (
                                  <div key={pIdx} className="product-card">
                                    <a href={p.link} target="_blank" rel="noreferrer" className="product-link">
                                      <div className="product-image-wrap">
                                        {img ? (
                                          <img src={img} alt={p.name || 'product'} className="product-image" />
                                        ) : (
                                          <div className="product-image product-image--placeholder">No image</div>
                                        )}
                                      </div>
                                      <div className="product-body">
                                        <div className="product-name">{p.name || p.link}</div>
                                        {p.price && <div className="product-price">{p.price}</div>}
                                      </div>
                                    </a>
                                  </div>
                                );
                              })}
                            </div>
                          )}
                        </div>
                      </div>
                    ))}
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
