fixture("sandbox.default")
local vp = root()

local function layout_summary(layout)
    local issues = {}
    for _, issue in ipairs(layout) do
        issues[#issues + 1] = issue.kind
            .. " ["
            .. table.concat(issue.widgets, ", ")
            .. "]: "
            .. issue.message
            .. " rect="
            .. tostring(issue.rect.min.x)
            .. ","
            .. tostring(issue.rect.min.y)
            .. "-"
            .. tostring(issue.rect.max.x)
            .. ","
            .. tostring(issue.rect.max.y)
    end
    return table.concat(issues, "\n")
end

vp:set_inner_size({ x = 900, y = 620 })
vp:wait_for_capture()

vp:widget_get("action.run"):click()
local editor = vp:widget_get("editor.source")
local second_source = "fn main() {\n    let value = 15;\n    println!(\"value = {}\", value);\n}"
editor:set_value(second_source)
vp:widget_get("action.run"):click()

local output = vp:wait_for_widget("output.text", function(widget)
    return string.find(widget.value_text, "Run 1", 1, true) ~= nil
        and string.find(widget.value_text, "area = 42", 1, true) ~= nil
        and string.find(widget.value_text, "Run 2", 1, true) ~= nil
        and string.find(widget.value_text, "value = 15", 1, true) ~= nil
end)
assert(output ~= nil, "output should retain a chronological record of script runs")

local panel = vp:widget_get("output.panel"):state()
local history = vp:widget_get("output.history"):state()
local viewport = vp:state()
assert(history.role == "scroll_area", "output history should be a scroll area")
assert(history.scroll_state ~= nil, "output history should expose scroll metadata")
assert(history.rect.min.x <= panel.rect.min.x + 16, "output history should start near the panel left edge")
assert(history.rect.max.x >= panel.rect.max.x - 16, "output history should fill the panel width")
assert(history.rect.max.y >= panel.rect.max.y - 16, "output history should fill the panel height")
assert(panel.rect.max.y >= viewport.inner_size.y - 20, "output panel should reach the bottom of the viewport")
assert(output.rect.min.x <= history.rect.min.x + 16, "output text should be left aligned")

local editor_scroll = vp:widget_get("editor.scroll"):state()
local editor_state = vp:widget_get("editor.source"):state()
assert(editor_scroll.role == "scroll_area", "editor should be hosted by a scroll area")
assert(editor_scroll.rect.max.y >= viewport.inner_size.y - 20, "editor scroll should reach the bottom of the viewport")
assert(editor_state.rect.max.y >= editor_scroll.rect.max.y - 4, "text code area should fill the editor scroll height")

local divider = vp:widget_get("layout.divider")
local divider_before = divider:state()
assert(divider_before.role == "separator", "pane divider should be a separator")
divider:drag({
    x = divider_before.rect.max.x + 80,
    y = (divider_before.rect.min.y + divider_before.rect.max.y) * 0.5,
}, {
    settle = false,
})
wait_for_frames(4)
local divider_after = vp:widget_get("layout.divider"):state()
assert(
    divider_after.rect.min.x > divider_before.rect.min.x + 20,
    "dragging the divider should resize the panes: before="
        .. tostring(divider_before.rect.min.x)
        .. " after="
        .. tostring(divider_after.rect.min.x)
)

local lines = {}
for index = 1, 32 do
    lines[#lines + 1] = "    println!(\"line " .. tostring(index) .. "\");"
end
editor:set_value("fn main() {\n" .. table.concat(lines, "\n") .. "\n}")
vp:widget_get("action.run"):click()

history = vp:wait_for_widget("output.history", function(widget)
    local scroll = widget.scroll_state
    return scroll ~= nil and scroll.content_size.y > scroll.viewport_size.y
end)
assert(history.scroll_state ~= nil, "long output should expose scroll metadata")

local layout = vp:check_layout()
assert(#layout == 0, "output history should remain layout-clean at the default test size:\n" .. layout_summary(layout))

vp:set_inner_size({ x = 700, y = 520 })
vp:wait_for_capture()
layout = vp:check_layout()
panel = vp:widget_get("output.panel"):state()
history = vp:widget_get("output.history"):state()
viewport = vp:state()
editor_scroll = vp:widget_get("editor.scroll"):state()
editor_state = vp:widget_get("editor.source"):state()
assert(history.rect.max.y >= panel.rect.max.y - 16, "narrow output history should fill the panel height")
assert(panel.rect.max.y >= viewport.inner_size.y - 20, "narrow output panel should reach the viewport bottom")
assert(editor_scroll.rect.max.y >= viewport.inner_size.y - 20, "narrow editor scroll should reach the viewport bottom")
assert(editor_state.rect.max.y >= editor_scroll.rect.max.y - 4, "narrow text code area should fill the editor scroll height")
assert(
    #layout == 0,
    "output history should remain layout-clean in a narrower viewport:\n"
        .. layout_summary(layout)
        .. "\neditor rect="
        .. tostring(editor_state.rect.min.x)
        .. ","
        .. tostring(editor_state.rect.min.y)
        .. "-"
        .. tostring(editor_state.rect.max.x)
        .. ","
        .. tostring(editor_state.rect.max.y)
)

return {
    output_len = string.len(output.value_text),
    scroll_content_height = history.scroll_state.content_size.y,
    scroll_viewport_height = history.scroll_state.viewport_size.y,
}
