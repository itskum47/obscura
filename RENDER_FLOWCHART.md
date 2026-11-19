# Obscura — Complete Rendering & Execution Flowcharts

---

## 1. Pure-Rust Visual Rendering & Rasterization Pipeline (`obscura-render`)

This flowchart details how Obscura transforms an HTML document, CSS stylesheets, and DOM mutations into a 2D software-rasterized framebuffer, screenshot, or PDF without Chromium or GPU dependencies.

```mermaid
flowchart TD
    %% ==========================================
    %% INPUT & DOM CONSTRUCTION
    %% ==========================================
    subgraph S1["Stage 1: Document & Style Ingestion"]
        HTML["Raw HTML Markup"] --> DOM_Parser["WHATWG Parser: html5ever\n(crates/obscura-dom)"]
        DOM_Parser --> DOM_Tree["Arena-Allocated DomTree\n- Shadow Roots (Open/Closed)\n- Slot Distribution"]
        
        CSS_External["External Stylesheets (<link rel='stylesheet'>)"] --> CSS_Parser["CSS Parser: cssparser\n(crates/obscura-render::css)"]
        CSS_Inline["Inline <style> Tags & Style Attributes"] --> CSS_Parser
        CSS_UA["User-Agent Default Stylesheet (HTML5 Specs)"] --> CSS_Parser
        CSS_Parser --> Style_Rules["Compiled CSS Rule List\n- Media Queries (screen, print, dark-mode)\n- Specificity Tuples (A, B, C, D)"]
    end

    %% ==========================================
    %% CASCADE & COMPUTED STYLE
    %% ==========================================
    subgraph S2["Stage 2: Style Cascade & Resolution (crates/obscura-render::style)"]
        DOM_Tree --> Cascade_Engine{"Style Cascade & Matching"}
        Style_Rules --> Cascade_Engine
        
        Cascade_Engine --> Match_Selectors["Selector Matching via selectors crate\n- Class, ID, Pseudo-classes (:hover, :focus, :checked)\n- Relational & Has Selectors"]
        Match_Selectors --> Compute_Style["Computed Style Resolution\n- Property Inheritance (font-family, color, etc.)\n- Unit Normalization (em, rem, vh, vw -> CSS px)\n- Display Type (block, inline, flex, grid, none)"]
        Compute_Style --> Render_Tree["Render Tree Nodes\n(Elements with display != none)"]
    end

    %% ==========================================
    %% GEOMETRY & LAYOUT
    %% ==========================================
    subgraph S3["Stage 3: Box Geometry & Layout Engine (crates/obscura-render::dom)"]
        Render_Tree --> Layout_Builder["Layout Tree Construction"]
        Viewport["Viewport Dimensions\n(width, height, device_scale_factor)"] --> Layout_Builder
        
        Layout_Builder --> Taffy_Engine["Taffy v0.12 Pure-Rust Layout Solver\n- Flexbox Direction, Alignment & Wrapping\n- CSS Grid Tracks, Spans & Areas\n- Intrinsic Sizing (min-content, max-content)"]
        
        subgraph Text_Shaping["Inline & Text Shaping (crates/obscura-render::inline)"]
            FontDB["System & Web Fonts (fontdb)"] --> Font_Shaper["Glyph Shaper & Rasterizer\n(cosmic-text & ab_glyph)"]
            Font_Shaper --> Text_Break["Unicode Line Breaking & Word Wrapping\n(unicode-linebreak & unicode-segmentation)"]
            Text_Break --> Text_Run["Inline Text Runs & Baseline Alignment"]
        end
        
        Taffy_Engine <--> Text_Shaping
        Taffy_Engine --> Box_Geometry["Resolved Box Geometry\n- Content, Padding, Border, Margin Boxes\n- Absolute & Relative Positions (x, y, w, h)\n- Overflow & Scroll Clip Rectangles"]
    end

    %% ==========================================
    %% PAINT & RASTERIZATION
    %% ==========================================
    subgraph S4["Stage 4: Software 2D Paint & Rasterization (crates/obscura-render::paint)"]
        Box_Geometry --> Stacking_Context["Stacking Context Tree\n- z-index Sorting & Stacking Levels\n- Opacity, Clip Paths & Overflow Masks"]
        
        Stacking_Context --> Paint_Passes["Sequential Paint Passes"]
        
        Paint_Passes --> P1["1. Background Colors, Gradients & Images"]
        Paint_Passes --> P2["2. Borders, Radii & Box Shadows (crates/obscura-render::border)"]
        Paint_Passes --> P3["3. Text Glyphs, Underlines & Strikethroughs"]
        Paint_Passes --> P4["4. Form Controls, Checkboxes, Input Carets"]
        Paint_Passes --> P5["5. Outlines, Focus Rings & Overlays"]
        
        P1 --> Tiny_Skia["tiny-skia CPU 2D Rasterizer\n- Pure-Rust Software Draw Pipeline\n- Subpixel Anti-Aliasing & Blending\n- Zero GPU / Zero Window Server Dependency"]
        P2 --> Tiny_Skia
        P3 --> Tiny_Skia
        P4 --> Tiny_Skia
        P5 --> Tiny_Skia
    end

    %% ==========================================
    %% ARTIFACT GENERATION
    %% ==========================================
    subgraph S5["Stage 5: Output Generation & Protocol Export"]
        Tiny_Skia --> Framebuffer["Raw Pixel Buffer (RGBA8888 Pixmap)"]
        
        Framebuffer --> PNG_Export["PNG Screenshot Export\n(image / resvg / mcp browser_screenshot)"]
        Framebuffer --> PDF_Export["Paginated Vector/Raster PDF\n(mcp browser_pdf)"]
        Box_Geometry --> Bounding_Rects["DOM Bounding Rects & Snapshots\n(CDP DOMSnapshot & Element BoundingClientRect)"]
    end
```

