import { useState, useEffect, useRef, useCallback } from "react";

const MOCK_FILES = [
  { name: "main.rs", path: "C:\\Projects\\turbofind\\src\\main.rs", size: 12480, ext: "rs", isDir: false, modified: "2026-03-01" },
  { name: "Cargo.toml", path: "C:\\Projects\\turbofind\\Cargo.toml", size: 482, ext: "toml", isDir: false, modified: "2026-03-01" },
  { name: "budget_2026.xlsx", path: "C:\\Users\\FuLong\\Documents\\budget_2026.xlsx", size: 245760, ext: "xlsx", isDir: false, modified: "2026-02-28" },
  { name: "portfolio", path: "C:\\Projects\\portfolio", size: 0, ext: "", isDir: true, modified: "2026-03-02" },
  { name: "shader_water.glsl", path: "C:\\Projects\\threejs-terrain\\shaders\\shader_water.glsl", size: 8923, ext: "glsl", isDir: false, modified: "2026-02-20" },
  { name: "campfire_scene.js", path: "C:\\Projects\\threejs-campfire\\src\\campfire_scene.js", size: 34200, ext: "js", isDir: false, modified: "2026-02-15" },
  { name: "README.md", path: "C:\\Projects\\turbofind\\README.md", size: 3200, ext: "md", isDir: false, modified: "2026-03-01" },
  { name: "synthwave_visualizer.html", path: "C:\\Projects\\visualizer\\synthwave_visualizer.html", size: 18400, ext: "html", isDir: false, modified: "2026-01-30" },
  { name: "terrain_generator.js", path: "C:\\Projects\\threejs-terrain\\src\\terrain_generator.js", size: 22100, ext: "js", isDir: false, modified: "2026-02-18" },
  { name: "pokemon_tracker.py", path: "C:\\Projects\\pokemon-cards\\pokemon_tracker.py", size: 15600, ext: "py", isDir: false, modified: "2026-02-10" },
  { name: "invoice_january.pdf", path: "C:\\Users\\FuLong\\Documents\\Invoices\\invoice_january.pdf", size: 142000, ext: "pdf", isDir: false, modified: "2026-01-15" },
  { name: "node_modules", path: "C:\\Projects\\portfolio\\node_modules", size: 0, ext: "", isDir: true, modified: "2026-03-02" },
  { name: "gerstner_waves.glsl", path: "C:\\Projects\\threejs-terrain\\shaders\\gerstner_waves.glsl", size: 5600, ext: "glsl", isDir: false, modified: "2026-02-19" },
  { name: "package.json", path: "C:\\Projects\\portfolio\\package.json", size: 890, ext: "json", isDir: false, modified: "2026-03-02" },
  { name: "config.yaml", path: "C:\\Users\\FuLong\\.config\\config.yaml", size: 1200, ext: "yaml", isDir: false, modified: "2026-02-01" },
  { name: "resume_fulong.docx", path: "C:\\Users\\FuLong\\Documents\\resume_fulong.docx", size: 48000, ext: "docx", isDir: false, modified: "2026-02-25" },
  { name: "screenshot_2026.png", path: "C:\\Users\\FuLong\\Pictures\\screenshot_2026.png", size: 2400000, ext: "png", isDir: false, modified: "2026-03-01" },
  { name: "backup.zip", path: "C:\\Users\\FuLong\\Downloads\\backup.zip", size: 89000000, ext: "zip", isDir: false, modified: "2026-02-28" },
  { name: "audio_stream.py", path: "C:\\Projects\\radio-player\\audio_stream.py", size: 9800, ext: "py", isDir: false, modified: "2025-08-10" },
  { name: "App.jsx", path: "C:\\Projects\\portfolio\\src\\App.jsx", size: 6700, ext: "jsx", isDir: false, modified: "2026-03-02" },
  { name: "matrix_effect.js", path: "C:\\Projects\\portfolio\\src\\effects\\matrix_effect.js", size: 4300, ext: "js", isDir: false, modified: "2026-03-01" },
  { name: "water_reflection.glsl", path: "C:\\Projects\\threejs-terrain\\shaders\\water_reflection.glsl", size: 3800, ext: "glsl", isDir: false, modified: "2026-02-22" },
  { name: "fps_controller.js", path: "C:\\Projects\\threejs-terrain\\src\\fps_controller.js", size: 11200, ext: "js", isDir: false, modified: "2026-02-21" },
  { name: "Bewerbung_Entwurf.docx", path: "C:\\Users\\FuLong\\Documents\\Bewerbung_Entwurf.docx", size: 35000, ext: "docx", isDir: false, modified: "2026-02-26" },
];

