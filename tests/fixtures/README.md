# Reference vectors

Byte-exact expectations captured by running the original Dart implementation, so the port can
be checked against it rather than against a re-reading of its source.

## `log_format.json`

Output of every `UfbtLogFormat` function from `dart-ufbt/lib/log/log_format.dart`, produced
with Dart SDK 3.12.2. Keys and their inputs:

| Key | Call |
| --- | --- |
| `ts` | `timestamp(DateTime(2026, 8, 23, 14, 5, 9, 7))` |
| `ts_min` | `timestamp(DateTime(2026, 1, 2, 0, 0, 0, 0))` |
| `ts_max` | `timestamp(DateTime(2026, 12, 31, 23, 59, 59, 999))` |
| `ts_micro` | `timestamp(DateTime(2026, 8, 23, 14, 5, 9, 7, 999))` |
| `msg_<level>` | `message(ts, <level>, 'hello world')` |
| `msg_empty` | `message(ts, info, '')` |
| `build_none` | `build('CC', 'src/main.c', [])` |
| `build_one` | `build('CC', 'src/main.c', ['-Os'])` |
| `build_many` | `build('LINK', 'app.fap', ['-Os', '-Wall'])` |
| `build_empty` | `build('', '', [''])` |
| `pct_<value>` | `percent(<value>)` |
| `bar_0` / `bar_50` / `bar_100` | `progressBar(running, 0 / 100 / 200 of 200)` |
| `bar_indeterminate` | `progressBar(running, 5, total: null)` |
| `bar_indeterminate_finished` | `progressBar(finished, 5, total: null)` |
| `bar_failed` | `progressBar(failed, 1 of 200)` |
| `bar_over` | `progressBar(running, 500 of 200)` |
| `bar_7_of_72` | `progressBar(running, 7 of 72)` |
| `bar_width_<n>` | `progressBar(running, 1 of 3, width: <n>)` |
| `bar_width_10_done` | `progressBar(finished, 1 of 3, width: 10)` |

`ts` is the shared timestamp `DateTime(2026, 8, 23, 14, 5, 9, 7)`; every bar uses the default
width of 72 unless the key says otherwise.
