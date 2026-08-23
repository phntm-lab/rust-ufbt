# Reference vectors

Byte-exact expectations for output that must match uFBT character for character, captured by
running the implementation this crate ports rather than re-read from its source.

## `log_format.json`

Every rendering `log::format` performs. The shared timestamp is
`2026-08-23 14:05:09.007` local time, referred to below as `ts`, and every bar uses the
default width of 72 unless the key says otherwise.

| Key | Call |
| --- | --- |
| `ts` | `timestamp(ts)` |
| `ts_min` | `timestamp(2026-01-02 00:00:00.000)` |
| `ts_max` | `timestamp(2026-12-31 23:59:59.999)` |
| `ts_micro` | `timestamp(2026-08-23 14:05:09.007999)` |
| `msg_<level>` | `message(ts, <level>, "hello world")` |
| `msg_empty` | `message(ts, info, "")` |
| `build_none` | `build("CC", "src/main.c", [])` |
| `build_one` | `build("CC", "src/main.c", ["-Os"])` |
| `build_many` | `build("LINK", "app.fap", ["-Os", "-Wall"])` |
| `build_empty` | `build("", "", [""])` |
| `pct_<value>` | `percent(<value>)` |
| `bar_0` / `bar_50` / `bar_100` | `progress_bar(running, 0 / 100 / 200 of 200)` |
| `bar_indeterminate` | `progress_bar(running, 5, no total)` |
| `bar_indeterminate_finished` | `progress_bar(finished, 5, no total)` |
| `bar_failed` | `progress_bar(failed, 1 of 200)` |
| `bar_over` | `progress_bar(running, 500 of 200)` |
| `bar_7_of_72` | `progress_bar(running, 7 of 72)` |
| `bar_width_<n>` | `progress_bar(running, 1 of 3, width <n>)` |
| `bar_width_10_done` | `progress_bar(finished, 1 of 3, width 10)` |
