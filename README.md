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

## Remote function calls

On 64-bit Windows, an app can open its attached game process and invoke a
function using the Microsoft x64 ABI:

```rust,no_run
use std::time::Duration;
use newbase::RemoteArgument;

let process = app.remote_process()?;
let result = unsafe {
    process.call(
        function_address,
        &[
            RemoteArgument::from(object_address),
            RemoteArgument::from(10_u32),
            RemoteArgument::from(0.5_f32),
        ],
        Duration::from_secs(2),
    )?
};
let integer_return = result.as_usize();
```

The function address and signature must be valid in the target. Integer and
pointer returns are read from `RAX`; floating-point returns are available with
`as_f32()` or `as_f64()`. Up to 16 integer, pointer, `f32`, or `f64` arguments
are supported. Aggregate/vector values, non-Microsoft calling conventions, and
variadic functions are not supported.

### Remote hooks

`RemoteHook` installs reversible x64 entry-point detours. It decodes whole
instructions, relocates relative calls/jumps, conditional branches, loop-family
branches, and RIP-relative memory operands into a trampoline, then selects a
5-byte relative jump or a register-preserving 14-byte absolute jump for the
entry patch.

```rust,no_run
let mut hook = unsafe {
    app.install_remote_detour(target_function, replacement_function)?
};

// The replacement can use this address to invoke the relocated original body.
let original_function = hook.trampoline_address();

// Target threads must be kept away from the entry while it is being patched.
unsafe { hook.disable()? };
unsafe { hook.enable()? };
unsafe { hook.remove()? };
```

`RemoteHook::install_code` can instead copy a position-independent machine-code
prelude into dynamically selected free memory in the target. It appends a jump
through the relocated original instructions automatically. Hook and trampoline
memory is initially writable, changed to executable/read-only before use, and
released after the original entry is restored.

All hook instruction reads and writes use newbase's kernel-driver memory path.
Consequently, hooks can only be created after newbase has fully initialized the
driver for the attached game process. Windows virtual-memory APIs are still used
to locate and allocate free regions, change page protection, and flush the
instruction cache; they are not used to transfer hook or target bytes.

## Dependency

The repository tracks the default texture-composition branch directly:

```toml
composite = { git = "https://github.com/B1Fr0st/composite", branch = "webview-texture" }
```
