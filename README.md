# newbase

Windows game-overlay base with a schema-driven WebView menu rendered through
[`composite`](https://github.com/B1Fr0st/composite).

## Menu generation

Consumer applications provide their existing `config_schema.toml` through
`AppBuilder::with_config_schema_str` or `AppBuilder::with_config_schema_path`.
At startup, newbase generates the menu directly from that runtime schema:

- schema sections become collapsible menu sections;
- metadata categories group related fields;
- toggle/checkbox widgets render boolean switches;
- smooth sliders render validated float or integer ranges;
- color pickers edit RGBA color values;
- combo boxes render enum variants;
- private fields are omitted.

Every WebView change is sent through `window.ipc.postMessage`, validated against
the schema, written to `ConfigStore`, and marked dirty for the existing config
persistence flow.

The supplied UI design is embedded at `resources/frontend.html`. Its original
newbase logo animation remains an ImGui foreground sequence; the WebView menu is
revealed only after that sequence completes.

## Dependency

The repository tracks the default texture-composition branch directly:

```toml
composite = { git = "https://github.com/B1Fr0st/composite", branch = "webview-texture" }
```
