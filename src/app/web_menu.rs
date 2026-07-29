use super::config_system::{ConfigStore, ConfigValue, FieldSchema, FieldType, WidgetType};

const TEMPLATE: &str = include_str!("../../resources/frontend.html");

const EXTRA_CSS: &str = r#"
#ui { display: none; }
.category-label {
  padding: 8px 0 3px; color: var(--text-faint); font-size: 9.5px;
  font-weight: 600; letter-spacing: .06em; text-transform: uppercase;
}
.config-select, .config-text {
  min-width: 108px; max-width: 145px; padding: 5px 7px; border-radius: 6px;
  border: 1px solid var(--border); background: var(--bg-raised);
  color: var(--text); font: inherit; outline: none;
}
.config-select:focus, .config-text:focus { border-color: var(--accent); }
.config-color {
  appearance: none; width: 31px; height: 24px; padding: 0; flex-shrink: 0;
  border: 1px solid var(--border-strong); background: var(--bg-raised); cursor: pointer;
}
.config-color:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.config-slider { width: 132px; display: grid; gap: 3px; text-align: right; }
.config-slider input { width: 132px; }
.config-readonly { color: var(--text-dim); font-family: ui-monospace, monospace; }
.field-description { padding: 0 0 5px; color: var(--text-faint); font-size: 10.5px; }
#log-panel { display: flex; flex-direction: column; margin-top: 6px; border-top: 1px solid var(--border); }
#log-toolbar { display: flex; align-items: center; gap: 6px; padding: 5px 0; }
#log-toolbar button { padding: 2px 8px; border-radius: 4px; border: 1px solid var(--border); background: var(--bg-raised); color: var(--text); font: inherit; font-size: 10px; cursor: pointer; }
#log-level-wrap { position: relative; }
#log-level-menu { display: none; position: absolute; bottom: 100%; left: 0; margin-bottom: 3px; flex-direction: column; min-width: 64px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg-raised); overflow: hidden; z-index: 20; }
#log-level-menu.visible { display: flex; }
#log-level-menu button { border: none; border-radius: 0; text-align: left; padding: 4px 8px; }
#log-entries { display: none; height: 170px; overflow-y: auto; font-family: ui-monospace, monospace; font-size: 10px; line-height: 1.5; padding: 4px 0; }
#log-entries.visible { display: block; }
#log-entries .log-line { white-space: pre-wrap; word-break: break-all; padding: 0 2px; }
#log-entries .log-error { color: #f87171; }
#log-entries .log-warn { color: #fbbf24; }
#log-entries .log-info { color: var(--text-dim); }
#log-entries .log-debug { color: #6b7280; }
#log-entries .log-trace { color: #4b5563; }
#log-entries .log-time { color: var(--text-faint); margin-right: 4px; }
#log-entries .log-src { color: var(--accent); margin-right: 4px; }
"#;

