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

