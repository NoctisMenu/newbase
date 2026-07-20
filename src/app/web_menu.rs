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
  width: 31px; height: 24px; padding: 2px; border-radius: 6px;
  border: 1px solid var(--border); background: var(--bg-raised); cursor: pointer;
}
.config-slider { width: 132px; display: grid; gap: 3px; text-align: right; }
.config-slider input { width: 132px; }
.config-readonly { color: var(--text-dim); font-family: ui-monospace, monospace; }
.field-description { padding: 0 0 5px; color: var(--text-faint); font-size: 10.5px; }
"#;

const MENU_SCRIPT: &str = r#"
const ipc = payload => {
  try { window.ipc.postMessage(JSON.stringify(payload)); } catch (_) {}
};

window.__newbaseSetVisible = visible => {
  document.getElementById('ui').style.display = visible ? 'block' : 'none';
  const hud = document.getElementById('fps-hud');
  if (hud) hud.style.display = visible ? 'none' : 'block';
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
document.querySelectorAll('input[type=range][data-config-key]').forEach(input => {
  const output = document.querySelector(`[data-value-for="${CSS.escape(input.dataset.configKey)}"]`);
  input.addEventListener('input', () => {
    const integer = input.dataset.kind === 'int';
    const value = integer ? parseInt(input.value, 10) : parseFloat(input.value);
    if (output) output.textContent = integer ? String(value) : Number(value).toFixed(2);
    ipc({type: 'config', key: input.dataset.configKey, value});
  });
});
document.querySelectorAll('select[data-config-key], input.config-text[data-config-key]').forEach(input => {
  input.addEventListener('change', () => ipc({type: 'config', key: input.dataset.configKey, value: input.value}));
});
document.querySelectorAll('input.config-color[data-config-key]').forEach(input => {
  input.addEventListener('input', () => {
    const value = input.value.slice(1);
    ipc({type: 'config', key: input.dataset.configKey, value: {
      r: parseInt(value.slice(0, 2), 16), g: parseInt(value.slice(2, 4), 16),
      b: parseInt(value.slice(4, 6), 16), a: Number(input.dataset.alpha || 255)
    }});
  });
});
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuCommand {
    None,
    Quit,
    SetVisible(bool),
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
            html.replace_range(content_start..content_start + relative_end, &sections);
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
                r##"<input class="config-color" type="color" data-config-key="{}" data-alpha="{}" value="#{:02X}{:02X}{:02X}">"##,
                escape_attr(key),
                a,
                r,
                g,
                b
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
        assert!(html.contains("<select class=\"config-select\""));
        assert!(html.contains("Draw the overlay"));
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
