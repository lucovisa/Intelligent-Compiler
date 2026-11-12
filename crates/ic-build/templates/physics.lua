local physics = {}
physics.__index = physics

function physics.new(opts)
    opts = opts or {}
    return setmetatable({
        gravity = opts.gravity or -900,
        bounds = opts.bounds,
        bodies = {},
    }, physics)
end

function physics:add(x, y, w, h, opts)
    opts = opts or {}
    local body = {
        x = x,
        y = y,
        w = w,
        h = h,
        vx = opts.vx or 0,
        vy = opts.vy or 0,
        static = opts.static or false,
        bounce = opts.bounce or 0.3,
    }
    self.bodies[#self.bodies + 1] = body
    return body
end

local function overlap(a, b)
    local dx = a.x - b.x
    local dy = a.y - b.y
    local px = (a.w + b.w) / 2 - math.abs(dx)
    local py = (a.h + b.h) / 2 - math.abs(dy)
    if px <= 0 or py <= 0 then
        return nil
    end
    return dx, dy, px, py
end

local function resolve(a, b)
    local dx, dy, px, py = overlap(a, b)
    if not dx then
        return
    end
    local wa = a.static and 0 or (b.static and 1 or 0.5)
    local wb = b.static and 0 or (a.static and 1 or 0.5)
    if px < py then
        local s = dx >= 0 and 1 or -1
        a.x = a.x + s * px * wa
        b.x = b.x - s * px * wb
        if not a.static then a.vx = -a.vx * a.bounce end
        if not b.static then b.vx = -b.vx * b.bounce end
    else
        local s = dy >= 0 and 1 or -1
        a.y = a.y + s * py * wa
        b.y = b.y - s * py * wb
        if not a.static then a.vy = -a.vy * a.bounce end
        if not b.static then b.vy = -b.vy * b.bounce end
    end
end

function physics:step(dt)
    local bodies = self.bodies
    for _, b in ipairs(bodies) do
        if not b.static then
            b.vy = b.vy + self.gravity * dt
            b.x = b.x + b.vx * dt
            b.y = b.y + b.vy * dt
        end
    end
    for i = 1, #bodies do
        for j = i + 1, #bodies do
            local a, b = bodies[i], bodies[j]
            if not (a.static and b.static) then
                resolve(a, b)
            end
        end
    end
    local bd = self.bounds
    if bd then
        for _, b in ipairs(bodies) do
            if not b.static then
                local hw, hh = b.w / 2, b.h / 2
                if b.x - hw < bd.left then b.x = bd.left + hw; b.vx = -b.vx * b.bounce end
                if b.x + hw > bd.right then b.x = bd.right - hw; b.vx = -b.vx * b.bounce end
                if b.y - hh < bd.bottom then b.y = bd.bottom + hh; b.vy = -b.vy * b.bounce end
                if b.y + hh > bd.top then b.y = bd.top - hh; b.vy = -b.vy * b.bounce end
            end
        end
    end
end

function physics:draw()
    for _, b in ipairs(self.bodies) do
        if b.static then
            ic.rect(b.x, b.y, b.w, b.h, 0.4, 0.45, 0.55)
        else
            ic.rect(b.x, b.y, b.w, b.h, 0.95, 0.6, 0.2)
        end
    end
end

return physics
