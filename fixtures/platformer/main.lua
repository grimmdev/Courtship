-- Sample LÖVE Platformer Fixture
local player = {
    x = 100,
    y = 300,
    vx = 0,
    vy = 0,
    jumps = 0,
    max_jumps = 1 -- Intentionally flawed for the 'fix-double-jump' benchmark task
}

local coins = {}

function love.load()
    print("Platformer fixture loaded.")
end

function love.update(dt)
    player.vy = player.vy + 980 * dt
    player.y = player.y + player.vy * dt
    if player.y > 400 then
        player.y = 400
        player.vy = 0
        player.jumps = 0
    end
end

function love.keypressed(key)
    if key == "space" then
        if player.jumps < 1 then -- Bug: only allows 1 jump
            player.vy = -400
            player.jumps = player.jumps + 1
        end
    end
end

function love.draw()
    love.graphics.clear(0.1, 0.1, 0.15)
    love.graphics.setColor(0.2, 0.8, 0.3)
    love.graphics.rectangle("fill", player.x, player.y, 32, 48)
    
    love.graphics.setColor(1, 0.84, 0)
    for _, coin in ipairs(coins) do
        love.graphics.circle("fill", coin.x, coin.y, 8)
    end
end

