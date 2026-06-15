# eguidev Issues

1. [x] `check_layout()` reports ancestor/descendant container overlap as a layout issue.

   Evidence from the sandbox: `vp:check_layout()` reported overlaps between `app.toolbar` and its
   child widgets (`status.check`, `action.reset`, `action.sample_error`, `action.run`,
   `action.check`). These are expected parent/child containment relationships, not sibling
   collisions. The eguidev overlap checker currently compares all visible non-scroll-area widgets
   pairwise without consulting `parent_id`.

   Resolution: `check_overlaps` now skips ancestor/descendant pairs while preserving sibling
   overlap detection. Verified with
   `cargo nextest run -p eguidev_runtime overlap_checks_ignore_ancestor_descendant_widgets`.

2. [x] `check_layout()` reports scroll-content overlap against widgets outside the scroll area.

   Evidence from the sandbox: after converting output into a scrollable run history, eguidev smoke
   failed `35_output_history_layout.luau` with overlaps between `output.text` and unrelated widgets
   outside its scroll ancestor (`app.toolbar`, `diagnostics.heading`, `diagnostics.empty`, and
   `output.heading`). The text is scroll-area content with a nested clip region, so these are not
   actionable page-level sibling collisions.

   Target resolution: teach `check_overlaps` to ignore pairs where either widget is clipped by a
   nested scroll/clip region, matching the existing clipping, overflow, and offscreen behavior for
   scroll-area descendants.

   Resolution: `check_overlaps` now compares a widget's visible clipped rectangle when it has a
   nested clip region, preserving real overlap detection while avoiding scroll-content false
   positives. Verified with
   `cargo nextest run -p eguidev_runtime overlap_checks_use_visible_rect_for_scroll_content
   overlap_checks_ignore_ancestor_descendant_widgets` and the sandbox's passing
   `35_output_history_layout.luau`.

3. [x] Custom `track_response_full` widgets cannot attach eguidev-captured layout metadata.

   Evidence from the sandbox: after wrapping the custom `editor.source` text edit in a scroll area,
   `check_layout()` still reported `offscreen [editor.source]` because the custom
   `track_response_full` call publishes rich text-edit role metadata but no `WidgetLayout`. The
   layout capture helper exists internally in eguidev, but it is not exported for custom widget
   instrumentation.

   Target resolution: expose `capture_layout` from eguidev so custom widgets can publish the same
   clip-aware layout metadata as `id_with_meta` while still supplying role-specific state.

   Resolution: eguidev now exports `capture_layout`, and the sandbox uses it for the custom
   `editor.source` text edit metadata. Verified by `cargo test -p ruau-sandbox-gui` and the passing
   eguidev layout smoke round.
