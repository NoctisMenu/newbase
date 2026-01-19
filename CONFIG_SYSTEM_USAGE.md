# Declarative TOML Config System - Usage Guide

## Overview

The new config system allows you to define configuration fields in `config_schema.toml` instead of writing Rust structs. The schema is embedded into the binary at compile time, so you don't need to distribute it separately. Changes to config automatically save only public fields to `config.toml`.


### 1. Accessing Config Values

```rust
use crate::app::config_system::keys;

// Get values (returns Result<T>)
let aim_enabled = self.config_store.get_bool(keys::AIM_ENABLED)?;
let aim_fov = self.config_store.get_float(keys::AIM_FOV)?;
let accent_color = self.config_store.get_color(keys::ACCENT_COLOR)?;
let refresh_rate = self.config_store.get_enum(keys::REFRESH_RATE)?;

// Set values
self.config_store.set_bool(keys::AIM_ENABLED, true)?;
self.config_store.set_float(keys::AIM_FOV, 12.5)?;  // Validates range automatically
self.config_store.set_color(keys::ACCENT_COLOR, Color32::RED)?;
```




## Adding New Config Fields

1. **Edit `config_schema.toml`:**

```toml
[sections.aim.fields.aim_prediction]
type = "bool"
widget = "checkbox"
default = false
public = true
[sections.aim.fields.aim_prediction.metadata]
display_name = "Aim Prediction"
description = "Predict enemy movement"
category = "Aim"
```

2. **Rebuild the application:**

Since the schema is embedded at compile time, you need to rebuild the binary after editing `config_schema.toml`:
```bash
cargo build --release
```

That's it! No struct definitions, no Default impl, no boilerplate.

## Available Config Keys

All keys are in `src/app/config_system/keys.rs`:


## Public vs Private Fields

Fields marked `public = false` in schema are NOT saved to `config.toml`:

```toml
[sections.core.fields.streamproof]
public = false  # Not visible in user's config.toml
```

This is useful for:
- Internal state
- Security/privacy settings
- Temporary flags

## Error Handling

All config operations return `Result<T, ConfigError>`:

```rust
// With ? operator
let value = self.config_store.get_bool(keys::AIM_ENABLED)?;

// With unwrap_or
let value = self.config_store.get_bool(keys::AIM_ENABLED).unwrap_or(false);

// With ok()
self.config_store.render_checkbox(ui, keys::AIM_ENABLED, accent).ok();

// With match
match self.config_store.set_float(keys::AIM_FOV, 20.0) {
    Ok(_) => {},
    Err(ConfigError::OutOfRange { .. }) => {
        log::error!("Value out of range!");
    }
    Err(e) => log::error!("Config error: {}", e),
}
```
