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

function scene.load(name)
    local data = require("scenes." .. name)
    local level = { nodes = {}, by_name = {} }
    for _, n in ipairs(data) do
        local node = {
            kind = n.kind,
            name = n.name,
            x = n.x,
            y = n.y,
            w = n.w,
            h = n.h,
            rot = n.rot,
            r = n.r,
            g = n.g,
            b = n.b,
            a = n.a,
            visual = n.visual,
        }
        level.nodes[#level.nodes + 1] = node
        level.by_name[n.name] = node
    end
    function level.draw()
        for _, n in ipairs(level.nodes) do
            if n.visual then
                ic.rect(n.x, n.y, n.w, n.h, n.r, n.g, n.b, n.a, n.rot)
            end
        end
    end
    return level
end

return scene
