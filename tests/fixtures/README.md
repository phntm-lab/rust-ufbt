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

## `console_sink.json`

Everything `log::ConsoleSink` writes for a given run of events, captured through an injected
output. Unless a key says otherwise the bar is 72 wide and one `#` stands for 300 units.

| Key | Events fed to the sink |
| --- | --- |
| `message` | one message event |
| `build` | one build event |
| `raw_newline` / `raw_no_newline` | one raw event, with and without a line break |
| `started` | a task starting with the title `Downloading` |
| `started_empty_title` | a task starting without a title |
| `running_determinate` | running, 50 of 200 |
| `running_indeterminate` | started, then running at 299, 300, 1500 and 1500 with no total |
| `running_indeterminate_without_started` | running at 900 with no total, never started |
| `finished_determinate` | finished, 200 of 200 |
| `finished_indeterminate` | started, running at 600, finished at 1000, all with no total |
| `failed` | a failed task |
| `two_tasks` | two tasks with different ids, interleaved |
| `reused_id_after_finish` | a task finished, then started again under the same id |
| `custom_bar_width` | running, 1 of 3, bar 10 wide |
| `custom_units_per_hash` | started, then running at 10 with no total, 4 units per `#` |

## `state_json.json`

Every byte `state::State::write` puts on disk, captured by writing each set of values with
uFBT and reading the file back. Each entry holds the `values` that were written and the
`encoded` text they produced.

| Key | Values |
| --- | --- |
| `typical` | a realistic state: hardware target, mode, channel, version, index URL |
| `empty` | no keys at all |
| `single` | one key |
| `insertion_order` | four keys in an order that is neither sorted nor reversed |
| `nulls` | a null next to a string |
| `scalars` | integer, negative, zero, fractional and whole doubles, both booleans, null |
| `nested_object` | a nested object, an object nested inside that, and an empty object |
| `lists` | an empty list, a list of strings, and a list mixing every kind of value |
| `escapes` | quote, backslash, the five short escapes, C0 controls, DEL, U+2028, a slash, non-ASCII text, an emoji, an empty string |
| `empty_key` | an empty key and an empty value |
