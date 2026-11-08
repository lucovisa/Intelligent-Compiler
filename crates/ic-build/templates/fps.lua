local fps = { value = 0, frames = 0, timer = 0, history = {}, size = 60 }

function fps.update(dt)
    fps.frames = fps.frames + 1
    fps.timer = fps.timer + dt
    local h = fps.history
    h[#h + 1] = dt
    if #h > fps.size then
        table.remove(h, 1)
    end
    if fps.timer >= 1 then
        fps.value = fps.frames / fps.timer
        fps.frames = 0
        fps.timer = 0
        print(string.format("FPS: %d", math.floor(fps.value + 0.5)))
    end
end

function fps.draw(x, y)
    x = x or -600
    y = y or 340
    for i, dt in ipairs(fps.history) do
        local h = math.min(dt * 1000, 40)
        local good = dt < 1 / 55
        if good then
            ic.rect(x + i * 4, y - 20 + h / 2, 3, h, 0.3, 1, 0.3, 0.9)
        else
            ic.rect(x + i * 4, y - 20 + h / 2, 3, h, 1, 0.4, 0.3, 0.9)
        end
    end
end

return fps
