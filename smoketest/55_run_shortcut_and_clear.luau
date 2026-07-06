fixture("sandbox.default")
local vp = root()

-- The keyboard shortcut runs the program.
vp:widget_get("editor.source"):click()
vp:key("cmd-enter")
local output = expect("output.text", { value_text_contains = "area = 42" })
assert(
    string.find(output.value_text, "Run 1 - ok", 1, true) ~= nil,
    "run history should record the shortcut-triggered run"
)

-- Clear discards the run history and resets the placeholder.
vp:widget_get("output.clear"):click()
local cleared = expect("output.text", { value_text_contains = "Run the program" })
assert(cleared ~= nil, "clear should reset the output panel")

-- Run numbering restarts after a clear.
vp:widget_get("action.run"):click()
local rerun = expect("output.text", { value_text_contains = "Run 1 - ok" })

return {
    first = output.value_text,
    cleared = cleared.value_text,
    rerun = rerun.value_text,
}
