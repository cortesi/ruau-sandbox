fixture("sandbox.default")
local vp = root()

local layout = vp:check_layout()
assert(type(layout) == "table", "viewport layout check should return an array")
assert(#layout == 0, "default viewport should not report layout issues")

local editor_issues = vp:widget_get("editor.source"):check_layout()
assert(#editor_issues == 0, "editor subtree should be layout-clean")

vp:set_inner_size({ x = 760, y = 520 })
vp:wait_for_capture()
layout = vp:check_layout()
assert(#layout == 0, "narrower viewport should remain layout-clean")

return {
    issues = #layout,
}