const MENU_SCRIPT: &str = r#"
(() => {
const ipc = payload => {
  try {
    window.ipc.postMessage(JSON.stringify(payload));
    return true;
  } catch (error) {
    console.error('[newbase menu] IPC failed', error, payload);
    return false;
  }
};
const debug = (message, details = {}) => {
  console.log(`[newbase menu] ${message}`, details);
  ipc({type: 'debug', message, readyState: document.readyState, details});
};

window.addEventListener('error', event => {
  ipc({
    type: 'js_error',
    message: event.message || 'unknown window error',
    filename: event.filename || '',
    line: event.lineno || 0,
    column: event.colno || 0
  });
});
window.addEventListener('unhandledrejection', event => {
  ipc({type: 'js_error', message: `Unhandled promise rejection: ${String(event.reason)}`});
});

debug('script started', {
  href: location.href,
  uiFound: Boolean(document.getElementById('ui')),
  bodyChildren: document.body?.children.length ?? -1,
  viewport: `${innerWidth}x${innerHeight}`
});

window.__newbaseSetVisible = visible => {
  const ui = document.getElementById('ui');
  if (!ui) {
    debug('visibility failed: #ui not found', {visible});
    return false;
  }
  ui.style.display = visible ? 'block' : 'none';
  const hud = document.getElementById('fps-hud');
  if (hud) hud.style.display = visible ? 'none' : 'block';
  const rect = ui.getBoundingClientRect();
  debug('visibility applied', {
    visible,
    inlineDisplay: ui.style.display,
    computedDisplay: getComputedStyle(ui).display,
    rect: {x: rect.x, y: rect.y, width: rect.width, height: rect.height}
  });
  return true;
};
window.__newbaseSetFps = fps => {
  const text = Math.max(0, Number(fps) || 0).toFixed(0) + ' fps';
  const main = document.getElementById('fps');
  const hud = document.getElementById('fps-hud');
  if (main) main.textContent = text;
  if (hud) hud.textContent = text;
};

document.getElementById('theme-btn')?.addEventListener('click', () => {
  document.body.classList.toggle('light');
});

document.querySelectorAll('.section-header').forEach(header => {
  header.addEventListener('click', () => header.parentElement.classList.toggle('collapsed'));
});

(() => {
  const panel = document.getElementById('ui');
  const header = document.getElementById('header');
  let dragging = false, sx = 0, sy = 0, ox = 0, oy = 0;
  header?.addEventListener('mousedown', event => {
    if (event.target.closest('.icon-btn')) return;
    const rect = panel.getBoundingClientRect();
    dragging = true; sx = event.clientX; sy = event.clientY;
    ox = rect.left; oy = rect.top; event.preventDefault();
  });
  window.addEventListener('mousemove', event => {
    if (!dragging) return;
    panel.style.left = Math.max(0, Math.min(innerWidth - panel.offsetWidth, ox + event.clientX - sx)) + 'px';
    panel.style.top = Math.max(0, Math.min(innerHeight - panel.offsetHeight, oy + event.clientY - sy)) + 'px';
  });
  window.addEventListener('mouseup', () => dragging = false);
})();

(() => {
  const button = document.getElementById('settings-btn');
  const menu = document.getElementById('settings-menu');
  button?.addEventListener('click', event => {
    event.stopPropagation(); menu.classList.toggle('visible');
  });
  document.addEventListener('click', event => {
    if (!menu?.contains(event.target) && event.target !== button) menu?.classList.remove('visible');
  });
})();

document.getElementById('dismiss-btn')?.addEventListener('click', () => {
  document.getElementById('settings-menu')?.classList.remove('visible');
  document.getElementById('modal-backdrop')?.classList.add('visible');
});
document.getElementById('modal-cancel')?.addEventListener('click', () => {
  document.getElementById('modal-backdrop')?.classList.remove('visible');
});
document.getElementById('modal-confirm')?.addEventListener('click', () => {
  document.getElementById('modal-backdrop')?.classList.remove('visible');
  window.__newbaseSetVisible(false); ipc({type: 'visibility', visible: false});
});
document.getElementById('quit-btn')?.addEventListener('click', () => {
  document.getElementById('settings-menu')?.classList.remove('visible');
  document.getElementById('quit-backdrop')?.classList.add('visible');
});
document.getElementById('quit-cancel')?.addEventListener('click', () => {
  document.getElementById('quit-backdrop')?.classList.remove('visible');
});
document.getElementById('quit-confirm')?.addEventListener('click', () => ipc({type: 'quit'}));

document.querySelectorAll('.switch[data-config-key]').forEach(button => {
  button.addEventListener('click', () => {
    const value = !button.classList.contains('active');
    button.classList.toggle('active', value);
    button.setAttribute('aria-pressed', String(value));
    ipc({type: 'config', key: button.dataset.configKey, value});
  });
});
const updateSliderFill = input => {
  const min = Number(input.min || 0);
  const max = Number(input.max || 100);
  const value = Number(input.value);
  const percent = max > min ? ((value - min) / (max - min)) * 100 : 0;
  input.style.setProperty('--fill', `${Math.max(0, Math.min(100, percent))}%`);
};
document.querySelectorAll('input[type=range][data-config-key]').forEach(input => {
  const output = document.querySelector(`[data-value-for="${CSS.escape(input.dataset.configKey)}"]`);
  const emitValue = () => {
    const integer = input.dataset.kind === 'int';
    const value = integer ? parseInt(input.value, 10) : parseFloat(input.value);
    if (output) output.textContent = integer ? String(value) : Number(value).toFixed(2);
    updateSliderFill(input);
    ipc({type: 'config', key: input.dataset.configKey, value});
  };
  const setSyntheticSliderValue = clientX => {
    const rect = input.getBoundingClientRect();
    if (rect.width <= 0) return;
    const min = Number(input.min || 0);
    const max = Number(input.max || 100);
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    let value = min + (max - min) * ratio;
    if (input.dataset.kind === 'int') {
      value = Math.round(value);
    } else if (input.step && input.step !== 'any') {
      const step = Number(input.step);
      if (Number.isFinite(step) && step > 0) value = min + Math.round((value - min) / step) * step;
    }
    input.value = String(Math.max(min, Math.min(max, value)));
    input.dispatchEvent(new Event('input', {bubbles: true}));
  };

  // Composite forwards off-screen WebView input as synthetic MouseEvents.
  // Chromium intentionally does not run the native range-control drag action
  // for untrusted events, so reproduce it from the event's client coordinate.
  let syntheticDrag = false;
  input.addEventListener('mousedown', event => {
    if (event.isTrusted || event.button !== 0) return;
    syntheticDrag = true;
    setSyntheticSliderValue(event.clientX);
    debug('synthetic slider drag started', {key: input.dataset.configKey, x: event.clientX});
  });
  input.addEventListener('mousemove', event => {
    if (!syntheticDrag || event.isTrusted) return;
    setSyntheticSliderValue(event.clientX);
    if ((event.buttons & 1) === 0) syntheticDrag = false;
  });
  input.addEventListener('mouseup', event => {
    if (!syntheticDrag || event.isTrusted || event.button !== 0) return;
    setSyntheticSliderValue(event.clientX);
    syntheticDrag = false;
    debug('synthetic slider drag finished', {key: input.dataset.configKey, value: input.value});
  });
  input.addEventListener('input', emitValue);
  updateSliderFill(input);
});
document.querySelectorAll('select[data-config-key], input.config-text[data-config-key]').forEach(input => {
  input.addEventListener('change', () => ipc({type: 'config', key: input.dataset.configKey, value: input.value}));
});
// Schema colors use the template's custom HSV picker. A native
// <input type="color"> cannot open from Composite's synthetic WebView events.
(() => {
  const swatches = document.querySelectorAll('.config-color[data-config-key]');
  const pop = document.getElementById('color-popover');
  const wheel = document.getElementById('wheel');
  const cursor = document.getElementById('wheel-cursor');
  const valBar = document.getElementById('val-slider');
  const hexInput = document.getElementById('hex-input');
  if (!swatches.length || !pop || !wheel || !cursor || !valBar || !hexInput) return;

  const context = wheel.getContext('2d');
  const size = wheel.width;
  const radius = size / 2;
  let hue = 0, saturation = 0, brightness = 0;
  let active = null;
  let wheelDrag = false;
  let valueDrag = false;

  const hsvToRgb = (h, s, v) => {
    const chroma = v * s;
    const sector = (h % 360) / 60;
    const x = chroma * (1 - Math.abs(sector % 2 - 1));
    let r = 0, g = 0, b = 0;
    if (sector < 1) { r = chroma; g = x; }
    else if (sector < 2) { r = x; g = chroma; }
    else if (sector < 3) { g = chroma; b = x; }
    else if (sector < 4) { g = x; b = chroma; }
    else if (sector < 5) { r = x; b = chroma; }
    else { r = chroma; b = x; }
    const match = v - chroma;
    return [r, g, b].map(channel => Math.round((channel + match) * 255));
  };
  const rgbToHsv = (red, green, blue) => {
    const r = red / 255, g = green / 255, b = blue / 255;
    const maximum = Math.max(r, g, b);
    const minimum = Math.min(r, g, b);
    const delta = maximum - minimum;
    let h = 0;
    if (delta !== 0) {
      if (maximum === r) h = ((g - b) / delta) % 6;
      else if (maximum === g) h = (b - r) / delta + 2;
      else h = (r - g) / delta + 4;
      h *= 60;
      if (h < 0) h += 360;
    }
    return [h, maximum === 0 ? 0 : delta / maximum, maximum];
  };
  const rgbToHex = (r, g, b) => '#' + [r, g, b]
    .map(channel => channel.toString(16).padStart(2, '0'))
    .join('').toUpperCase();

  const drawWheel = () => {
    const image = context.createImageData(size, size);
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const dx = x - radius, dy = y - radius;
        const distance = Math.sqrt(dx * dx + dy * dy);
        const index = (y * size + x) * 4;
        if (distance > radius) {
          image.data[index + 3] = 0;
          continue;
        }
        let angle = Math.atan2(dy, dx) * 180 / Math.PI;
        if (angle < 0) angle += 360;
        const [r, g, b] = hsvToRgb(angle, Math.min(1, distance / radius), brightness);
        image.data[index] = r;
        image.data[index + 1] = g;
        image.data[index + 2] = b;
        image.data[index + 3] = distance > radius - 1
          ? Math.round((radius - distance) * 255)
          : 255;
      }
    }
    context.putImageData(image, 0, 0);
  };
  const updateIndicators = () => {
    const angle = hue * Math.PI / 180;
    const distance = saturation * radius;
    const [r, g, b] = hsvToRgb(hue, saturation, brightness);
    cursor.style.left = `${radius + Math.cos(angle) * distance}px`;
    cursor.style.top = `${radius + Math.sin(angle) * distance}px`;
    cursor.style.background = `rgb(${r},${g},${b})`;
    const full = hsvToRgb(hue, saturation, 1);
    valBar.style.background = `linear-gradient(to right, #000, rgb(${full[0]},${full[1]},${full[2]}))`;
    valBar.style.setProperty('--val-pos', `${brightness * 100}%`);
  };
  const commit = updateHex => {
    if (!active) return;
    const [r, g, b] = hsvToRgb(hue, saturation, brightness);
    const hex = rgbToHex(r, g, b);
    active.dataset.value = hex;
    active.style.background = hex;
    if (updateHex) hexInput.value = hex;
    ipc({type: 'config', key: active.dataset.configKey, value: {
      r, g, b, a: Number(active.dataset.alpha || 255)
    }});
  };
  const setFromHex = hex => {
    if (!/^#[0-9a-f]{6}$/i.test(hex)) return false;
    const packed = parseInt(hex.slice(1), 16);
    [hue, saturation, brightness] = rgbToHsv(
      (packed >> 16) & 255,
      (packed >> 8) & 255,
      packed & 255
    );
    drawWheel();
    updateIndicators();
    return true;
  };
  const pickWheel = event => {
    const rect = wheel.getBoundingClientRect();
    const scaleX = size / rect.width;
    const scaleY = size / rect.height;
    const x = (event.clientX - rect.left) * scaleX - radius;
    const y = (event.clientY - rect.top) * scaleY - radius;
    saturation = Math.min(1, Math.sqrt(x * x + y * y) / radius);
    hue = Math.atan2(y, x) * 180 / Math.PI;
    if (hue < 0) hue += 360;
    updateIndicators();
    commit(true);
  };
  const pickValue = event => {
    const rect = valBar.getBoundingClientRect();
    if (rect.width <= 0) return;
    brightness = Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width));
    drawWheel();
    updateIndicators();
    commit(true);
  };
  const close = () => {
    pop.classList.remove('visible');
    active = null;
    wheelDrag = false;
    valueDrag = false;
  };
  const open = swatch => {
    active = swatch;
    const hex = swatch.dataset.value || '#FFFFFF';
    setFromHex(hex);
    hexInput.value = hex.toUpperCase();
    pop.classList.add('visible');
    const swatchRect = swatch.getBoundingClientRect();
    const popRect = pop.getBoundingClientRect();
    let left = swatchRect.right + 8;
    if (left + popRect.width > innerWidth - 6) left = swatchRect.left - popRect.width - 8;
    const top = Math.max(6, Math.min(innerHeight - popRect.height - 6, swatchRect.top));
    pop.style.left = `${Math.max(6, left)}px`;
    pop.style.top = `${top}px`;
    drawWheel();
    updateIndicators();
    debug('color picker opened', {key: swatch.dataset.configKey, value: hex});
  };

  swatches.forEach(swatch => {
    swatch.addEventListener('click', event => {
      event.stopPropagation();
      if (pop.classList.contains('visible') && active === swatch) close();
      else open(swatch);
    });
  });
  wheel.addEventListener('mousedown', event => {
    if (event.button !== 0) return;
    wheelDrag = true;
    pickWheel(event);
    event.preventDefault();
  });
  valBar.addEventListener('mousedown', event => {
    if (event.button !== 0) return;
    valueDrag = true;
    pickValue(event);
    event.preventDefault();
  });
  window.addEventListener('mousemove', event => {
    if (wheelDrag) pickWheel(event);
    if (valueDrag) pickValue(event);
  });
  window.addEventListener('mouseup', () => {
    wheelDrag = false;
    valueDrag = false;
  });
  hexInput.addEventListener('input', () => {
    let value = hexInput.value.trim();
    if (value && !value.startsWith('#')) value = '#' + value;
    if (setFromHex(value)) commit(false);
  });
  pop.addEventListener('mousedown', event => event.stopPropagation());
  document.addEventListener('mousedown', event => {
    if (!pop.contains(event.target) && !event.target.closest?.('.config-color')) close();
  });
  window.addEventListener('keydown', event => {
    if (event.key === 'Escape' && pop.classList.contains('visible')) close();
  });
})();