const ICONS = {
  rs: "🦀", py: "🐍", js: "📜", jsx: "⚛️", ts: "📘", tsx: "⚛️",
  html: "🌐", css: "🎨", glsl: "✨", json: "📋", yaml: "📋", toml: "📋",
  md: "📝", txt: "📄", pdf: "📕", doc: "📘", docx: "📘",
  xls: "📗", xlsx: "📗", png: "🖼️", jpg: "🖼️", gif: "🖼️", svg: "🖼️",
  mp3: "🎵", wav: "🎵", mp4: "🎬", mkv: "🎬",
  zip: "📦", rar: "📦", "7z": "📦", exe: "⚙️",
  dir: "📁",
};

function fuzzyMatch(text, query) {
  const t = text.toLowerCase();
  const q = query.toLowerCase();
  if (t.includes(q)) return { match: true, score: 100 + (q.length / t.length) * 50 };
  let qi = 0, score = 0, consecutive = 0;
  for (let ti = 0; ti < t.length && qi < q.length; ti++) {
    if (t[ti] === q[qi]) {
      qi++;
      consecutive++;
      score += consecutive * 2;
    } else {
      consecutive = 0;
    }
  }
  return qi === q.length ? { match: true, score } : { match: false, score: 0 };
}

function formatSize(bytes) {
  if (bytes === 0) return "—";
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + " GB";
  if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + " MB";
  if (bytes >= 1024) return Math.round(bytes / 1024) + " KB";
  return bytes + " B";
}

