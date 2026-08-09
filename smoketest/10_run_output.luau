eguidev.fixture("sandbox.default")
local vp = eguidev.root

eguidev.widget("action.run"):click()
local output = eguidev.widget("output.text"):expect({ value_text_contains = "area = 42" })
assert(output ~= nil)
assert(output ~= nil, "run should render default program output")

return {
    output = output.value_text,
}
