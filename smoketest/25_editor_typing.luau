fixture("sandbox.default")
local vp = root()

local typed_source = table.concat({
    "function typed_value(): number",
    "    return 9",
    "end",
    "",
    "print(\"typed = \" .. typed_value())",
}, "\n")
local editor = vp:widget_get("editor.source")
editor:type_text(typed_source, { clear = true })

local edited = expect("editor.source", { value = typed_source })
assert(edited.value_text == typed_source, "editor should accept typed replacement text")
assert(
    string.find(edited.value_text, "function typed_value", 1, true) ~= nil,
    "typed editor content should preserve top-level Luau function declarations"
)

vp:widget_get("action.run"):click()
local output = expect("output.text", { value_text_contains = "typed = 9" })
assert(output ~= nil, "run should use the source entered through type_text")

return {
    output = output.value_text,
}