export default function TurboFind() {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [results, setResults] = useState([]);
  const [searchTime, setSearchTime] = useState(0);
  const [indexCount] = useState(MOCK_FILES.length);
  const [showSplash, setShowSplash] = useState(true);
  const inputRef = useRef(null);
  const listRef = useRef(null);

  useEffect(() => {
    const t = setTimeout(() => setShowSplash(false), 1800);
    return () => clearTimeout(t);
  }, []);

  useEffect(() => {
    if (!showSplash && inputRef.current) inputRef.current.focus();
  }, [showSplash]);

  const search = useCallback((q) => {
    const start = performance.now();
    if (!q.trim()) {
      setResults([]);
      setSearchTime(0);
      return;
    }

    let extFilter = null;
    let dirOnly = false;
    const terms = [];
    for (const part of q.split(/\s+/)) {
      if (part.startsWith("ext:")) extFilter = part.slice(4).toLowerCase().replace(".", "");
      else if (part === "dir:" || part === "folder:") dirOnly = true;
      else terms.push(part);
    }
    const searchQ = terms.join(" ");

    let matched = MOCK_FILES
      .map((f) => {
        if (extFilter && f.ext !== extFilter) return null;
        if (dirOnly && !f.isDir) return null;
        if (!searchQ) return { file: f, score: 0 };
        const r = fuzzyMatch(f.name, searchQ);
        return r.match ? { file: f, score: r.score } : null;
      })
      .filter(Boolean)
      .sort((a, b) => b.score - a.score);

    setResults(matched);
    setSearchTime(performance.now() - start);
    setSelected(0);
  }, []);

  const handleKeyDown = (e) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((s) => Math.min(s + 1, results.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((s) => Math.max(s - 1, 0));
    } else if (e.key === "Enter" && results[selected]) {
      // Simulate opening
    }
  };

  useEffect(() => {
    const el = listRef.current?.children[selected];
    if (el) el.scrollIntoView({ block: "nearest" });
  }, [selected]);

  if (showSplash) {
    return (
      <div style={{
        background: "#0a0a0f",
        height: "100vh",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        flexDirection: "column",
        fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      }}>
        <style>{`
          @import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@300;400;600;700&family=Outfit:wght@300;400;600;800&display=swap');
          @keyframes bolt { 0%,100%{opacity:0.3;transform:scale(0.9)} 50%{opacity:1;transform:scale(1.1)} }
          @keyframes fadeUp { from{opacity:0;transform:translateY(20px)} to{opacity:1;transform:translateY(0)} }
          @keyframes scanline { 0%{top:-10%} 100%{top:110%} }
          @keyframes barGrow { from{width:0} to{width:100%} }
        `}</style>
        <div style={{ fontSize: 64, animation: "bolt 0.8s ease-in-out infinite" }}>⚡</div>
        <div style={{
          color: "#00ffd5",
          fontSize: 28,
          fontWeight: 700,
          letterSpacing: 6,
          marginTop: 16,
          animation: "fadeUp 0.6s ease-out",
          fontFamily: "'Outfit', sans-serif",
        }}>TURBOFIND</div>
        <div style={{
          color: "#444",
          fontSize: 12,
          marginTop: 8,
          letterSpacing: 3,
        }}>INDEXING FILESYSTEM...</div>
        <div style={{
          width: 200,
          height: 2,
          background: "#1a1a2e",
          borderRadius: 2,
          marginTop: 20,
          overflow: "hidden",
        }}>
          <div style={{
            height: "100%",
            background: "linear-gradient(90deg, #00ffd5, #00a8ff)",
            animation: "barGrow 1.5s ease-out forwards",
            borderRadius: 2,
          }}/>
        </div>
      </div>
    );
  }

  return (
    <div
      style={{
        background: "#0a0a0f",
        minHeight: "100vh",
        fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
        color: "#e0e0e0",
        position: "relative",
        overflow: "hidden",
      }}
      onClick={() => inputRef.current?.focus()}
    >
      <style>{`
        @import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@300;400;600;700&family=Outfit:wght@300;400;600;800&display=swap');
        * { box-sizing: border-box; margin: 0; padding: 0; }
        ::-webkit-scrollbar { width: 6px; }
        ::-webkit-scrollbar-track { background: #0a0a0f; }
        ::-webkit-scrollbar-thumb { background: #1e1e3a; border-radius: 3px; }
        ::-webkit-scrollbar-thumb:hover { background: #00ffd5; }
        @keyframes fadeIn { from{opacity:0;transform:translateY(-10px)} to{opacity:1;transform:translateY(0)} }
        @keyframes pulse { 0%,100%{box-shadow:0 0 20px rgba(0,255,213,0.1)} 50%{box-shadow:0 0 40px rgba(0,255,213,0.2)} }
        @keyframes gridMove { 0%{background-position:0 0} 100%{background-position:40px 40px} }
      `}</style>

      {/* Animated grid background */}
      <div style={{
        position: "fixed",
        inset: 0,
        backgroundImage: "linear-gradient(rgba(0,255,213,0.03) 1px, transparent 1px), linear-gradient(90deg, rgba(0,255,213,0.03) 1px, transparent 1px)",
        backgroundSize: "40px 40px",
        animation: "gridMove 8s linear infinite",
        pointerEvents: "none",
        zIndex: 0,
      }}/>

      {/* Radial glow */}
      <div style={{
        position: "fixed",
        top: 0,
        left: "50%",
        transform: "translateX(-50%)",
        width: 800,
        height: 400,
        background: "radial-gradient(ellipse at center, rgba(0,255,213,0.06) 0%, transparent 70%)",
        pointerEvents: "none",
        zIndex: 0,
      }}/>

      <div style={{ position: "relative", zIndex: 1, maxWidth: 900, margin: "0 auto", padding: "32px 24px" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 32 }}>
          <span style={{ fontSize: 28 }}>⚡</span>
          <span style={{
            fontFamily: "'Outfit', sans-serif",
            fontSize: 22,
            fontWeight: 800,
            letterSpacing: 4,
            background: "linear-gradient(135deg, #00ffd5, #00a8ff)",
            WebkitBackgroundClip: "text",
            WebkitTextFillColor: "transparent",
          }}>TURBOFIND</span>
          <div style={{ flex: 1 }}/>
          <span style={{ color: "#3a3a5c", fontSize: 12 }}>
            {indexCount.toLocaleString()} files indexed
          </span>
        </div>

        {/* Search Bar */}
        <div style={{
          background: "#12121f",
          border: "1px solid #1e1e3a",
          borderRadius: 12,
          padding: "16px 20px",
          display: "flex",
          alignItems: "center",
          gap: 14,
          animation: query ? "pulse 2s ease-in-out infinite" : "none",
          transition: "border-color 0.3s, box-shadow 0.3s",
          borderColor: query ? "#00ffd5" : "#1e1e3a",
        }}>
          <span style={{ fontSize: 20, opacity: 0.6 }}>🔍</span>
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => { setQuery(e.target.value); search(e.target.value); }}
            onKeyDown={handleKeyDown}
            placeholder="Search files... (ext:rs for filters)"
            style={{
              flex: 1,
              background: "none",
              border: "none",
              outline: "none",
              color: "#e0e0e0",
              fontSize: 16,
              fontFamily: "'JetBrains Mono', monospace",
              letterSpacing: 0.5,
            }}
          />
          {query && (
            <div style={{ color: "#3a3a5c", fontSize: 11, whiteSpace: "nowrap" }}>
              {results.length} results · {searchTime.toFixed(2)}ms
            </div>
          )}
          {query && (
            <button
              onClick={() => { setQuery(""); setResults([]); }}
              style={{
                background: "#1e1e3a",
                border: "none",
                color: "#666",
                borderRadius: 6,
                width: 24,
                height: 24,
                cursor: "pointer",
                fontSize: 14,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >✕</button>
          )}
        </div>

        {/* Filter chips */}
        <div style={{ display: "flex", gap: 8, marginTop: 12, flexWrap: "wrap" }}>
          {["ext:rs", "ext:js", "ext:glsl", "ext:py", "ext:pdf", "dir:"].map((f) => (
            <button
              key={f}
              onClick={() => {
                const newQ = query.includes(f) ? query.replace(f, "").trim() : `${f} ${query}`.trim();
                setQuery(newQ);
                search(newQ);
              }}
              style={{
                background: query.includes(f) ? "rgba(0,255,213,0.15)" : "#12121f",
                border: `1px solid ${query.includes(f) ? "#00ffd5" : "#1e1e3a"}`,
                color: query.includes(f) ? "#00ffd5" : "#555",
                borderRadius: 20,
                padding: "4px 12px",
                fontSize: 11,
                cursor: "pointer",
                fontFamily: "'JetBrains Mono', monospace",
                transition: "all 0.2s",
              }}
            >{f}</button>
          ))}
        </div>

        {/* Results */}
        <div ref={listRef} style={{ marginTop: 20 }}>
          {!query && (
            <div style={{
              color: "#2a2a4a",
              textAlign: "center",
              padding: "60px 0",
              fontSize: 13,
            }}>
              <div style={{ fontSize: 48, marginBottom: 16, opacity: 0.3 }}>⚡</div>
              <div>Start typing to search {indexCount.toLocaleString()} indexed files</div>
              <div style={{ marginTop: 8, fontSize: 11, color: "#1e1e3a" }}>
                ↑↓ Navigate · Enter Open · ext:rs filter by type
              </div>
            </div>
          )}

          {query && results.length === 0 && (
            <div style={{
              color: "#3a3a5c",
              textAlign: "center",
              padding: "60px 0",
              fontSize: 13,
            }}>
              No files matching "{query}"
            </div>
          )}

          {results.map((r, i) => {
            const f = r.file;
            const icon = f.isDir ? ICONS.dir : (ICONS[f.ext] || "📄");
            const isSelected = i === selected;
            const parent = f.path.replace(/\\[^\\]+$/, "");

            return (
              <div
                key={f.path}
                onMouseEnter={() => setSelected(i)}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 14,
                  padding: "10px 16px",
                  borderRadius: 8,
                  cursor: "pointer",
                  background: isSelected ? "rgba(0,255,213,0.08)" : "transparent",
                  borderLeft: isSelected ? "2px solid #00ffd5" : "2px solid transparent",
                  transition: "all 0.15s",
                  animation: `fadeIn 0.2s ease-out ${i * 0.03}s both`,
                }}
              >
                <span style={{ fontSize: 18, width: 28, textAlign: "center", flexShrink: 0 }}>{icon}</span>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{
                    color: isSelected ? "#00ffd5" : "#d0d0e0",
                    fontSize: 14,
                    fontWeight: isSelected ? 600 : 400,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}>{f.name}</div>
                  <div style={{
                    color: "#2a2a4a",
                    fontSize: 11,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                    marginTop: 2,
                  }}>{parent}</div>
                </div>
                <div style={{
                  color: "#2a2a4a",
                  fontSize: 11,
                  whiteSpace: "nowrap",
                  flexShrink: 0,
                }}>{formatSize(f.size)}</div>
                <div style={{
                  color: "#1e1e3a",
                  fontSize: 11,
                  whiteSpace: "nowrap",
                  flexShrink: 0,
                  width: 80,
                  textAlign: "right",
                }}>{f.modified}</div>
              </div>
            );
          })}
        </div>

        {/* Footer */}
        <div style={{
          position: "fixed",
          bottom: 0,
          left: 0,
          right: 0,
          padding: "10px 24px",
          background: "linear-gradient(transparent, #0a0a0f 40%)",
          display: "flex",
          justifyContent: "center",
          gap: 24,
          fontSize: 11,
          color: "#2a2a4a",
          fontFamily: "'JetBrains Mono', monospace",
        }}>
          <span><kbd style={{
            background: "#12121f",
            border: "1px solid #1e1e3a",
            borderRadius: 3,
            padding: "1px 6px",
            fontSize: 10,
          }}>↑↓</kbd> Navigate</span>
          <span><kbd style={{
            background: "#12121f",
            border: "1px solid #1e1e3a",
            borderRadius: 3,
            padding: "1px 6px",
            fontSize: 10,
          }}>Enter</kbd> Open</span>
          <span><kbd style={{
            background: "#12121f",
            border: "1px solid #1e1e3a",
            borderRadius: 3,
            padding: "1px 6px",
            fontSize: 10,
          }}>Ctrl+O</kbd> Open Folder</span>
          <span style={{ color: "#1a1a2e" }}>│</span>
          <span>Rust 🦀 Powered</span>
        </div>
      </div>
    </div>
  );
}
