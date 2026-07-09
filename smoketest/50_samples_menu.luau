fixture("sandbox.default")
local vp = root()

-- The samples menu lists every registered sample.
vp:widget_get("samples.menu"):click()
expect("samples.item.0", { visible = true })
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
expect_absent("samples.item.0")

local editor = expect("editor.source", { value_text_contains = "width" })
local diagnostic = expect("diagnostics.0.message", { value_text_contains = "height" })
local second_severity = expect("diagnostics.1.severity", { value_text_contains = "error" })

-- Loading a sample must isolate it from previous runs.
local output = vp:widget_get("output.text"):state()
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
