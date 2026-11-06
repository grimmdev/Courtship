-- Courtship LOVE Runtime Bridge
-- Provides safe error containment, deterministic input simulation, and MCP TCP inspection.

local bridge = {
    version = "1.0.1",
    port = 12345,
    server = nil,
    clients = {},
    objects_registry = {},
    last_error = nil,
    paused = false,
    input_queue = {},
    active_keys = {},
    deterministic_mode = false,
    fixed_dt = 1 / 60
}

local socket = nil
pcall(function() socket = require("socket") end)

-------------------------------------------------------------------------------
-- Safe Crash Interception (love.errhand override)
-------------------------------------------------------------------------------

--- Inspects the stack trace and captures call-site local variables upon panic.
--- @param msg any Error message or object passed to love.errhand
--- @return table Diagnostic payload containing file, line, traceback, and locals
local function extract_crash_diagnostics(msg)
    local err_text = tostring(msg)
    local tb = debug.traceback("", 2)

    local file, line = err_text:match("([^:%s]+):(%d+):")
    if not file then
        file, line = tb:match("([^:%s]+):(%d+):")
    end

    local captured_locals = {}
    local stack_depth = 3
    while stack_depth <= 6 do
        local var_index = 1
        while true do
            local name, val = debug.getlocal(stack_depth, var_index)
            if not name then break end
            if not name:find("^%(") then
                local formatted = tostring(val)
                if type(val) == "table" then
                    formatted = "<table: " .. tostring(val) .. ">"
                end
                captured_locals[name] = formatted
            end
            var_index = var_index + 1
        end
        stack_depth = stack_depth + 1
    end

    return {
        file = file or "unknown",
        line = tonumber(line) or 0,
        message = err_text,
        traceback = tb,
        locals = captured_locals,
        timestamp = os.time()
    }
end

--- Installs the non-fatal error handler replacing the default blue screen.
function bridge.install_safe_error_handler()
    if not love then return end

    love.errhand = function(msg)
        bridge.last_error = extract_crash_diagnostics(msg)
        bridge.paused = true
        print(string.format("[Courtship Sandbox] Intercepted runtime error at %s:%d: %s",
            bridge.last_error.file, bridge.last_error.line, bridge.last_error.message
        ))

        if not love.window or not love.window.isOpen() then
            return
        end

        -- Maintain event processing so the TCP bridge and MCP client stay alive
        while true do
            love.event.pump()
            for name, a in love.event.poll() do
                if name == "quit" then
                    return a or 0
                end
            end

            bridge.poll(0.016)

            -- Check if code hot-reload or manual intervention cleared the fault
            if not bridge.last_error then
                bridge.paused = false
                break
            end

            -- Render non-intrusive diagnostic overlay
            if love.graphics and love.graphics.isActive() then
                love.graphics.origin()
                love.graphics.clear(0.10, 0.10, 0.14)

                -- Diagnostic header bar
                love.graphics.setColor(0.90, 0.22, 0.22)
                love.graphics.rectangle("fill", 20, 20, 760, 42)
                love.graphics.setColor(1, 1, 1)
                love.graphics.print("[Courtship Safe Sandbox] Runtime Panic Intercepted - Bridge Active", 32, 34)

                -- Diagnostic content
                love.graphics.setColor(0.85, 0.85, 0.90)
                local summary = string.format("Source: %s:%d\nDetail: %s\n\nGame loop paused. Send hot-patch or run_lua to resume.",
                    bridge.last_error.file, bridge.last_error.line, bridge.last_error.message
                )
                love.graphics.printf(summary, 32, 84, 736)
                love.graphics.present()
            end

            if love.timer then
                love.timer.sleep(0.016)
            end
        end
    end
end

-------------------------------------------------------------------------------
-- Agent Joypad (simulate_input & deterministic frame stepping)
-------------------------------------------------------------------------------

--- Dispatches a sequence of timed keyboard input events.
--- @param sequence table Array of input action tables: { action = "press"|"keydown"|"keyup", key = string, duration_ms = number }
--- @return boolean Success status
function bridge.simulate_input(sequence)
    if type(sequence) ~= "table" then return false end

    for _, event in ipairs(sequence) do
        local key = event.key or "space"
        local action = event.action or "press"
        local duration_secs = (event.duration_ms or 100) / 1000

        if action == "keydown" then
            bridge.active_keys[key] = true
            if love.keypressed then love.keypressed(key) end
        elseif action == "keyup" then
            bridge.active_keys[key] = nil
            if love.keyreleased then love.keyreleased(key) end
        elseif action == "press" then
            bridge.active_keys[key] = true
            if love.keypressed then love.keypressed(key) end
            table.insert(bridge.input_queue, {
                key = key,
                remaining = duration_secs
            })
        end
    end
    return true
