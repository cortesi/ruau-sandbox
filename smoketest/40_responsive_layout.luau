eguidev.fixture("sandbox.default")
local vp = eguidev.root

local sizes = {
    { x = 900, y = 620 },
    { x = 700, y = 520 },
}

for _, size in sizes do
    vp:resize({ inner_size = size })
    vp:wait_capture()
    local layout = vp:layout_issues()
    assert(
        #layout == 0,
        "viewport " .. tostring(size.x) .. "x" .. tostring(size.y) .. " should be layout-clean"
    )
end

return {
    checked_sizes = #sizes,
}