// Tell the native overlay that the generated DOM and visibility hook exist.
// The overlay must not issue its one-time reveal before this message arrives:
// WebView2 navigation is asynchronous and early evaluate_script calls are
// otherwise silently ignored.
debug('event bindings complete', {
  fields: document.querySelectorAll('[data-config-key]').length,
  sections: document.querySelectorAll('.section').length,
  uiFound: Boolean(document.getElementById('ui'))
});

// --- Log viewer ---
const LOG_RANK = {error: 1, warn: 2, info: 3, debug: 4, trace: 5};
window.__logBuffer = [];
window.__logLevel = 'info';
const logEntriesEl = () => document.getElementById('log-entries');
const logPasses = entry => (LOG_RANK[entry.l] || 5) <= (LOG_RANK[window.__logLevel] || 3);
const logLineNode = entry => {
  const line = document.createElement('div');
  line.className = 'log-line log-' + entry.l;
  const time = document.createElement('span');
  time.className = 'log-time';
  time.textContent = entry.t.toFixed(1) + 's';
  const src = document.createElement('span');
  src.className = 'log-src';
  src.textContent = entry.src;
  line.appendChild(time);
  line.appendChild(src);
  line.appendChild(document.createTextNode(entry.m));
  return line;
};
const renderAllLogs = () => {
  const container = logEntriesEl();
  if (!container) return;
  const visible = window.__logBuffer.filter(logPasses);
  const slice = visible.length > 300 ? visible.slice(visible.length - 300) : visible;
  const fragment = document.createDocumentFragment();
  for (const entry of slice) fragment.appendChild(logLineNode(entry));
  container.replaceChildren(fragment);
  container.scrollTop = container.scrollHeight;
};
window.__newbasePushLogs = entries => {
  const container = logEntriesEl();
  if (!container || !entries.length) return;
  const stickToBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 24;
  const fragment = document.createDocumentFragment();
  let appended = 0;
  for (const entry of entries) {
    window.__logBuffer.push(entry);
    if (logPasses(entry)) { fragment.appendChild(logLineNode(entry)); appended++; }
  }
  if (window.__logBuffer.length > 600) window.__logBuffer.splice(0, window.__logBuffer.length - 600);
  if (appended) {
    container.appendChild(fragment);
    while (container.children.length > 300) container.removeChild(container.firstChild);
    if (stickToBottom) container.scrollTop = container.scrollHeight;
  }
};
(() => {
  const levelBtn = document.getElementById('log-level-btn');
  const levelMenu = document.getElementById('log-level-menu');
  const label = value => value.charAt(0).toUpperCase() + value.slice(1);
  levelBtn?.addEventListener('click', event => {
    event.stopPropagation();
    levelMenu.classList.toggle('visible');
  });
  levelMenu?.querySelectorAll('button[data-level]').forEach(option => {
    option.addEventListener('click', event => {
      event.stopPropagation();
      const level = option.dataset.level;
      window.__logLevel = level;
      levelBtn.textContent = label(level);
      levelMenu.classList.remove('visible');
      ipc({type: 'log_level', level});
      renderAllLogs();
    });
  });
  document.addEventListener('click', event => {
    if (levelMenu && !levelMenu.contains(event.target) && event.target !== levelBtn) {
      levelMenu.classList.remove('visible');
    }
  });
})();
document.getElementById('log-up')?.addEventListener('click', () => {
  const container = logEntriesEl();
  if (container) container.scrollBy({top: -60});
});
document.getElementById('log-down')?.addEventListener('click', () => {
  const container = logEntriesEl();
  if (container) container.scrollBy({top: 60});
});
document.getElementById('log-clear')?.addEventListener('click', () => {
  window.__logBuffer = [];
  const container = logEntriesEl();
  if (container) container.replaceChildren();
});
document.getElementById('log-toggle')?.addEventListener('click', () => {
  document.getElementById('log-entries')?.classList.toggle('visible');
});

