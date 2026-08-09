eguidev.fixture("sandbox.default")
local vp = eguidev.root

local function run_until_output(predicate, description)
    local last_output = ""
    for _ = 1, 5 do
        eguidev.widget("action.run"):click()
        eguidev.wait_frames(4)
        local output = eguidev.widget("output.text"):state()
        assert(output ~= nil)
        last_output = output.value_text
        if predicate(output) then
            return output
        end
    end
    error(description .. "\nlast output:\n" .. last_output)
end

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

vp:resize({ inner_size = { x = 900, y = 620 } })
vp:wait_capture()

run_until_output(function(widget)
    return string.find(widget.value_text, "Run 1", 1, true) ~= nil
        and string.find(widget.value_text, "area = 42", 1, true) ~= nil
end, "first run should render default program output")
local editor = eguidev.widget("editor.source")
local second_source = table.concat({
    "function value(): number",
    "    return 15",
    "end",
    "",
    "print(\"value = \" .. value())",
}, "\n")
editor:set_value(second_source)
eguidev.widget("editor.source"):wait({ value = second_source })

local output = run_until_output(function(widget)
    return string.find(widget.value_text, "Run 1", 1, true) ~= nil
        and string.find(widget.value_text, "area = 42", 1, true) ~= nil
        and string.find(widget.value_text, "Run 2", 1, true) ~= nil
        and string.find(widget.value_text, "value = 15", 1, true) ~= nil
end, "second run should append edited program output")
assert(output ~= nil, "output should retain a chronological record of script runs")

local panel = eguidev.widget("output.panel"):state()
assert(panel ~= nil)
local history = eguidev.widget("output.history"):state()
assert(history ~= nil)
local viewport = vp:state()
assert(viewport ~= nil)
assert(history.role == "scroll_area", "output history should be a scroll area")
assert(history.scroll_state ~= nil, "output history should expose scroll metadata")
assert(history.rect.min.x <= panel.rect.min.x + 16, "output history should start near the panel left edge")
assert(history.rect.max.x >= panel.rect.max.x - 16, "output history should fill the panel width")
assert(history.rect.max.y >= panel.rect.max.y - 16, "output history should fill the panel height")
assert(panel.rect.max.y >= viewport.inner_size.y - 20, "output panel should reach the bottom of the viewport")
assert(output.rect.min.x <= history.rect.min.x + 16, "output text should be left aligned")

local editor_scroll = eguidev.widget("editor.scroll"):state()
assert(editor_scroll ~= nil)
local editor_state = eguidev.widget("editor.source"):state()
assert(editor_state ~= nil)
assert(editor_scroll.role == "scroll_area", "editor should be hosted by a scroll area")
assert(editor_scroll.rect.max.y >= viewport.inner_size.y - 20, "editor scroll should reach the bottom of the viewport")
assert(editor_state.rect.max.y >= editor_scroll.rect.max.y - 4, "text code area should fill the editor scroll height")

local divider = eguidev.widget("layout.divider")
local divider_before = divider:state()
assert(divider_before ~= nil)
assert(divider_before.role == "separator", "pane divider should be a separator")
divider:drag_to({
    x = divider_before.rect.max.x + 80,
    y = (divider_before.rect.min.y + divider_before.rect.max.y) * 0.5,
}, {
    settle = false,
})
eguidev.wait_frames(4)
local divider_after = eguidev.widget("layout.divider"):state()
assert(divider_after ~= nil)
assert(
    divider_after.rect.min.x > divider_before.rect.min.x + 20,
    "dragging the divider should resize the panes: before="
        .. tostring(divider_before.rect.min.x)
        .. " after="
        .. tostring(divider_after.rect.min.x)
)

local lines: { string } = {}
lines[#lines + 1] = "function line_text(index)"
lines[#lines + 1] = "    return \"line \" .. index"
lines[#lines + 1] = "end"
for index = 1, 32 do
    lines[#lines + 1] = "print(line_text(" .. tostring(index) .. "))"
end
editor:set_value(table.concat(lines, "\n"))
eguidev.widget("action.run"):click()

history = eguidev.widget("output.history"):wait(function(widget)
    if widget == nil then
        return false
    end
    local scroll = widget.scroll_state
    return scroll ~= nil and scroll.content_size.y > scroll.viewport_size.y
end)
assert(history ~= nil)
assert(history.scroll_state ~= nil, "long output should expose scroll metadata")

local layout = vp:layout_issues()
assert(#layout == 0, "output history should remain layout-clean at the default test size:\n" .. layout_summary(layout))

vp:resize({ inner_size = { x = 700, y = 520 } })
vp:wait_capture()
layout = vp:layout_issues()
panel = eguidev.widget("output.panel"):state()
assert(panel ~= nil)
history = eguidev.widget("output.history"):state()
assert(history ~= nil)
viewport = vp:state()
assert(viewport ~= nil)
editor_scroll = eguidev.widget("editor.scroll"):state()
assert(editor_scroll ~= nil)
editor_state = eguidev.widget("editor.source"):state()
assert(editor_state ~= nil)
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

local final_scroll = history.scroll_state
assert(final_scroll ~= nil)

return {
    output_len = string.len(output.value_text),
    scroll_content_height = final_scroll.content_size.y,
    scroll_viewport_height = final_scroll.viewport_size.y,
}
