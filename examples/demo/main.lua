-- IC demo: 10 000 летающих квадратов на Lua. WASD — двигать, Esc — выход.
ic.title = "IC Demo (Lua)"

local COUNT = 10000
local px, py, vx, vy, cr, cg, cb = {}, {}, {}, {}, {}, {}, {}
local player = { x = 0, y = 0 }
local fps_timer, fps_frames = 0, 0

function ic.load()
    math.randomseed(42)
    for i = 1, COUNT do
        px[i] = math.random(-600, 600)
        py[i] = math.random(-340, 340)
        vx[i] = math.random(-120, 120)
        vy[i] = math.random(-120, 120)
        cr[i], cg[i], cb[i] = math.random(), math.random(), math.random()
    end
    print("IC demo: " .. COUNT .. " спрайтов. WASD — двигать, Esc — выход.")
end

function ic.fixed_update(dt)
    local w, h = ic.screen()
    local hh = 360
    local hw = hh * w / h
    for i = 1, COUNT do
        local x = px[i] + vx[i] * dt
        local y = py[i] + vy[i] * dt
        if x > hw or x < -hw then vx[i] = -vx[i]; x = math.max(-hw, math.min(hw, x)) end
        if y > hh or y < -hh then vy[i] = -vy[i]; y = math.max(-hh, math.min(hh, y)) end
        px[i], py[i] = x, y
    end
end

function ic.update(dt)
    if ic.key_pressed("escape") then ic.quit() end

    local speed = 400 * dt
    if ic.key_down("w") or ic.key_down("up") then player.y = player.y + speed end
    if ic.key_down("s") or ic.key_down("down") then player.y = player.y - speed end
    if ic.key_down("d") or ic.key_down("right") then player.x = player.x + speed end
    if ic.key_down("a") or ic.key_down("left") then player.x = player.x - speed end

    local rect, t = ic.rect, ic.time()
    for i = 1, COUNT do
        rect(px[i], py[i], 6, 6, cr[i], cg[i], cb[i], 0.85, t)
    end
    rect(player.x, player.y, 40, 40, 1, 1, 1, 1, t * 2)

    fps_timer = fps_timer + dt
    fps_frames = fps_frames + 1
    if fps_timer >= 1 then
        print("FPS: " .. fps_frames)
        fps_timer, fps_frames = 0, 0
    end
end
