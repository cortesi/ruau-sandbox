fixture("sandbox.default")
local vp = root()

vp:widget_get("action.run"):click()
local output = vp:wait_for_widget("output.text", function(widget)
    return string.find(widget.value_text, "area = 42", 1, true) ~= nil
end)
assert(output ~= nil, "run should render default program output")

return {
    output = output.value_text,
}
