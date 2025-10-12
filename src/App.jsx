import React, { useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./index.css";

function App() {
  const [tags, setTags] = useState([]);
  const [input, setInput] = useState("");
  const [results, setResults] = useState([]);
  const inputRef = useRef(null);

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
    try {
      const res = await invoke("scrape_products", { queries: tags });
      setResults(res);
    } catch (e) {
      console.error(e);
      alert("Error during scraping: " + String(e));
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
    <div className="container">
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
            {results.length === 0 && <p className="text-sm text-gray-500">Belum ada hasil</p>}
            {results.map((r, idx) => (
              <div key={idx} className="result-item">
                <img src={r.photo || '/vite.svg'} alt="foto" className="result-photo"/>
                <div className="flex-1">
                  <div className="font-semibold">{r.name}</div>
                  <div className="text-sm text-green-600">{r.price}</div>
                  <div className="text-sm text-gray-600">{r.shop} — {r.location}</div>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