const readySent = ipc({type: 'ready'});
console.log('[newbase menu] ready dispatched', {readySent});
})();
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuCommand {
    None,
    Ready,
    Quit,
    SetVisible(bool),
    SetLogLevel(log::LevelFilter),
}

pub(crate) fn build_html(store: &ConfigStore) -> String {
    let mut html = TEMPLATE.replace("{{BOX_HEX}}", "#6366f1");

    if let (Some(start), Some(end)) = (
        html.find("<div id=\"splash\">"),
        html.find("<div id=\"ui\">"),
    ) {
        html.replace_range(start..end, "");
    }

    if let Some(style_end) = html.find("</style>") {
        html.insert_str(style_end, EXTRA_CSS);
    }

    html = html.replacen("<span>Overlay</span>", "<span>newbase</span>", 1);
    html = html.replace("Hide menu until restart", "Hide menu");
    html = html.replace(
        "The menu cannot be reopened until the program restarts.",
        "Press Insert to show the menu again.",
    );
    let sections = render_sections(store);
    if let Some(body_open) = html.find("<div id=\"body\">") {
        let content_start = body_open + "<div id=\"body\">".len();
        if let Some(relative_end) = html[content_start..].find("</div><!-- #body -->") {
            let log_panel = r#"<div id="log-panel"><div id="log-toolbar"><button id="log-toggle" type="button">Logs</button><div id="log-level-wrap"><button id="log-level-btn" type="button">Info</button><div id="log-level-menu"><button type="button" data-level="error">Error</button><button type="button" data-level="warn">Warn</button><button type="button" data-level="info">Info</button><button type="button" data-level="debug">Debug</button><button type="button" data-level="trace">Trace</button></div></div><button id="log-up" type="button">&#9650;</button><button id="log-down" type="button">&#9660;</button><button id="log-clear" type="button">Clear</button></div><div id="log-entries"></div></div>"#;
            let replacement = format!("{sections}{log_panel}");
            html.replace_range(content_start..content_start + relative_end, &replacement);
        }
    }

    if let (Some(script_start), Some(script_end)) =
        (html.rfind("<script>"), html.rfind("</script>"))
    {
        html.replace_range(
            script_start..script_end + "</script>".len(),
            &format!("<script>{MENU_SCRIPT}</script>"),
        );
    }
    html
}

