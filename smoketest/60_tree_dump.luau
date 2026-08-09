local function find_dump_widget(node: any, id: string): any?
    if node.id == id then
        return node
    end
    for _, child in ipairs(node.children or {}) do
        local found = find_dump_widget(child, id)
        if found ~= nil then
            return found
        end
    end
    return nil
end

eguidev.fixture("sandbox.default")
local vp = eguidev.root
eguidev.widget("editor.source"):expect({ visible = true })
eguidev.widget("action.run"):click()
eguidev.widget("output.text"):expect({ value_text_contains = "area = 42" })
eguidev.wait(function()
    return eguidev.diagnostic("sandbox.state").history.runs == 1
end)

local sandbox_state = eguidev.diagnostic("sandbox.state") :: any
assert(sandbox_state.check.ok == true, "sandbox diagnostic should report a clean check")
assert(sandbox_state.run.ok == true, "sandbox diagnostic should report a clean run")
assert(sandbox_state.run.has_output == true, "sandbox diagnostic should report run output")
assert(sandbox_state.history.runs == 1, "sandbox diagnostic should count run history")

local left_tree = eguidev.dump({ root = eguidev.widget("layout.left_pane"), fields = "full" }) :: any
assert(#left_tree.viewports == 1, "left-pane dump should include one viewport")
assert(#left_tree.viewports[1].widgets == 1, "left-pane dump should be rooted at layout.left_pane")

local left_pane = left_tree.viewports[1].widgets[1]
assert(left_pane.id == "layout.left_pane", "left-pane dump root should match the requested root")
local editor = find_dump_widget(left_pane, "editor.source")
assert(editor ~= nil, "left-pane dump should include the source editor")
assert(editor.role == "text_edit", "source editor role should be preserved in the dump")
assert(
    string.find(editor.value_text, "function area", 1, true) ~= nil,
    "source editor text should be preserved in the dump"
)

local right_tree = eguidev.dump({ root = eguidev.widget("layout.right_pane"), fields = "full" }) :: any
assert(#right_tree.viewports == 1, "right-pane dump should include one viewport")
assert(#right_tree.viewports[1].widgets == 1, "right-pane dump should be rooted at layout.right_pane")

local right_pane = right_tree.viewports[1].widgets[1]
assert(right_pane.id == "layout.right_pane", "right-pane dump root should match the requested root")
local history = find_dump_widget(right_pane, "output.history")
assert(history ~= nil, "right-pane dump should include output history")
assert(history.scroll_state ~= nil, "full dump should include output history scroll metadata")

local output = find_dump_widget(right_pane, "output.text")
assert(output ~= nil, "right-pane dump should include output text")
assert(string.find(output.value_text, "area = 42", 1, true) ~= nil, "dump should include run output")

local text = eguidev.dump_text({ root = eguidev.widget("layout.right_pane"), include_invisible = false })
assert(string.find(text, "output.text", 1, true) ~= nil, "text dump should include output text")

return {
    editor_len = string.len(editor.value_text),
    output_len = string.len(output.value_text),
    history_runs = sandbox_state.history.runs,
}
