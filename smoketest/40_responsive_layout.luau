fixture("sandbox.default")
local vp = root()

local sizes = {
    { x = 900, y = 620 },
    { x = 700, y = 520 },
}

for _, size in sizes do
    vp:set_inner_size(size)
    vp:wait_for_capture()
    local layout = vp:check_layout()
    assert(
        #layout == 0,
        "viewport " .. tostring(size.x) .. "x" .. tostring(size.y) .. " should be layout-clean"
    )
end

return {
    checked_sizes = #sizes,
}
