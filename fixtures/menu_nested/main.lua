-- Menu-Nested Fixture Game (Stresses depth capping and nested object graph traversal)
local scene_tree = {
    id = "root_scene",
    type = "Scene",
    canvas = {
        id = "main_canvas",
        layer = 1,
        window = {
            id = "settings_modal",
            title = "Audio & Video Options",
            layout = {
                tab_panel = {
                    active_tab = "graphics",
                    column = {
                        width = 400,
                        row = {
                            id = "resolution_row",
                            button = {
                                id = "toggle_fullscreen",
                                label = "Fullscreen",
                                style = {
                                    bg = "#333333",
                                    border = {
                                        color = "#aaaaaa",
                                        radius = 4,
                                        deep_physics = {
                                            mass = 1.0,
                                            damping = 0.5
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

function love.load()
    print("Nested menu fixture loaded.")
end

function love.update(dt) end

function love.draw()
    love.graphics.clear(0.1, 0.1, 0.1)
    love.graphics.setColor(1, 1, 1)
    love.graphics.print("Nested Menu Fixture Active", 100, 100)
end

