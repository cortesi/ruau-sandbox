eguidev.fixture("sandbox.default")
local vp = eguidev.root

local source = table.concat({
    "--!strict",
    "function add(left: number, right: number): number",
    "    return left + right",
    "end",
    "",
    "print(\"value = \" .. add(10, 5))",
}, "\n")
local editor = eguidev.widget("editor.source")
editor:set_value(source)

local edited = eguidev.widget("editor.source"):expect({ value = source })
assert(edited ~= nil)
assert(edited.value_text == source, "editor should accept scripted source replacement")
assert(
    string.find(edited.value_text, "\nfunction add", 1, true) ~= nil,
    "editor should preserve top-level Luau function declarations"
)

eguidev.widget("action.run"):click()
local output = eguidev.widget("output.text"):expect({ value_text_contains = "value = 15" })
assert(output ~= nil)
assert(output ~= nil, "run should render output from edited source")

local broken = "--!strict\nprint(missing)"
editor = eguidev.widget("editor.source")
editor:set_value(broken)

local diagnostic = eguidev.widget("diagnostics.0.message"):expect({ value_text_contains = "missing" })
assert(diagnostic ~= nil)
assert(diagnostic ~= nil, "diagnostic should name the missing value")

local severity = eguidev.widget("diagnostics.0.severity"):state()
assert(severity ~= nil)
assert(severity.value_text == "error", "diagnostic card should show its severity")
local location = eguidev.widget("diagnostics.0.location"):state()
assert(location ~= nil)
assert(
    string.find(location.value_text, "line 2", 1, true) ~= nil,
    "diagnostic card should show the source line"
)

return {
    output = output.value_text,
    diagnostic = diagnostic.value_text,
    severity = severity.value_text,
}
