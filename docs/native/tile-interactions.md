# Native tile controls: issues #72 and #73

This change starts from integration commit `f6e18fede6e9f633d063840f9268f994d0c3a224`.
It changes the Race-control feed settings, Notice activation, Stint expansion controls,
and accessible names for the Track map and timing-tower tyre badges. It does not change
the gpui dependency or establish visual or screen-reader release evidence.

## Behavior and tests

The settings dialog offers all eight Race-control feed entry types as independent
checkboxes. Missing or non-array settings select all types. Arrays retain their string
entries, including unknown types; non-string entries are ignored, matching the web
build. Clearing the last selected type saves an empty array. The shared list field sends
only `entryTypes` through `Command::Setting`, preserving the other settings.

`feed_settings_toggle_known_types_without_losing_future_types_or_empty_selection`
tests defaults, unknown strings, non-string filtering and empty selections.
`feed_checkbox_changes_use_the_layout_draft_and_preserve_other_settings_on_save` tests
the public dashboard controller: edits remain a draft, Cancel restores the original,
and Done persists the list and unrelated settings. Its files use a fresh temporary
folder; it never opens the user's layouts.

The Notice tile keeps the exact short line at both densities. Ordinary dashboard
activation opens the existing status window, scrolled to the full notice at the bottom.
The link has a tab stop and a visible focus border. Stream output and edit mode keep
the text without the link control. The tests
`both_densities_show_the_spec_wording_word_for_word` and
`full_notice_is_available_in_ordinary_view_but_not_stream_or_edit_mode` cover the text
and destination policy; they do not launch or inspect a window.

Completed Stint rows expose button names containing the compound and lap range, plus
their expanded state. They have tab stops, a visible focus border and a minimum height
of 44 logical pixels. The dashboard handles Tab and Shift+Tab traversal; gpui dispatches
Enter and Space activation to the same click handler as the pointer.
Retained focus handles and the tile's scroll handle bring a newly focused completed
set into view without resetting its expansion state.
`completed_set_control_names_the_set_and_reports_its_expansion` and
`a_completed_set_expands_and_collapses_on_its_own` test the public control model and
expansion memory. They do not simulate operating-system keyboard input.

The Track map image name includes its track name and existing car-state summary,
without adding driver names or a live region. The timing-tower tyre image name uses
the existing compound label. `map_accessible_name_keeps_car_state_without_exposing_driver_names`
and `tyres_have_a_coloured_letter_badge_and_an_accessible_compound_name` test those
view models. Compilation verifies that the renderer calls supported gpui APIs; these
tests do not inspect the Windows accessibility tree.

## Pinned framework evidence

The inspected dependency checkout is
`C:/Users/Vankata/.cargo/git/checkouts/zed-a70e2ad075855582/654efae`, at exactly
`654efae441bc8eb117d0042bb28cd7cf3570bdc9`, matching `Cargo.lock`. Its tracked source
is unmodified. Earlier claims that this gpui lacks accessible names or Windows
accessibility support were incorrect.

- [`gpui/src/elements/div.rs`](https://github.com/zed-industries/zed/blob/654efae441bc8eb117d0042bb28cd7cf3570bdc9/crates/gpui/src/elements/div.rs#L1354)
  provides `role`, `aria_label`, `aria_expanded` and `aria_toggled`. Nodes require an
  element ID and role. The same file provides `tab_stop` at line 824, `focus_visible`
  at line 1342 and `focusable` at line 1563.
- [Keyboard click handling in `div.rs`](https://github.com/zed-industries/zed/blob/654efae441bc8eb117d0042bb28cd7cf3570bdc9/crates/gpui/src/elements/div.rs#L3075)
  synthesizes an Enter/Space click on key release while focus is unchanged. Modified
  keys do not activate the control.
- [`gpui_windows/src/window.rs`](https://github.com/zed-industries/zed/blob/654efae441bc8eb117d0042bb28cd7cf3570bdc9/crates/gpui_windows/src/window.rs#L1082)
  initializes the AccessKit Windows adapter in `a11y_init` and forwards active tree
  updates in `a11y_tree_update` at line 1102.
- [`gpui_windows/src/events.rs`](https://github.com/zed-industries/zed/blob/654efae441bc8eb117d0042bb28cd7cf3570bdc9/crates/gpui_windows/src/events.rs#L846)
  handles `WM_GETOBJECT` through `accesskit_windows`, returning its UI Automation
  provider. This is evidence of platform support, not proof of screen-reader output.

## Remaining release gates

Manual Windows verification must still check checkbox activation, keyboard traversal,
focus visibility during live redraws, Notice navigation and initial scroll position,
and screen-reader names and expanded states. Contrast, target sizes across window
classes, and side-by-side comparison with the web build on the same real recording
remain release gates under VISUAL-02 and issues #72/#73. No game process or original
user data is accessed by this change's automated checks.

## Automated verification

- `cargo test -p pitwall-native --locked --bin pitwall-native`: 202 passed.
- `cargo test --workspace --locked`: 769 passed, 8 ignored, no failures. The ignored
  tests require manual UDP/throughput checks, real-data copies or an interactive
  Windows overlay check; none is used as tile-interaction evidence.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo build -p pitwall-native --release --locked`: passed.
- `pnpm install --frozen-lockfile` and `pnpm build`: passed. The web bundle is required
  by the collector's build script for workspace checks and remains in the product.

The new selection, destination, expansion-control and map-name tests were introduced
before their corresponding APIs and first failed to compile. The controller test then
verified draft, Cancel and persisted Done behavior through the public window interface.
Cargo checks use fresh TEMP/TMP directories to avoid legacy test-directory collisions.
