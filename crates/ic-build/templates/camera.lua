local camera = { x = 0, y = 0, height = 720, shake_time = 0, shake_power = 0 }

function camera.follow(tx, ty, dt, speed)
    local k = math.min(1, (speed or 6) * dt)
    camera.x = camera.x + (tx - camera.x) * k
    camera.y = camera.y + (ty - camera.y) * k
end

function camera.shake(power, time)
    camera.shake_power = power
    camera.shake_time = time
end

function camera.apply(dt)
    local ox, oy = 0, 0
    if camera.shake_time > 0 then
        camera.shake_time = camera.shake_time - dt
        ox = (math.random() - 0.5) * camera.shake_power
        oy = (math.random() - 0.5) * camera.shake_power
    end
    ic.camera(camera.x + ox, camera.y + oy, camera.height)
end

return camera