pub(crate) fn apply_message(store: &mut ConfigStore, message: &str) -> Result<MenuCommand, String> {
    let payload: serde_json::Value =
        serde_json::from_str(message).map_err(|error| error.to_string())?;
    match payload.get("type").and_then(serde_json::Value::as_str) {
        Some("debug") => {
            let message = payload
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("(no message)");
            let ready_state = payload
                .get("readyState")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let details = payload
                .get("details")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            log::info!(
                "WebView menu HTML: {message} (readyState={ready_state}, details={details})"
            );
            Ok(MenuCommand::None)
        }
        Some("ready") => Ok(MenuCommand::Ready),
        Some("quit") => Ok(MenuCommand::Quit),
        Some("visibility") => Ok(MenuCommand::SetVisible(
            payload
                .get("visible")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        )),
        Some("config") => {
            let key = payload
                .get("key")
                .and_then(serde_json::Value::as_str)
                .ok_or("config message has no key")?;
            let value = payload.get("value").ok_or("config message has no value")?;
            let field_type = store
                .get_field_schema(key)
                .map(|field| field.field_type.clone())
                .ok_or_else(|| format!("unknown config key '{key}'"))?;
            match field_type {
                FieldType::Bool => store.set_bool(key, value.as_bool().ok_or("expected bool")?),
                FieldType::Float { .. } => {
                    store.set_float(key, value.as_f64().ok_or("expected number")? as f32)
                }
                FieldType::Int { .. } => {
                    store.set_int(key, value.as_i64().ok_or("expected integer")? as i32)
                }
                FieldType::Color => {
                    let channel = |name| {
                        value
                            .get(name)
                            .and_then(serde_json::Value::as_u64)
                            .map(|v| v.min(255) as u8)
                    };
                    store.set_color_rgba(
                        key,
                        channel("r").ok_or("color has no red channel")?,
                        channel("g").ok_or("color has no green channel")?,
                        channel("b").ok_or("color has no blue channel")?,
                        channel("a").unwrap_or(255),
                    )
                }
                FieldType::Enum { .. } => store.set_enum(
                    key,
                    value.as_str().ok_or("expected enum string")?.to_owned(),
                ),
                FieldType::String => {
                    store.set_string(key, value.as_str().ok_or("expected string")?.to_owned())
                }
            }
            .map_err(|error| error.to_string())?;
            Ok(MenuCommand::None)
        }
        Some("js_error") => Err(format!("WebView JavaScript error: {payload}")),
        Some("log_level") => {
            let level = match payload
                .get("level")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("info")
            {
                "error" => log::LevelFilter::Error,
                "warn" => log::LevelFilter::Warn,
                "debug" => log::LevelFilter::Debug,
                "trace" => log::LevelFilter::Trace,
                _ => log::LevelFilter::Info,
            };
            Ok(MenuCommand::SetLogLevel(level))
        }
        _ => Ok(MenuCommand::None),
    }
}

