fixture("sandbox.default")
local vp = root()

-- The keyboard shortcut runs the program.
vp:widget_get("editor.source"):click()
vp:key("cmd-enter")
local output = vp:wait_for_widget("output.text", function(widget)
    return string.find(widget.value_text, "area = 42", 1, true) ~= nil
end)
assert(
    string.find(output.value_text, "Run 1 - ok", 1, true) ~= nil,
    "run history should record the shortcut-triggered run"
)

-- Clear discards the run history and resets the placeholder.
vp:widget_get("output.clear"):click()
local cleared = vp:wait_for_widget("output.text", function(widget)
    return string.find(widget.value_text, "Run the program", 1, true) ~= nil
end)
assert(cleared ~= nil, "clear should reset the output panel")

-- Run numbering restarts after a clear.
vp:widget_get("action.run"):click()
local rerun = vp:wait_for_widget("output.text", function(widget)
    return string.find(widget.value_text, "Run 1 - ok", 1, true) ~= nil
end)

return {
    first = output.value_text,
    cleared = cleared.value_text,
    rerun = rerun.value_text,
}
