-- UI-Heavy Fixture Game (Stresses draw calls, HUD rendering, and screenshots)
local hud = {
    hp = 78,
    max_hp = 100,
    mana = 45,
    max_mana = 50,
    score = 14200,
    inventory = {
        { id = "sword_iron", name = "Iron Sword", slot = 1, count = 1 },
        { id = "potion_health", name = "Healing Draught", slot = 2, count = 5 },
        { id = "bomb", name = "Cherry Bomb", slot = 3, count = 12 }
    }
}

function love.load()
    print("UI-Heavy fixture initialized.")
end

function love.update(dt)
    -- Periodic UI animations
end

function love.draw()
    love.graphics.clear(0.05, 0.05, 0.08)

    -- Status Bars
    love.graphics.setColor(0.8, 0.1, 0.1)
    love.graphics.rectangle("fill", 20, 20, (hud.hp / hud.max_hp) * 200, 20)
    love.graphics.setColor(1, 1, 1)
    love.graphics.rectangle("line", 20, 20, 200, 20)
    love.graphics.print(string.format("HP: %d/%d", hud.hp, hud.max_hp), 25, 24)

    -- Mana Bar
    love.graphics.setColor(0.1, 0.3, 0.9)
    love.graphics.rectangle("fill", 20, 48, (hud.mana / hud.max_mana) * 200, 16)
    love.graphics.setColor(1, 1, 1)
    love.graphics.rectangle("line", 20, 48, 200, 16)
    love.graphics.print(string.format("MP: %d/%d", hud.mana, hud.max_mana), 25, 50)

    -- Inventory Grid (many draw calls)
    for col = 0, 7 do
        for row = 0, 3 do
            local x = 20 + col * 48
            local y = 400 + row * 48
            love.graphics.setColor(0.2, 0.2, 0.25)
            love.graphics.rectangle("fill", x, y, 44, 44)
            love.graphics.setColor(0.5, 0.5, 0.6)
            love.graphics.rectangle("line", x, y, 44, 44)
        end
    end

    -- Minimap readout
    love.graphics.setColor(0.15, 0.2, 0.15)
    love.graphics.rectangle("fill", 620, 20, 160, 160)
    love.graphics.setColor(0.3, 0.8, 0.3)
    love.graphics.rectangle("line", 620, 20, 160, 160)
    love.graphics.circle("fill", 700, 100, 4) -- Player blip
end