---

## 2. Dual-Engine Intelligent Execution Router Flowchart (`obscura-agent::router`)

This flowchart details how Obscura dynamically routes incoming web requests between the **sub-20ms Pure-Rust Fast Path** and the **Supervised V8 Browser Escalation Path**.

```mermaid
flowchart TD
    %% ==========================================
    %% REQUEST INTAKE
    %% ==========================================
    Req["Incoming Web Request / Target URL"] --> Net_Probe["Fast HTTP Network Probe\n(crates/obscura-net: reqwest + rustls)"]
    
    Net_Probe --> SSRF_Check{"SSRF Guard Check\n(crates/obscura-ssrf)"}
    SSRF_Check -->|Private / Loopback / Metadata IP| Deny["403 Forbidden: Blocked by SSRF Policy"]
    
    SSRF_Check -->|Permitted Public Host| Fetch_Response["Receive Response Status, Headers & First Chunk"]
    
    %% ==========================================
    %% CONTENT-TYPE INSPECTION
    %% ==========================================
    Fetch_Response --> Type_Check{"Inspect Content-Type & Payload"}
    
    Type_Check -->|application/json| Direct_JSON["Direct API Processing\n- Zero DOM, Zero V8\n- Latency: < 10ms\n- Memory: ~0 MB"]
    Type_Check -->|text/xml, application/rss+xml, sitemap.xml| Feed_Parser["Feed & Sitemap Parser\n(crates/obscura-extract::feeds)\n- roxmltree Parsing\n- Latency: < 15ms"]
    Type_Check -->|text/html| HTML_Analysis["DOM & Marker Analysis\n(crates/obscura-agent::router)"]
    
    %% ==========================================
    %% SPA & DYNAMIC CONTENT PROBING
    %% ==========================================
    HTML_Analysis --> SPA_Probe{"Evaluate Dynamic SPA Indicators"}
    
    SPA_Probe -->|SPA Shell Detected: id='root', id='app', app-root| Text_Density{"Check Static Text Volume"}
    SPA_Probe -->|noscript Block: 'Enable JavaScript'| Escalate_JS["Escalation Required:\nJavaScript Execution Mandatory"]
    SPA_Probe -->|Standard Informational HTML| Substantive_Check{"Substantive Text > 300 Chars?"}
    
    Text_Density -->|Text < 250 Chars AND Scripts >= 2| Escalate_JS
    Text_Density -->|Rich Pre-rendered Text Found (SSR / Next.js / Astro)| Fast_Path["Fast Path Approved:\nStatic Content Fully Present"]
    
    Substantive_Check -->|Yes: News, Article, Docs, Wiki, Blog| Fast_Path
    Substantive_Check -->|No: Empty Skeleton or Obfuscated Shell| Escalate_JS
    
    %% ==========================================
    %% EXECUTION PATHS
    %% ==========================================
    subgraph FAST_PIPELINE["Path A: Pure-Rust Lightweight Intelligence (< 20ms)"]
        Fast_Path --> Pure_DOM["Pure-Rust DOM Parse: html5ever\n(crates/obscura-dom)"]
        Pure_DOM --> Extract_Engine["Unified Extractor (crates/obscura-extract)\n- Arc90 Readability Algorithm\n- OpenGraph & Twitter Cards\n- JSON-LD Schema.org Structuring\n- HTML Tables to Markdown & JSON"]
        Extract_Engine --> Fast_Output["Clean Evidence-Backed Markdown & Data"]
    end
    
    subgraph BROWSER_PIPELINE["Path B: Supervised Headless Browser (V8 Escalation)"]
        Escalate_JS --> Spawn_Worker["Spawn Sandboxed Worker Process\n(crates/obscura-supervisor + obscura-isolation)"]
        Spawn_Worker --> Page_Navigate["Load Page in V8 Runtime (crates/obscura-js)\n- Execute Client-Side React/Vue Hydration\n- Wait for Network Quiescence (networkidle0)\n- Trigger Timers & DOM Mutation Observers"]
        Page_Navigate --> Browser_Snapshot["Semantic DOM Distillation / Snapshot\n(crates/obscura-agent: SemanticDistiller)"]
        Browser_Snapshot --> Browser_Output["Rendered Dynamic DOM & Interactive State"]
    end
    
    %% Output Convergence
    Fast_Output --> Final_Delivery["Relevance Scoring & Multi-Source Synthesis"]
    Browser_Output --> Final_Delivery
    Direct_JSON --> Final_Delivery
    Feed_Parser --> Final_Delivery
```
