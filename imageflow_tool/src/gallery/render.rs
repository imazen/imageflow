//! Shared WYSIWYG static gallery renderer used identically by `gallery build` (static export)
//! and `gallery serve` (live preview canvas).
//!
//! Security & accessibility guarantees:
//! - Every user string (`title`, `subtitle`, `author`, `caption`, `alt_text`, filenames) is
//!   contextually HTML-escaped; embedded JSON escapes `<`, `>`, `&`, U+2028, and U+2029.
//! - Zero `innerHTML` with untrusted content in client-side JS; lightbox builds DOM via
//!   `createElement` + `textContent` + explicit attributes.
//! - Full keyboard navigation (`ArrowLeft`, `ArrowRight`, `Escape`, zoom toggle) with focus
//!   trapping and focus restoration when closing the lightbox.

use super::ResolvedGalleryItem;
use crate::bulk::planner::escape_html;
use crate::bulk::schema::{
    GalleryCaptionStyle, GalleryConfig, GalleryFontFamily, GalleryLayout, GalleryTheme,
};

/// Escape JSON safely for embedding inside `<script type="application/json">...</script>`.
pub fn escape_json_for_script(raw_json: &str) -> String {
    let mut out = String::with_capacity(raw_json.len() + 16);
    for ch in raw_json.chars() {
        match ch {
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            _ => out.push(ch),
        }
    }
    out
}

struct ThemeTokens {
    bg: &'static str,
    surface: &'static str,
    surface_2: &'static str,
    border: &'static str,
    text: &'static str,
    muted: &'static str,
    accent: &'static str,
    shadow: &'static str,
}

fn theme_tokens(theme: GalleryTheme) -> ThemeTokens {
    match theme {
        GalleryTheme::GalleryDark => ThemeTokens {
            bg: "#0b0c10",
            surface: "#14161d",
            surface_2: "#1c202a",
            border: "rgba(255,255,255,0.09)",
            text: "#f3f4f6",
            muted: "#9ca3af",
            accent: "#60a5fa",
            shadow: "0 10px 30px rgba(0,0,0,0.45)",
        },
        GalleryTheme::PaperLight => ThemeTokens {
            bg: "#f7f5f0",
            surface: "#ffffff",
            surface_2: "#efece4",
            border: "rgba(24,24,27,0.10)",
            text: "#18181b",
            muted: "#52525b",
            accent: "#2563eb",
            shadow: "0 8px 24px rgba(15,23,42,0.08)",
        },
        GalleryTheme::MinimalWhite => ThemeTokens {
            bg: "#ffffff",
            surface: "#fafafa",
            surface_2: "#f4f4f5",
            border: "rgba(0,0,0,0.08)",
            text: "#111827",
            muted: "#6b7280",
            accent: "#111827",
            shadow: "0 4px 16px rgba(0,0,0,0.06)",
        },
        GalleryTheme::WarmLinen => ThemeTokens {
            bg: "#f4efe6",
            surface: "#fbf8f2",
            surface_2: "#eae1d2",
            border: "rgba(68,51,34,0.12)",
            text: "#2c221a",
            muted: "#6d5b4b",
            accent: "#b45309",
            shadow: "0 8px 24px rgba(44,34,26,0.09)",
        },
        GalleryTheme::MidnightSlate => ThemeTokens {
            bg: "#0f172a",
            surface: "#1e293b",
            surface_2: "#283548",
            border: "rgba(148,163,184,0.16)",
            text: "#f8fafc",
            muted: "#94a3b8",
            accent: "#38bdf8",
            shadow: "0 12px 32px rgba(2,6,23,0.5)",
        },
    }
}