end

--- Advances game physics and logic deterministically by N frames.
--- @param frame_count number Number of frames to advance
--- @param dt number|nil Fixed delta time per frame (defaults to 1/60)
--- @return table State report after stepped frames
function bridge.step_frames(frame_count, dt)
    local count = tonumber(frame_count) or 1
    local step_dt = tonumber(dt) or bridge.fixed_dt

    bridge.deterministic_mode = true

    for _ = 1, count do
        bridge.update_inputs(step_dt)
        if love.update then
            love.update(step_dt)
        end
    end

    return {
        stepped_frames = count,
        dt_applied = step_dt,
        active_objects = bridge.get_objects()
    }
end

--- Updates active key duration timers and dispatches releases on expiration.
--- @param dt number Delta time in seconds
function bridge.update_inputs(dt)
    local index = 1
    while index <= #bridge.input_queue do
        local item = bridge.input_queue[index]
        item.remaining = item.remaining - dt
        if item.remaining <= 0 then
            bridge.active_keys[item.key] = nil
            if love.keyreleased then
                love.keyreleased(item.key)
            end
            table.remove(bridge.input_queue, index)
        else
            index = index + 1
        end
    end
end

-------------------------------------------------------------------------------
-- Bridge Core API
-------------------------------------------------------------------------------

--- Initializes TCP server listener and hooks the safe error sandbox.
--- @param opts table Configuration options ({ port = number })
function bridge.init(opts)
    opts = opts or {}
    bridge.port = opts.port or bridge.port
    print(string.format("[Courtship Bridge v%s] Initializing on port %d...", bridge.version, bridge.port))

    if socket then
        local s, err = socket.bind("127.0.0.1", bridge.port)
        if s then
            s:settimeout(0)
            bridge.server = s
            print("[Courtship Bridge] TCP listener established.")
        else
            print("[Courtship Bridge] Warning: Could not bind TCP socket: " .. tostring(err))
        end
    else
        print("[Courtship Bridge] luasocket not available; operating in headless mode.")
    end

    bridge.install_safe_error_handler()
end

--- Registers an entity table for inspection and projection.
--- @param id string Unique entity identifier
--- @param obj_type string Classification name (e.g. 'Player', 'Coin')
--- @param tbl table Reference to the game entity table
function bridge.register_object(id, obj_type, tbl)
    bridge.objects_registry[id] = {
        id = id,
        type = obj_type,
        ref = tbl
    }
end

--- Returns a list of all registered objects with coordinates and identity.
--- @return table Array of entity state snapshots
function bridge.get_objects()
    local result = {}
    for id, meta in pairs(bridge.objects_registry) do
        local ref = meta.ref or {}
        table.insert(result, {
            id = id,
            type = meta.type,
            x = ref.x or 0,
            y = ref.y or 0,
            name = ref.name or id
        })
    end
    return result
end

--- Safely executes arbitrary Lua code within the game state.
--- @param code string Lua source snippet
--- @return table Execution result status
function bridge.run_lua(code)
    local chunk, parse_err = load(code)
    if not chunk then
        return { success = false, error = tostring(parse_err) }
    end
    local ok, eval_res = pcall(chunk)
    if not ok then
        return { success = false, error = tostring(eval_res) }
    end
    return { success = true, result = eval_res }
end

--- Clears the active crash state and resumes the standard game loop.
--- @return table Operation status
function bridge.clear_crash()
    bridge.last_error = nil
    bridge.paused = false
    return { status = "cleared", resumed = true }
end

--- Retrieves the last intercepted crash diagnostics without clearing them.
--- @return table Crash payload or clean status
function bridge.get_last_crash()
    return bridge.last_error or { status = "clean", message = "No active crash" }
end

--- Main per-frame polling routine called by love.update.
--- @param dt number Delta time in seconds
function bridge.poll(dt)
    bridge.update_inputs(dt or 0.016)

    if not bridge.server then return end
    local incoming = bridge.server:accept()
    if incoming then
        incoming:settimeout(0)
        table.insert(bridge.clients, incoming)
    end
end

return bridge
