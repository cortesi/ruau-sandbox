eguidev.fixture("sandbox.default")
local viewport = eguidev.root

viewport:resize({ inner_size = { x = 900, y = 620 } })
viewport:wait_capture()

eguidev.widget("action.run"):click()
local output = eguidev.widget("output.text"):expect({ value_text_contains = "area = 42" })
assert(output ~= nil, "default example output should be visible")

local layout_issues = viewport:layout_issues()
assert(#layout_issues == 0, "README screenshot viewport should be layout-clean")

return {
    output = output.value_text,
    screenshot = viewport:screenshot(),
}
