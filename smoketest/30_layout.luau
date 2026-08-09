eguidev.fixture("sandbox.default")
local vp = eguidev.root

local layout = vp:layout_issues()
assert(type(layout) == "table", "viewport layout check should return an array")
assert(#layout == 0, "default viewport should not report layout issues")

local editor_issues = eguidev.widget("editor.source"):layout_issues()
assert(#editor_issues == 0, "editor subtree should be layout-clean")

vp:resize({ inner_size = { x = 760, y = 520 } })
vp:wait_capture()
layout = vp:layout_issues()
assert(#layout == 0, "narrower viewport should remain layout-clean")

return {
    issues = #layout,
}
