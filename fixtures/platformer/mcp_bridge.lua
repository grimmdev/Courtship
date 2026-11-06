-- Copy of mcp_bridge.lua for fixture game
local ok, bridge = pcall(require, "mcp_bridge")
if ok and bridge then return bridge end
return dofile("../../crates/bridge/assets/mcp_bridge.lua")
