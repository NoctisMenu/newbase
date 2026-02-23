# Lua Scripting Interface

The runtime loads every `*.lua` file in the `scripts/` folder and hot-reloads on file changes.

## Lifecycle callbacks

Every script can define:

```lua
function on_load(ctx) end
function on_tick(ctx) end
function on_unload() end
```

`on_tick(ctx)` is called continuously from the `lua` thread.

## Tick context (`ctx`)

- `ctx.timestamp_ms`: unix time in milliseconds.
- `ctx.player_count`: number of players in snapshot.
- `ctx.entity_count`: number of entities in snapshot.
- `ctx.local_player`: local player table, or `nil`.
- `ctx.players`: array of player tables.
- `ctx.entities`: array of entity tables.

### Player table

- `index`: player index in snapshot.
- `pos`: `{ x, y, z }`
- `alive`, `health`, `max_health`, `is_local`, `team_id`, `ult_cd`
- `hero`: hero name string
- `hero_id`: hero enum integer
- `bones`: array of `{ x, y, z }`
- `skeleton_links`: array of index arrays
- `abilities`: array of ability tables

### Ability table

- `slot`: enum name string (ex: `ESlot_Weapon_Primary`)
- `slot_id`: enum numeric value
- `cooling_down`, `channeling`, `cooldown_start`, `cooldown_end`, `data_ptr`

### Entity table

- `name`
- `type`: `"soul"` or `"creep"`
- `visible`
- `attackable`
- `pos`: `{ x, y, z }`

## Read primitives (`read`)

Typed reads (return value or `nil`):

- `read.u8(addr)`, `read.u16(addr)`, `read.u32(addr)`, `read.u64(addr)`
- `read.i8(addr)`, `read.i16(addr)`, `read.i32(addr)`, `read.i64(addr)`
- `read.f32(addr)`, `read.f64(addr)`
- `read.bool(addr)`
- `read.ptr(addr)`

String/byte reads:

- `read.cstr(addr [, max_len])` (default 96, max 1024)
- `read.bytes(addr, len)` (max 4096, returns Lua string or `nil`)

Pointer chain helper:

- `read.ptr_chain(base, { off1, off2, ... })`

## Process helpers (`process`)

- `process.client_base()`
- `process.offsets()` returning:
  - `entity_list`
  - `view_matrix`

## Logging (`console`)

- `console.info(msg)`
- `console.warn(msg)`
- `console.error(msg)`

Log lines are tagged with script file name.
