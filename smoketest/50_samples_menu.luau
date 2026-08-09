eguidev.fixture("sandbox.default")
local vp = eguidev.root

-- The samples menu lists every registered sample.
eguidev.widget("samples.menu"):click()
eguidev.widget("samples.item.0"):expect({ visible = true })
local items = vp:widgets({ id_prefix = "samples.item." })
assert(#items >= 6, "samples menu should list all samples, found " .. tostring(#items))

-- Selecting the broken sample loads it, closes the menu, and surfaces errors.
local broken_index = nil
for _, item in ipairs(items) do
    local state = item:state()
    if state ~= nil and state.label == "Broken program" then
        broken_index = item.id
    end
end
assert(broken_index ~= nil, "samples menu should contain the broken sample")

eguidev.widget(broken_index):click()
eguidev.widget("samples.item.0"):wait({ present = false })

local editor = eguidev.widget("editor.source"):expect({ value_text_contains = "width" })
assert(editor ~= nil)
local diagnostic = eguidev.widget("diagnostics.0.message"):expect({ value_text_contains = "height" })
assert(diagnostic ~= nil)
local second_severity = eguidev.widget("diagnostics.1.severity"):expect({ value_text_contains = "error" })
assert(second_severity ~= nil)

-- Loading a sample must isolate it from previous runs.
local output = eguidev.widget("output.text"):state()
assert(output ~= nil)
assert(
    string.find(output.value_text, "Run the program", 1, true) ~= nil,
    "loading a sample should clear the run history"
)

return {
    sample_count = #items,
    diagnostic = diagnostic.value_text,
    second_severity = second_severity.value_text,
    editor_len = string.len(editor.value_text),
}
