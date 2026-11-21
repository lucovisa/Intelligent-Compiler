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
            tex = n.tex,
            text = n.text,
            size = n.size,
        }
        level.nodes[#level.nodes + 1] = node
        level.by_name[n.name] = node
    end

    function level.draw()
        for _, n in ipairs(level.nodes) do
            if n.visual then
                if n.tex ~= "" then
                    ic.image(n.tex, n.x, n.y, n.w, n.h, n.rot, 1, 1, 1, n.a)
                elseif n.kind ~= "Label" then
                    ic.rect(n.x, n.y, n.w, n.h, n.r, n.g, n.b, n.a, n.rot)
                end
                if n.text ~= "" then
                    if n.kind == "Label" then
                        ic.text(n.text, n.x, n.y, n.size, n.r, n.g, n.b, n.a)
                    else
                        ic.text(n.text, n.x, n.y, n.size, 1, 1, 1, 1)
                    end
                end
            end
        end
    end

    function level.at(x, y)
        for i = #level.nodes, 1, -1 do
            local n = level.nodes[i]
            if n.visual and math.abs(x - n.x) <= n.w / 2 and math.abs(y - n.y) <= n.h / 2 then
                return n
            end
        end
    end

    function level.clicked(name)
        local n = level.by_name[name]
        if not n or not ic.mouse_pressed(1) then
            return false
        end
        local mx, my = ic.mouse()
        return math.abs(mx - n.x) <= n.w / 2 and math.abs(my - n.y) <= n.h / 2
    end

    return level
end

return scene
