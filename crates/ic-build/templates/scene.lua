local scene = { current = nil, scenes = {} }

function scene.add(name, s)
    scene.scenes[name] = s
end

function scene.switch(name, ...)
    local s = scene.scenes[name]
    if not s then
        error("нет сцены: " .. tostring(name))
    end
    if scene.current and scene.current.leave then
        scene.current.leave()
    end
    scene.current = s
    if s.enter then
        s.enter(...)
    end
end

function scene.update(dt)
    local s = scene.current
    if s and s.update then
        s.update(dt)
    end
end

function scene.fixed_update(dt)
    local s = scene.current
    if s and s.fixed_update then
        s.fixed_update(dt)
    end
end

return scene
