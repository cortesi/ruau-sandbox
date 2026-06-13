fixture("sandbox.default")
local vp = root()

local source = "fn main() {\n    let x: i64 = 10;\n    let y = x + 5;\n    println!(\"value = {}\", y);\n}"
local editor = vp:widget_get("editor.source")
editor:set_value(source)

local edited = vp:wait_for_widget("editor.source", function(widget)
    return widget.value_text == source
end)
assert(edited.value_text == source, "editor should accept scripted source replacement")
assert(
    string.find(edited.value_text, "\n    let y", 1, true) ~= nil,
    "editor should preserve indentation in source text"
)

vp:widget_get("action.run"):click()
local output = vp:wait_for_widget("output.text", function(widget)
    return string.find(widget.value_text, "value = 15", 1, true) ~= nil
end)
assert(output ~= nil, "run should render output from edited source")

local broken = "fn main() {\n    println!(missing);\n}"
editor = vp:widget_get("editor.source")
editor:set_value(broken)
local status = vp:wait_for_widget("status.check", function(widget)
    return widget.value_text == "errors"
end)
assert(status ~= nil, "status should switch to errors for invalid source")

local diagnostic = vp:wait_for_widget("diagnostics.0.message", function(widget)
    return string.find(widget.value_text, "missing", 1, true) ~= nil
end)
assert(diagnostic ~= nil, "diagnostic should name the missing value")

return {
    output = output.value_text,
    diagnostic = diagnostic.value_text,
}
