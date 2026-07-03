fixture("sandbox.default")
local vp = root()

-- The samples menu lists every registered sample.
vp:widget_get("samples.menu"):click()
vp:wait_for_widget_visible("samples.item.0")
local items = vp:widget_list({ id_prefix = "samples.item." })
assert(#items >= 6, "samples menu should list all samples, found " .. tostring(#items))

-- Selecting the broken sample loads it, closes the menu, and surfaces errors.
local broken_index = nil
for _, item in ipairs(items) do
    if item:state().label == "Broken program" then
        broken_index = item.id
    end
end
assert(broken_index ~= nil, "samples menu should contain the broken sample")

vp:widget_get(broken_index):click()
vp:wait_for_widget_absent("samples.item.0")

local editor = vp:wait_for_widget("editor.source", function(widget)
    return string.find(widget.value_text, "width", 1, true) ~= nil
end)
local status = vp:wait_for_widget("status.check", function(widget)
    return widget.value_text == "2 errors"
end)
local diagnostic = vp:wait_for_widget("diagnostics.0.message", function(widget)
    return string.find(widget.value_text, "height", 1, true) ~= nil
end)

-- Loading a sample must isolate it from previous runs.
local output = vp:widget_get("output.text"):state()
assert(
    string.find(output.value_text, "Run the program", 1, true) ~= nil,
    "loading a sample should clear the run history"
)

return {
    sample_count = #items,
    status = status.value_text,
    diagnostic = diagnostic.value_text,
    editor_len = string.len(editor.value_text),
}
