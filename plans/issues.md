# Sandbox Issues

1. [x] `edev fixtures` times out during MCP initialization for the sandbox app.

   Evidence: `edev fixtures --verbose` spawned `cargo run --quiet -p ruau-sandbox-gui --features
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

4. [x] The output pane is a single dynamic label instead of a full-width scrollable run history.

   Evidence: the new eguidev smoke round failed `35_output_history_layout.luau` at line 13 while
   waiting for `output.text` to contain both `Run 1` / `area = 42` and `Run 2` / `value = 15`.
   The current UI replaces the previous run with the latest output and sizes the output surface to
   the current label instead of exposing a left-aligned, scrollable record area that fills its
   allocated slot.

   Target resolution: keep a chronological output record for script runs, expose `output.history`
   as a real eguidev scroll area, make `output.panel` fill the available output slot, and keep the
   output text left aligned with no unallocated black gap below the pane.

   Resolution: the GUI now stores numbered run records, renders them in a left-aligned
   `output.history` scroll area, and uses `output.panel` as the full-height output surface. Verified
   with `35_output_history_layout.luau`, including multiple-run retention, long-output scrolling,
   full-width history geometry, left-aligned text, and bottom-fill assertions at 900x620 and
   700x520.

5. [x] The source editor can extend offscreen at narrow heights after loading a long script.

   Evidence: after the output history and eguidev overlap fixes, `35_output_history_layout.luau`
   progressed to its 700x520 viewport check and `vp:check_layout()` reported
   `offscreen [editor.source]: Widget is outside the viewport`. The editor column is still driven by
   the multiline editor's requested content height instead of being bounded to the remaining
   viewport height.

   Target resolution: allocate the editor a fixed remaining-height slot, keep it internally
   scrollable, and preserve a clean layout at the 700x520 eguidev viewport used by the regression
   test.

   Resolution: the custom editor is now rendered inside an `editor.scroll` area and publishes
   captured layout metadata for `editor.source`, so long scripts scroll inside the editor instead of
   expanding the page. Verified by the passing 700x520 `35_output_history_layout.luau` layout check
   and the full `edev smoke --fail-fast --verbose` suite.
