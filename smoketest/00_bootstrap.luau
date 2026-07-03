fixture("sandbox.default")
local vp = root()

local fixtures_available = fixtures()
assert(#fixtures_available >= 2, "sandbox fixtures should be registered")

local editor = vp:widget_get("editor.source")
local editor_state = editor:state()
assert(editor_state.role == "text_edit", "source editor should be a text edit")
assert(editor_state.multiline == true, "source editor should be multiline")
assert(string.find(editor_state.value_text, "print(", 1, true) ~= nil, "default source should be visible")
assert(
    string.find(editor_state.value_text, "function area", 1, true) ~= nil,
    "default source should include a function declaration"
)

local status = vp:widget_get("status.check"):state()
assert(status.value_text == "checked", "default source should start checked")

return {
    fixture_count = #fixtures_available,
    editor_len = string.len(editor_state.value_text),
}