fn render_sections(store: &ConfigStore) -> String {
    let mut sections: Vec<_> = store.schema().sections.iter().collect();
    sections.sort_by(|(a_key, a), (b_key, b)| {
        a.display_name.cmp(&b.display_name).then(a_key.cmp(b_key))
    });
    let mut html = String::new();
    for (index, (section_key, section)) in sections.into_iter().enumerate() {
        html.push_str(&format!(
            r##"<div class="section{}" data-section="{}"><div class="section-header"><span>{}</span><svg class="chev" width="10" height="10"><use href="#ico-chevron"/></svg></div><div class="section-body">"##,
            if index == 0 { "" } else { " collapsed" },
            escape_attr(section_key),
            escape_html(&section.display_name)
        ));
        let mut fields: Vec<_> = section
            .fields
            .iter()
            .filter(|(_, field)| field.public)
            .collect();
        fields.sort_by(|(a_key, a), (b_key, b)| {
            a.metadata
                .category
                .cmp(&b.metadata.category)
                .then(a.metadata.display_name.cmp(&b.metadata.display_name))
                .then(a_key.cmp(b_key))
        });
        let mut category = None::<&str>;
        for (field_key, field) in fields {
            if category != Some(field.metadata.category.as_str()) {
                category = Some(&field.metadata.category);
                if !field.metadata.category.is_empty() {
                    html.push_str(&format!(
                        r#"<div class="category-label">{}</div>"#,
                        escape_html(&field.metadata.category)
                    ));
                }
            }
            let key = format!("{section_key}.{field_key}");
            html.push_str(&render_field(&key, field, store.value(&key)));
        }
        html.push_str("</div></div>");
    }
    if html.is_empty() {
        html.push_str(r#"<div class="section"><div class="section-body"><div class="field-description">No public fields were found in the loaded config schema.</div></div></div>"#);
    }
    html
}

fn render_field(key: &str, field: &FieldSchema, value: Option<&ConfigValue>) -> String {
    let label = escape_html(&field.metadata.display_name);
    let tooltip = if field.metadata.tooltip.is_empty() {
        &field.metadata.description
    } else {
        &field.metadata.tooltip
    };
    let help = if tooltip.is_empty() {
        String::new()
    } else {
        format!(
            r##"<span class="help" data-tip="{}"><svg><use href="#ico-help"/></svg></span>"##,
            escape_attr(tooltip)
        )
    };
    let widget = match (&field.widget_type, &field.field_type, value) {
        (
            WidgetType::Checkbox | WidgetType::Toggle,
            FieldType::Bool,
            Some(ConfigValue::Bool(current)),
        ) => format!(
            r#"<button class="switch{}" data-config-key="{}" aria-pressed="{}"></button>"#,
            if *current { " active" } else { "" },
            escape_attr(key),
            current
        ),
        (
            WidgetType::SmoothSlider { .. },
            FieldType::Float { min, max },
            Some(ConfigValue::Float(current)),
        ) => slider(key, *min, *max, *current, false),
        (
            WidgetType::SmoothSlider { .. },
            FieldType::Int { min, max },
            Some(ConfigValue::Int(current)),
        ) => slider(key, *min, *max, *current, true),
        (WidgetType::ColorPicker, FieldType::Color, Some(ConfigValue::Color { r, g, b, a })) => {
            format!(
                r##"<button type="button" class="color-swatch config-color" data-config-key="{}" data-alpha="{}" data-value="#{:02X}{:02X}{:02X}" style="background:#{:02X}{:02X}{:02X}" aria-label="Choose {}"></button>"##,
                escape_attr(key),
                a,
                r,
                g,
                b,
                r,
                g,
                b,
                escape_attr(&field.metadata.display_name),
            )
        }
        (WidgetType::ComboBox, FieldType::Enum { variants }, Some(ConfigValue::Enum(current))) => {
            let options = variants
                .iter()
                .map(|variant| {
                    format!(
                        r#"<option value="{}"{}>{}</option>"#,
                        escape_attr(variant),
                        if variant == current { " selected" } else { "" },
                        escape_html(variant)
                    )
                })
                .collect::<String>();
            format!(
                r#"<select class="config-select" data-config-key="{}">{options}</select>"#,
                escape_attr(key)
            )
        }
        (_, FieldType::String, Some(ConfigValue::String(current)))
            if !matches!(field.widget_type, WidgetType::None) =>
        {
            format!(
                r#"<input class="config-text" type="text" data-config-key="{}" value="{}">"#,
                escape_attr(key),
                escape_attr(current)
            )
        }
        _ => format!(
            r#"<span class="config-readonly">{}</span>"#,
            escape_html(&display_value(value))
        ),
    };
    let description = if field.metadata.description.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="field-description">{}</div>"#,
            escape_html(&field.metadata.description)
        )
    };
    format!(
        r#"<div class="row"><div class="label-wrap"><span>{label}</span>{help}</div>{widget}</div>{description}"#
    )
}

