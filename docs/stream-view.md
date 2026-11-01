# Stream view

In OBS, add a Browser Source pointing to:

`http://<collector>:8777/?layout=Pitwall%20Classic&stream=1`

Start at 1920×1080. Enable **Use custom frame rate** and set it to **60**; OBS leaves this disabled by default. Actual OBS frame rate still needs device testing.

Optional URL parameters:

- `class=wide|tablet|phone` forces an arrangement.
- `speed=kmh|mph`, `temp=c|f`, and `pressure=psi|bar` override device units without saving them.

Stream view follows the player. Set a tile's Pinned driver in the saved layout to follow someone else. A one-tile layout fills the source.

Missing layouts stay blank. For #48 integration, `ControllerOptions.onMissingStreamLayout(name)` reports the name; `null` clears it after recovery. The collector logging/status route remains unwired.
