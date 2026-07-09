fixture("sandbox.default")
local vp = root()

local fixtures_available = fixtures()
assert(#fixtures_available >= 2, "sandbox fixtures should be registered")

local ruau_link = vp:widget_get("app.title.ruau"):state()
assert(ruau_link.role == "link", "Ruau title should be a link")
assert(
    ruau_link.value_text == "https://github.com/cortesi/ruau",
    "Ruau title link should point at the Ruau repository"
)

local editor = vp:widget_get("editor.source")
local editor_state = editor:state()
assert(editor_state.role == "text_edit", "source editor should be a text edit")
assert(editor_state.multiline == true, "source editor should be multiline")
assert(string.find(editor_state.value_text, "print(", 1, true) ~= nil, "default source should be visible")
assert(
    string.find(editor_state.value_text, "function area", 1, true) ~= nil,
    "default source should include a function declaration"
)

local diagnostics_empty = vp:widget_get("diagnostics.empty"):state()
assert(diagnostics_empty.value_text == "No errors", "default source should start without diagnostics")

return {
    fixture_count = #fixtures_available,
    editor_len = string.len(editor_state.value_text),
}
