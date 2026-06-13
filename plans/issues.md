# Sandbox Issues

1. [x] `edev fixtures` times out during MCP initialization for the sandbox app.

   Evidence: `edev fixtures --verbose` spawned `cargo run --quiet -p oxau-sandbox-gui --features
   devtools -- --dev-mcp`, then failed with `app init failed: Request timed out after 120000ms:
   req-1`. The same `edev fixtures --quiet` launcher flow succeeds in `~/git/public/spacecurve`,
   so the current evidence points at this app's eguidev startup wiring rather than a global eguidev
   failure.

   Resolution: rerunning after the devtools build completed produced `app MCP connected` and listed
   both `sandbox.default` and `sandbox.error`. No code change was required.

2. [x] The custom source editor does not consume queued eguidev value overrides and does not
   publish multiline text-edit metadata.

   Evidence: `editor.source` is instrumented through `eguidev::id_with_meta`, but the custom
   `TextEdit` path does not call `take_widget_value_override` before rendering. eguidev documents
   this as required for custom widgets, and without it smoke tests that use `set_value` can observe
   the old editor buffer even though the widget advertises itself as a text edit.

   Additional evidence: `edev smoke --fail-fast --verbose` failed `00_bootstrap.luau` with `source
   editor should be multiline`, because the custom `id_with_meta` path did not publish
   `RoleState::TextEdit`.

   Resolution: the editor now consumes queued text overrides and records itself with
   `track_response_full`, `WidgetMeta`, and `RoleState::TextEdit { multiline: true, password:
   false }`.

3. [x] The diagnostics pane can push the output pane below the visible viewport.

   Evidence: `vp:check_layout()` reported `output.heading` and `output.text` as clipped and
   offscreen in the default fixture. The right column rendered an unconstrained diagnostics
   scroll area followed by the output pane, so diagnostics could consume the whole column height.

   Resolution: the right column now allocates explicit vertical slots for diagnostics and output
   before rendering them. Verified by `edev smoke --fail-fast --verbose`, including the
   `30_layout.luau` default and narrower-viewport checks.