fn font_stack(font: GalleryFontFamily) -> &'static str {
    match font {
        GalleryFontFamily::Sans => {
            "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif"
        }
        GalleryFontFamily::Serif => {
            "'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, 'Times New Roman', serif"
        }
        GalleryFontFamily::Mono => {
            "'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace"
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// Render a complete self-contained HTML5 gallery document.
///
/// When `preview_mode` is `true`, the document also listens for same-origin `postMessage`
/// live-design updates from the interactive studio sidebar.
pub fn render_gallery_html(
    config: &GalleryConfig,
    items: &[ResolvedGalleryItem],
    preview_mode: bool,
) -> String {
    let d = &config.design;
    let tok = theme_tokens(d.theme);
    let accent = d
        .accent_color_hex
        .as_deref()
        .filter(|s| crate::bulk::schema::validate_hex_color(s).is_ok())
        .unwrap_or(tok.accent);
    let fonts = font_stack(d.font_family);
    let grid_ar = d
        .grid_aspect_ratio
        .map(|a| a.css_string())
        .unwrap_or_else(|| "4 / 3".to_string());

    let layout_class = match d.layout {
        GalleryLayout::Justified => "layout-justified",
        GalleryLayout::Masonry => "layout-masonry",
        GalleryLayout::Grid => "layout-grid",
        GalleryLayout::Editorial => "layout-editorial",
        GalleryLayout::Filmstrip => "layout-filmstrip",
    };
    let caption_class = match d.caption_style {
        GalleryCaptionStyle::Below => "cap-below",
        GalleryCaptionStyle::HoverOverlay => "cap-overlay",
        GalleryCaptionStyle::LightboxOnly => "cap-lightbox",
        GalleryCaptionStyle::Hidden => "cap-hidden",
    };

    let mut cards_html = String::with_capacity(items.len() * 640);
    for (idx, item) in items.iter().enumerate() {
        let featured_cls = if item.featured { " is-featured" } else { "" };
        let flex_basis = (d.target_row_height_px as f64 * item.aspect_ratio).round().max(120.0);
        let flex_grow = (item.aspect_ratio * 100.0).round().max(30.0);
        let badge_html = if d.show_exif_badges {
            let mut parts = vec![format!("{}×{}", item.original_width, item.original_height)];
            if item.smallest_variant_bytes > 0 {
                parts.push(format_bytes(item.smallest_variant_bytes));
            }
            if let Some(score) = item.best_verified_score {
                parts.push(format!("zensim {score:.1}"));
            }
            if item.media_kind == "video" || item.media_kind == "animation" {
                parts.push(item.media_kind.to_ascii_uppercase());
            }
            format!("<span class=\"meta-badge\">{}</span>", escape_html(&parts.join(" · ")))
        } else {
            String::new()
        };

        let caption_inner = if !item.caption.is_empty() {
            format!(
                "<div class=\"cap-title\">{}</div><div class=\"cap-desc\">{}</div>{}",
                escape_html(&item.title),
                escape_html(&item.caption),
                badge_html
            )
        } else {
            format!("<div class=\"cap-title\">{}</div>{}", escape_html(&item.title), badge_html)
        };

        let trigger_attrs = if d.enable_lightbox {
            format!(
                "tabindex=\"0\" role=\"button\" aria-label=\"Open {} in lightbox\" data-index=\"{idx}\"",
                escape_html(&item.title)
            )
        } else {
            format!("data-index=\"{idx}\"")
        };

        cards_html.push_str(&format!(
            "<figure class=\"g-card{featured_cls}\" data-id=\"{}\" style=\"--ar:{:.4};--jf-basis:{:.0}px;--jf-grow:{:.0};\">\n\
               <div class=\"g-frame\" {trigger_attrs}>\n\
                 {}\n\
               </div>\n\
               <figcaption class=\"g-cap\">{caption_inner}</figcaption>\n\
             </figure>\n",
            escape_html(&item.id),
            item.aspect_ratio,
            flex_basis,
            flex_grow,
            item.picture_html,
        ));
    }

    let items_json_raw = serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string());
    let items_json = escape_json_for_script(&items_json_raw);

    let subtitle_html = config
        .subtitle
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("<p class=\"g-sub\">{}</p>", escape_html(s)))
        .unwrap_or_default();
    let author_html = config
        .author
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("<div class=\"g-author\">By {}</div>", escape_html(s)))
        .unwrap_or_default();
    let footer_html = config
        .footer_text
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("<footer class=\"g-footer\">{}</footer>", escape_html(s)))
        .unwrap_or_default();

    let preview_bridge = if preview_mode {
        r#"
window.addEventListener('message', (ev) => {
  if (ev.origin !== window.location.origin) return;
  const msg = ev.data;
  if (!msg || msg.type !== 'imageflow-live-design') return;
  const r = document.documentElement.style;
  const root = document.getElementById('g-root');
  if (msg.cssVars) {
    for (const [k, v] of Object.entries(msg.cssVars)) r.setProperty(k, v);
  }
  if (msg.layoutClass && root) {
    root.className = 'g-grid ' + msg.layoutClass + ' ' + (msg.captionClass || 'cap-below');
  }
  if (typeof msg.title === 'string') {
    const el = document.getElementById('g-title');
    if (el) el.textContent = msg.title;
  }
  if (typeof msg.subtitle === 'string') {
    const el = document.getElementById('g-sub');
    if (el) el.textContent = msg.subtitle;
  }
});
"#
    } else {
        ""
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title_esc}</title>
<style>
:root {{
  --bg: {bg};
  --surface: {surface};
  --surface-2: {surface_2};
  --border: {border};
  --text: {text};
  --muted: {muted};
  --accent: {accent};
  --shadow: {shadow};
  --font: {fonts};
  --gap: {gap}px;
  --radius: {radius}px;
  --max-w: {max_w}px;
  --pad: {pad}px;
  --row-h: {row_h}px;
  --cols-d: {cols_d};
  --cols-t: {cols_t};
  --cols-m: {cols_m};
  --grid-ar: {grid_ar};
}}
* {{ box-sizing: border-box; }}
html, body {{ margin: 0; padding: 0; background: var(--bg); color: var(--text); font-family: var(--font); -webkit-font-smoothing: antialiased; }}
.g-shell {{ max-width: var(--max-w); margin: 0 auto; padding: var(--pad); }}
.g-header {{ margin-bottom: calc(var(--gap) * 1.6); border-bottom: 1px solid var(--border); padding-bottom: 18px; }}
.g-header h1 {{ margin: 0 0 6px; font-size: clamp(1.6rem, 3vw, 2.5rem); letter-spacing: -0.02em; }}
.g-sub {{ margin: 0; color: var(--muted); font-size: 1rem; line-height: 1.5; }}
.g-author {{ margin-top: 8px; font-size: 0.85rem; color: var(--accent); font-weight: 600; }}
.g-card {{ margin: 0; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; box-shadow: var(--shadow); position: relative; transition: transform 180ms ease, border-color 180ms ease; }}
.g-card:hover {{ border-color: var(--accent); transform: translateY(-2px); }}
.g-frame {{ cursor: {cursor}; overflow: hidden; background: var(--surface-2); position: relative; display: block; }}
.g-frame:focus-visible {{ outline: 2px solid var(--accent); outline-offset: -2px; }}
.g-frame picture, .g-frame img {{ display: block; width: 100%; height: 100%; object-fit: cover; }}
.g-cap {{ padding: 10px 12px; display: flex; flex-direction: column; gap: 3px; }}
.cap-title {{ font-weight: 600; font-size: 0.92rem; line-height: 1.3; }}
.cap-desc {{ font-size: 0.82rem; color: var(--muted); line-height: 1.4; }}
.meta-badge {{ margin-top: 4px; font-size: 0.72rem; color: var(--muted); font-variant-numeric: tabular-nums; }}
.cap-hidden .g-cap, .cap-lightbox .g-cap {{ display: none; }}
.cap-overlay .g-cap {{ position: absolute; left: 0; right: 0; bottom: 0; background: linear-gradient(transparent, rgba(0,0,0,0.82)); color: #fff; opacity: 0; transition: opacity 180ms ease; pointer-events: none; }}
.cap-overlay .g-cap .cap-desc, .cap-overlay .g-cap .meta-badge {{ color: rgba(255,255,255,0.8); }}
.cap-overlay .g-card:hover .g-cap, .cap-overlay .g-card:focus-within .g-cap {{ opacity: 1; }}

/* Layout 1: Justified aspect-preserving rows */
.layout-justified {{ display: flex; flex-wrap: wrap; gap: var(--gap); }}
.layout-justified .g-card {{ flex: var(--jf-grow, 100) 1 var(--jf-basis, 260px); display: flex; flex-direction: column; }}
.layout-justified .g-frame {{ height: var(--row-h); }}
.layout-justified::after {{ content: ""; flex: 9999 1 0; }}

/* Layout 2: Balanced Masonry */
.layout-masonry {{ column-count: var(--cols-d); column-gap: var(--gap); }}
.layout-masonry .g-card {{ break-inside: avoid; margin-bottom: var(--gap); }}
.layout-masonry .g-frame img {{ height: auto; }}

/* Layout 3: Uniform Grid */
.layout-grid {{ display: grid; grid-template-columns: repeat(var(--cols-d), minmax(0, 1fr)); gap: var(--gap); }}
.layout-grid .g-frame {{ aspect-ratio: var(--grid-ar); }}

/* Layout 4: Editorial Photo Essay */
.layout-editorial {{ display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: var(--gap); }}
.layout-editorial .g-card {{ grid-column: span 3; }}
.layout-editorial .g-card:first-child, .layout-editorial .g-card.is-featured {{ grid-column: 1 / -1; }}
.layout-editorial .g-card:first-child .g-frame, .layout-editorial .g-card.is-featured .g-frame {{ max-height: 70vh; aspect-ratio: 16 / 9; }}
.layout-editorial .g-card:nth-child(5n+2), .layout-editorial .g-card:nth-child(5n+3), .layout-editorial .g-card:nth-child(5n+4) {{ grid-column: span 2; }}
.layout-editorial .g-frame {{ aspect-ratio: 4 / 3; }}

/* Layout 5: Filmstrip Showcase */
.layout-filmstrip {{ display: flex; gap: var(--gap); overflow-x: auto; scroll-snap-type: x mandatory; padding-bottom: 14px; }}
.layout-filmstrip .g-card {{ flex: 0 0 min(78vw, 680px); scroll-snap-align: center; }}
.layout-filmstrip .g-frame {{ aspect-ratio: 3 / 2; }}

@media (max-width: 1024px) {{
  .layout-masonry {{ column-count: var(--cols-t); }}
  .layout-grid {{ grid-template-columns: repeat(var(--cols-t), minmax(0, 1fr)); }}
  .layout-editorial .g-card, .layout-editorial .g-card:nth-child(5n+2), .layout-editorial .g-card:nth-child(5n+3), .layout-editorial .g-card:nth-child(5n+4) {{ grid-column: span 3; }}
}}
@media (max-width: 640px) {{
  .layout-masonry {{ column-count: var(--cols-m); }}
  .layout-grid {{ grid-template-columns: repeat(var(--cols-m), minmax(0, 1fr)); }}
  .layout-editorial .g-card {{ grid-column: 1 / -1 !important; }}
  .layout-justified .g-frame {{ height: auto; aspect-ratio: var(--ar); }}
}}

/* Accessible Lightbox */
.lb-backdrop {{ position: fixed; inset: 0; background: rgba(4,6,12,0.94); z-index: 9999; display: none; flex-direction: column; padding: 18px; color: #f3f4f6; }}
.lb-backdrop.open {{ display: flex; }}
.lb-bar {{ display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-bottom: 12px; }}
.lb-counter {{ font-size: 0.88rem; color: #9ca3af; font-variant-numeric: tabular-nums; }}
.lb-actions {{ display: flex; gap: 8px; }}
.lb-btn {{ background: rgba(255,255,255,0.10); color: #fff; border: 1px solid rgba(255,255,255,0.18); border-radius: 8px; padding: 7px 12px; cursor: pointer; font-size: 0.86rem; text-decoration: none; }}
.lb-btn:hover, .lb-btn:focus-visible {{ background: var(--accent); border-color: var(--accent); outline: none; }}
.lb-stage {{ flex: 1; min-height: 0; display: flex; align-items: center; justify-content: center; position: relative; overflow: auto; }}
.lb-stage img, .lb-stage video {{ max-width: 100%; max-height: 100%; object-fit: contain; border-radius: 6px; transition: transform 180ms ease; }}
.lb-stage.zoomed img {{ max-width: none; max-height: none; cursor: zoom-out; }}
.lb-nav {{ position: absolute; top: 50%; transform: translateY(-50%); background: rgba(0,0,0,0.55); border: 1px solid rgba(255,255,255,0.2); color: #fff; width: 44px; height: 44px; border-radius: 999px; font-size: 1.3rem; cursor: pointer; }}
.lb-prev {{ left: 10px; }}
.lb-next {{ right: 10px; }}
.lb-foot {{ margin-top: 12px; text-align: center; max-width: 820px; margin-inline: auto; }}
.lb-title {{ font-weight: 600; font-size: 1.02rem; }}
.lb-caption {{ color: #cbd5e1; font-size: 0.9rem; margin-top: 4px; }}
.lb-meta {{ color: #94a3b8; font-size: 0.78rem; margin-top: 6px; font-variant-numeric: tabular-nums; }}
.g-footer {{ margin-top: 40px; padding-top: 16px; border-top: 1px solid var(--border); color: var(--muted); font-size: 0.82rem; text-align: center; }}
</style>
</head>
<body>
<main class="g-shell">
  <header class="g-header">
    <h1 id="g-title">{title_esc}</h1>
    {subtitle_html}
    {author_html}
  </header>
  <section id="g-root" class="g-grid {layout_class} {caption_class}" aria-label="{title_esc}">
    {cards_html}
  </section>
  {footer_html}
</main>
<div id="lb" class="lb-backdrop" role="dialog" aria-modal="true" aria-label="Gallery viewer">
  <div class="lb-bar">
    <span id="lb-counter" class="lb-counter"></span>
    <div class="lb-actions">
      <button id="lb-zoom" class="lb-btn" type="button">Zoom</button>
      {dl_btn}
      <button id="lb-close" class="lb-btn" type="button">Close (Esc)</button>
    </div>
  </div>
  <div id="lb-stage" class="lb-stage">
    <button id="lb-prev" class="lb-nav lb-prev" type="button" aria-label="Previous photo">&#8249;</button>
    <div id="lb-media"></div>
    <button id="lb-next" class="lb-nav lb-next" type="button" aria-label="Next photo">&#8250;</button>
  </div>
  <div class="lb-foot">
    <div id="lb-title" class="lb-title"></div>
    <div id="lb-caption" class="lb-caption"></div>
    <div id="lb-meta" class="lb-meta"></div>
  </div>
</div>
<script id="g-data" type="application/json">{items_json}</script>
<script>
(() => {{
  const items = JSON.parse(document.getElementById('g-data').textContent || '[]');
  const lb = document.getElementById('lb');
  const lbMedia = document.getElementById('lb-media');
  const lbStage = document.getElementById('lb-stage');
  const lbTitle = document.getElementById('lb-title');
  const lbCaption = document.getElementById('lb-caption');
  const lbMeta = document.getElementById('lb-meta');
  const lbCounter = document.getElementById('lb-counter');
  const lbClose = document.getElementById('lb-close');
  const lbPrev = document.getElementById('lb-prev');
  const lbNext = document.getElementById('lb-next');
  const lbZoom = document.getElementById('lb-zoom');
  const lbDl = document.getElementById('lb-dl');
  const enableLightbox = {enable_lb};
  let cur = -1;
  let lastFocus = null;

  function renderSlide(idx) {{
    if (!items.length) return;
    cur = ((idx % items.length) + items.length) % items.length;
    const it = items[cur];
    lbStage.classList.remove('zoomed');
    lbMedia.replaceChildren();
    if (it.video_variants && it.video_variants.length > 0) {{
      const vid = document.createElement('video');
      vid.controls = true;
      vid.playsInline = true;
      vid.autoplay = true;
      vid.loop = true;
      for (const vv of it.video_variants) {{
        const src = document.createElement('source');
        src.src = vv.relative_path;
        src.type = vv.mime_type;
        vid.appendChild(src);
      }}
      lbMedia.appendChild(vid);
    }} else {{
      const pic = document.createElement('picture');
      const byFmt = new Map();
      for (const v of (it.variants || [])) {{
        if (!byFmt.has(v.mime_type)) byFmt.set(v.mime_type, []);
        byFmt.get(v.mime_type).push(v);
      }}
      let fallback = null;
      for (const [mime, list] of byFmt.entries()) {{
        list.sort((a, b) => a.width - b.width);
        if (mime === 'image/jpeg' || mime === 'image/png') {{
          fallback = list[list.length - 1];
        }} else {{
          const s = document.createElement('source');
          s.type = mime;
          s.srcset = list.map(v => v.relative_path + ' ' + v.width + 'w').join(', ');
          s.sizes = '100vw';
          pic.appendChild(s);
        }}
      }}
      const best = fallback || (it.variants && it.variants[it.variants.length - 1]);
      if (best) {{
        const img = document.createElement('img');
        img.src = best.relative_path;
        img.alt = it.alt_text || it.title || '';
        img.decoding = 'async';
        pic.appendChild(img);
      }}
      lbMedia.appendChild(pic);
    }}
    lbCounter.textContent = (cur + 1) + ' / ' + items.length;
    lbTitle.textContent = it.title || '';
    lbCaption.textContent = it.caption || '';
    const bits = [it.original_width + '×' + it.original_height];
    if (it.best_verified_score != null) bits.push('zensim_a ' + it.best_verified_score.toFixed(1));
    if (it.savings_pct > 0) bits.push(it.savings_pct.toFixed(0) + '% smaller');
    lbMeta.textContent = bits.join(' · ');
    if (lbDl && it.download_href) {{
      lbDl.href = it.download_href;
    }}
  }}

  function openLb(idx) {{
    if (!enableLightbox || !items.length) return;
    lastFocus = document.activeElement;
    renderSlide(idx);
    lb.classList.add('open');
    lbClose.focus();
  }}

  function closeLb() {{
    lb.classList.remove('open');
    lbMedia.replaceChildren();
    if (lastFocus && typeof lastFocus.focus === 'function') lastFocus.focus();
  }}

  document.querySelectorAll('.g-frame[data-index]').forEach((el) => {{
    el.addEventListener('click', () => openLb(Number(el.dataset.index)));
    el.addEventListener('keydown', (e) => {{
      if (e.key === 'Enter' || e.key === ' ') {{ e.preventDefault(); openLb(Number(el.dataset.index)); }}
    }});
  }});
  lbClose.addEventListener('click', closeLb);
  lbPrev.addEventListener('click', () => renderSlide(cur - 1));
  lbNext.addEventListener('click', () => renderSlide(cur + 1));
  lbZoom.addEventListener('click', () => lbStage.classList.toggle('zoomed'));
  lb.addEventListener('click', (e) => {{ if (e.target === lb || e.target === lbStage) closeLb(); }});
  document.addEventListener('keydown', (e) => {{
    if (!lb.classList.contains('open')) return;
    if (e.key === 'Escape') closeLb();
    else if (e.key === 'ArrowLeft') renderSlide(cur - 1);
    else if (e.key === 'ArrowRight') renderSlide(cur + 1);
    else if (e.key === 'Tab') {{
      const focusables = lb.querySelectorAll('button, a[href]');
      if (!focusables.length) return;
      const first = focusables[0], last = focusables[focusables.length - 1];
      if (e.shiftKey && document.activeElement === first) {{ e.preventDefault(); last.focus(); }}
      else if (!e.shiftKey && document.activeElement === last) {{ e.preventDefault(); first.focus(); }}
    }}
  }});
  {preview_bridge}
}})();
</script>
</body>
</html>"#,
        title_esc = escape_html(&config.title),
        bg = tok.bg,
        surface = tok.surface,
        surface_2 = tok.surface_2,
        border = tok.border,
        text = tok.text,
        muted = tok.muted,
        accent = accent,
        shadow = tok.shadow,
        fonts = fonts,
        gap = d.gap_px,
        radius = d.corner_radius_px,
        max_w = d.max_width_px,
        pad = d.padding_px,
        row_h = d.target_row_height_px,
        cols_d = d.columns_desktop,
        cols_t = d.columns_tablet,
        cols_m = d.columns_mobile,
        grid_ar = grid_ar,
        cursor = if d.enable_lightbox { "pointer" } else { "default" },
        dl_btn = if d.show_download_button {
            "<a id=\"lb-dl\" class=\"lb-btn\" download>Download</a>"
        } else {
            ""
        },
        enable_lb = if d.enable_lightbox { "true" } else { "false" },
    )
}
