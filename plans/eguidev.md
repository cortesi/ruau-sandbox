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