fn slider<T: std::fmt::Display>(key: &str, min: T, max: T, value: T, integer: bool) -> String {
    let kind = if integer { "int" } else { "float" };
    let step = if integer { "1" } else { "any" };
    format!(
        r#"<div class="config-slider"><span class="val" data-value-for="{}">{}</span><input type="range" data-config-key="{}" data-kind="{}" min="{}" max="{}" step="{}" value="{}"></div>"#,
        escape_attr(key),
        value,
        escape_attr(key),
        kind,
        min,
        max,
        step,
        value
    )
}

fn display_value(value: Option<&ConfigValue>) -> String {
    match value {
        Some(ConfigValue::Bool(value)) => value.to_string(),
        Some(ConfigValue::Float(value)) => format!("{value:.2}"),
        Some(ConfigValue::Int(value)) => value.to_string(),
        Some(ConfigValue::Color { r, g, b, a }) => format!("#{r:02X}{g:02X}{b:02X}{a:02X}"),
        Some(ConfigValue::Enum(value) | ConfigValue::String(value)) => value.clone(),
        None => "—".to_owned(),
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value)
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_is_replaced_with_runtime_sections() {
        let store = ConfigStore::load_from_schema_str(
            r##"version = 1
[sections.visuals]
display_name = "Visuals"
[sections.visuals.fields.enabled]
type = "bool"
widget_type = "toggle"
default = true
public = true
[sections.visuals.fields.enabled.metadata]
display_name = "Enabled"
description = "Draw the overlay"
tooltip = ""
category = "General"

[sections.visuals.fields.distance]
type = "float"
min = 0.0
max = 500.0
widget_type = "smoothslider"
default = 125.0
public = true
[sections.visuals.fields.distance.metadata]
display_name = "Distance"
description = "Maximum render distance"
tooltip = ""
category = "General"

[sections.visuals.fields.count]
type = "int"
min = 1
max = 10
widget_type = "smoothslider"
default = 3
public = true
[sections.visuals.fields.count.metadata]
display_name = "Count"
description = ""
tooltip = ""
category = "Advanced"

[sections.visuals.fields.tint]
type = "color"
widget_type = "colorpicker"
default = "#112233CC"
public = true
[sections.visuals.fields.tint.metadata]
display_name = "Tint"
description = ""
tooltip = "Tint color"
category = "General"

[sections.visuals.fields.mode]
type = "enum"
variants = ["corner", "filled"]
widget_type = "combobox"
default = "corner"
public = true
[sections.visuals.fields.mode.metadata]
display_name = "Mode"
description = ""
tooltip = ""
category = "General"
"##,
            "target/test-web-menu.toml",
        )
        .unwrap();
        let html = build_html(&store);
        assert!(html.contains("data-config-key=\"visuals.enabled\""));
        assert!(html.contains("data-config-key=\"visuals.distance\""));
        assert!(html.contains("data-config-key=\"visuals.tint\""));
        assert!(html.contains("class=\"color-swatch config-color\""));
        assert!(html.contains("color picker opened"));
        assert!(html.contains("<select class=\"config-select\""));
        assert!(html.contains("Draw the overlay"));
        assert!(html.contains("(() => {\nconst ipc = payload =>"));
        assert!(html.contains("synthetic slider drag started"));
        assert!(html.contains("input.style.setProperty('--fill'"));
        assert_eq!(html.matches("<script>").count(), 1);
        assert!(!html.contains("{{"));
        assert!(!html.contains("id=\"splash\""));
    }

    #[test]
    fn ipc_messages_update_typed_config_values() {
        let mut store = ConfigStore::load_from_schema_str(
            r##"version = 1
[sections.runtime]
display_name = "Runtime"
[sections.runtime.fields.enabled]
type = "bool"
widget_type = "toggle"
default = false
[sections.runtime.fields.enabled.metadata]
display_name = "Enabled"
category = "General"
[sections.runtime.fields.amount]
type = "int"
min = 0
max = 10
widget_type = "smoothslider"
default = 1
[sections.runtime.fields.amount.metadata]
display_name = "Amount"
category = "General"
[sections.runtime.fields.tint]
type = "color"
widget_type = "colorpicker"
default = "#000000FF"
[sections.runtime.fields.tint.metadata]
display_name = "Tint"
category = "General"
"##,
            "target/test-web-menu-ipc.toml",
        )
        .unwrap();

        assert_eq!(
            apply_message(&mut store, r#"{"type":"ready"}"#).unwrap(),
            MenuCommand::Ready
        );

        assert_eq!(
            apply_message(
                &mut store,
                r#"{"type":"config","key":"runtime.enabled","value":true}"#,
            )
            .unwrap(),
            MenuCommand::None
        );
        apply_message(
            &mut store,
            r#"{"type":"config","key":"runtime.amount","value":7}"#,
        )
        .unwrap();
        apply_message(
            &mut store,
            r#"{"type":"config","key":"runtime.tint","value":{"r":10,"g":20,"b":30,"a":40}}"#,
        )
        .unwrap();

        assert!(matches!(
            store.value("runtime.enabled"),
            Some(ConfigValue::Bool(true))
        ));
        assert!(matches!(
            store.value("runtime.amount"),
            Some(ConfigValue::Int(7))
        ));
        assert!(matches!(
            store.value("runtime.tint"),
            Some(ConfigValue::Color {
                r: 10,
                g: 20,
                b: 30,
                a: 40
            })
        ));
    }
}
