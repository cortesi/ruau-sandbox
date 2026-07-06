fixture("sandbox.default")
local vp = root()

vp:widget_get("action.run"):click()
local output = expect("output.text", { value_text_contains = "area = 42" })
assert(output ~= nil, "run should render default program output")

return {
    output = output.value_text,
}
