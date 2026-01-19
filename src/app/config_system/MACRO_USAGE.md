# Config System Macros

Efficient macros for accessing and modifying configuration values using string-based field names.

## Available Macros

### `config!` - Get config values from App

The primary macro for reading config values through the App struct.

**Usage:**
```rust
impl App {
    pub fn some_function(&self) {
        // Get boolean values
        let aim_enabled = config!(self, "aim_enabled");
        let streamproof = config!(self, "streamproof");

        // Get float values
        let fov = config!(self, "aim_fov");
        let smoothing = config!(self, "aim_smoothing");

        // Get enum values (as String)
        let refresh_rate = config!(self, "refresh_rate");

        // Get color values
        let accent = config!(self, "accent_color");
    }
}
```

### `config_get!` - Get config values from ConfigStore

Use this when you have direct access to a ConfigStore instance.

**Usage:**
```rust
let config_store = ConfigStore::load().unwrap();

let enabled = config_get!(config_store, "aim_enabled");
let fov = config_get!(config_store, "aim_fov");
```

### `config_set!` - Set config values

Modify configuration values efficiently.

**Usage:**
```rust
impl App {
    pub fn toggle_feature(&mut self) {
        // Set boolean
        config_set!(self, "aim_enabled", true);

        // Set float
        config_set!(self, "aim_fov", 12.5);

        // Set enum
        config_set!(self, "refresh_rate", "Hz144");

        // Set color
        config_set!(self, "accent_color", Color32::from_rgb(255, 0, 0));
    }
}
```

## Supported Fields

| Field Name | Type | Default | Macro Example |
|------------|------|---------|---------------|
| `aim_enabled` | bool | `false` | `config!(self, "aim_enabled")` |
| `aim_fov` | f32 | `5.0` | `config!(self, "aim_fov")` |
| `aim_smoothing` | f32 | `0.0` | `config!(self, "aim_smoothing")` |
| `humanize_aim` | bool | `true` | `config!(self, "humanize_aim")` |
| `streamproof` | bool | `true` | `config!(self, "streamproof")` |
| `discord_presence` | bool | `true` | `config!(self, "discord_presence")` |
| `spotify` | bool | `true` | `config!(self, "spotify")` |
| `accent_color` | Color32 | `GREEN` | `config!(self, "accent_color")` |
| `refresh_rate` | String | `"Hz240"` | `config!(self, "refresh_rate")` |
| `accent_hex` | String | `""` | `config!(self, "accent_hex")` |

## Benefits

1. **Compile-time checking** - Field names are checked at compile time
2. **Type safety** - Correct getter/setter is used for each field type
3. **Default values** - Built-in fallback values if config is missing
4. **Cleaner code** - Less boilerplate than manual key lookups
5. **Auto-completion** - Easy to discover available fields

## Example: Before and After

**Before (manual key access):**
```rust
use crate::app::config_system::keys;

let discord_enabled = self.config_store
    .get_bool(keys::DISCORD_PRESENCE)
    .unwrap_or(false);

if discord_enabled {
    // ...
}
```

**After (using macro):**
```rust
let discord_enabled = config!(self, "discord_presence");

if discord_enabled {
    // ...
}
```

## Adding New Fields

When new fields are added to `config_schema.toml`, update the macros in `macros.rs`:

1. Add a getter arm to `config!` macro
2. Add a getter arm to `config_get!` macro
3. Add a setter arm to `config_set!` macro
4. Update this documentation table

The macro ensures type-safe access and provides compile-time errors if used incorrectly.
