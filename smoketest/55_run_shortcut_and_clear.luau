eguidev.fixture("sandbox.default")
local vp = eguidev.root

-- The keyboard shortcut runs the program.
eguidev.widget("editor.source"):click()
vp:key("cmd-enter")
local output = eguidev.widget("output.text"):expect({ value_text_contains = "area = 42" })
assert(output ~= nil)
assert(
    string.find(output.value_text, "Run 1", 1, true) ~= nil,
    "run history should record the shortcut-triggered run"
)
assert(string.find(output.value_text, " - ok", 1, true) == nil, "run history should not render ok text")

-- Clear discards the run history and resets the placeholder.
eguidev.widget("output.clear"):click()
local cleared = eguidev.widget("output.text"):expect({ value_text_contains = "Run the program" })
assert(cleared ~= nil)
assert(cleared ~= nil, "clear should reset the output panel")

-- Run numbering restarts after a clear.
eguidev.widget("action.run"):click()
local rerun = eguidev.widget("output.text"):expect({ value_text_contains = "Run 1" })
assert(rerun ~= nil)
assert(string.find(rerun.value_text, " - ok", 1, true) == nil, "rerun history should not render ok text")

return {
    first = output.value_text,
    cleared = cleared.value_text,
    rerun = rerun.value_text,
}
