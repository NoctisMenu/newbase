-- scripts/example.lua
-- Callback contract:
--   on_load(ctx)   -> optional
--   on_tick(ctx)   -> optional
--   on_unload()    -> optional
--
-- `ctx.players` and `ctx.entities` are refreshed every tick.
-- Memory primitives are exposed through `read.*`.

local last_health_log_ms = 0

function on_load(ctx)
    console.info("example.lua loaded")
end

function on_tick(ctx)
    local lp = ctx.local_player
    if lp == nil then
        return
    end

    if lp.max_health > 0 and lp.health / lp.max_health < 0.35 then
        if ctx.timestamp_ms - last_health_log_ms > 1000 then
            console.warn(string.format("low HP: %d / %d", lp.health, lp.max_health))
            last_health_log_ms = ctx.timestamp_ms
        end
    end

    -- Example of custom read primitives:
    local base = process.client_base()
    local offs = process.offsets()
    local entity_list = read.ptr(base + offs.entity_list)
    if entity_list == nil then
        return
    end
end

function on_unload()
    console.info("example.lua unloaded")
end
