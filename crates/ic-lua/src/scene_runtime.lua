local skip = {
    Node = true,
    Node2D = true,
    Marker2D = true,
    Camera2D = true,
    Timer = true,
    AudioStreamPlayer = true,
    AudioStreamPlayer2D = true,
    Path2D = true,
    AnimationPlayer = true,
    NavigationRegion2D = true,
    CollisionShape2D = true,
    RayCast2D = true,
    CanvasLayer = true,
    Control = true,
    ParallaxBackground = true,
    TileMap = true,
}

local current = nil
local fps, fps_time, fps_frames = 0, 0, 0

local function unescape(s)
    return (s:gsub("%%0A", "\n"):gsub("%%7C", "|"):gsub("%%25", "%%"))
end

local function split(line)
    local out = {}
    for part in (line .. "|"):gmatch("([^|]*)|") do
        out[#out + 1] = part
    end
    return out
end

local function linear(v)
    local c = v / 255
    if c <= 0.04045 then
        return c / 12.92
    end
    return ((c + 0.055) / 1.055) ^ 2.4
end

local function parse(text)
    local scene = { nodes = {}, by_name = {}, bodies = {}, solids = {} }
    for line in text:gmatch("[^\r\n]+") do
        local p = split(line)
        if p[1] == "node" and #p >= 17 then
            local n = {
                kind = p[2],
                name = unescape(p[3]),
                x = tonumber(p[4]) or 0,
                y = tonumber(p[5]) or 0,
                w = tonumber(p[6]) or 0,
                h = tonumber(p[7]) or 0,
                r = linear(tonumber(p[8]) or 255),
                g = linear(tonumber(p[9]) or 255),
                b = linear(tonumber(p[10]) or 255),
                rot = math.rad(tonumber(p[11]) or 0),
                a = (tonumber(p[12]) or 255) / 255,
                visible = p[13] ~= "0",
                tex = unescape(p[15] or ""),
                text = unescape(p[16] or ""),
                size = tonumber(p[17]) or 24,
                gravity = tonumber(p[19]) or 900,
                bounce = tonumber(p[20]) or 0,
                parent_name = unescape(p[18] or ""),
                children = {},
                vx = 0,
                vy = 0,
            }
            scene.nodes[#scene.nodes + 1] = n
            scene.by_name[n.name] = n
            if n.kind == "Body" then
                scene.bodies[#scene.bodies + 1] = n
            elseif n.kind == "Solid" then
                scene.solids[#scene.solids + 1] = n
            end
        end
    end
    for _, n in ipairs(scene.nodes) do
        local parent = scene.by_name[n.parent_name]
        if parent and parent ~= n then
            n.parent = parent
            parent.children[#parent.children + 1] = n
        end
    end
    return scene
end

local function world(n)
    local x, y = n.x, n.y
    local p = n.parent
    while p do
        x, y = x + p.x, y + p.y
        p = p.parent
    end
    return x, y
end

local function shown(n)
    while n do
        if not n.visible then
            return false
        end
        n = n.parent
    end
    return true
end

function ic.world(n)
    return world(n)
end

function ic.scene_load(path)
    local text = ic._read(path)
    if not text then
        error("нет сцены: " .. tostring(path), 2)
    end
    current = parse(text)
    ic.scene = current
    return current
end

function ic.scene_unload()
    current = nil
    ic.scene = nil
end

function ic.node(name)
    return current and current.by_name[name]
end

function ic._scene_boot()
    if ic._read("main.ic") then
        ic.scene_load("main.ic")
    end
end

function ic._scene_fixed(dt)
    if not current then
        return
    end
    for _, b in ipairs(current.bodies) do
        b.vy = b.vy - b.gravity * dt
        b.x = b.x + b.vx * dt
        b.y = b.y + b.vy * dt
        for _, o in ipairs(current.solids) do
            local dx, dy = b.x - o.x, b.y - o.y
            local px = (b.w + o.w) / 2 - math.abs(dx)
            local py = (b.h + o.h) / 2 - math.abs(dy)
            if px > 0 and py > 0 then
                if px < py then
                    b.x = b.x + (dx < 0 and -px or px)
                    b.vx = -b.vx * b.bounce
                else
                    b.y = b.y + (dy < 0 and -py or py)
                    b.vy = -b.vy * b.bounce
                    if math.abs(b.vy) < 20 then
                        b.vy = 0
                    end
                end
            end
        end
    end
end

function ic._scene_draw(dt)
    fps_time = fps_time + (dt or 0)
    fps_frames = fps_frames + 1
    if fps_time >= 0.5 then
        fps = math.floor(fps_frames / fps_time + 0.5)
        fps_time, fps_frames = 0, 0
    end
    if not current then
        return
    end
    for _, n in ipairs(current.nodes) do
        if not skip[n.kind] and shown(n) then
            local x, y = world(n)
            if n.kind == "FPS" then
                ic.text("FPS: " .. fps, x, y, n.size, n.r, n.g, n.b, n.a)
            else
                if n.tex ~= "" then
                    ic.image(n.tex, x, y, n.w, n.h, n.rot, 1, 1, 1, n.a)
                elseif n.kind ~= "Label" then
                    ic.rect(x, y, n.w, n.h, n.r, n.g, n.b, n.a, n.rot)
                end
                if n.text ~= "" then
                    if n.kind == "Label" then
                        ic.text(n.text, x, y, n.size, n.r, n.g, n.b, n.a)
                    else
                        ic.text(n.text, x, y, n.size, 1, 1, 1, 1)
                    end
                end
            end
        end
    end
end
