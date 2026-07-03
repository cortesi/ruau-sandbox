fixture("sandbox.default")
local vp = root()

local source = table.concat({
    "--!strict",
    "function add(left: number, right: number): number",
    "    return left + right",
    "end",
    "",
    "print(\"value = \" .. add(10, 5))",
}, "\n")
local editor = vp:widget_get("editor.source")
editor:set_value(source)

local edited = vp:wait_for_widget("editor.source", function(widget)
    return widget.value_text == source
end)
assert(edited.value_text == source, "editor should accept scripted source replacement")
assert(
    string.find(edited.value_text, "\nfunction add", 1, true) ~= nil,
    "editor should preserve top-level Luau function declarations"
)

vp:widget_get("action.run"):click()
local output = vp:wait_for_widget("output.text", function(widget)
    return string.find(widget.value_text, "value = 15", 1, true) ~= nil
end)
assert(output ~= nil, "run should render output from edited source")

local broken = "--!strict\nprint(missing)"
editor = vp:widget_get("editor.source")
editor:set_value(broken)
local status = vp:wait_for_widget("status.check", function(widget)
    return string.find(widget.value_text, "error", 1, true) ~= nil
end)
assert(status.value_text == "1 error", "status should count errors for invalid source")

local diagnostic = vp:wait_for_widget("diagnostics.0.message", function(widget)
    return string.find(widget.value_text, "missing", 1, true) ~= nil
end)
assert(diagnostic ~= nil, "diagnostic should name the missing value")

local severity = vp:widget_get("diagnostics.0.severity"):state()
assert(severity.value_text == "error", "diagnostic card should show its severity")
local location = vp:widget_get("diagnostics.0.location"):state()
assert(
    string.find(location.value_text, "line 2", 1, true) ~= nil,
    "diagnostic card should show the source line"
)

return {
    output = output.value_text,
    diagnostic = diagnostic.value_text,
    status = status.value_text,
}
