# Using hpt-validator from an agent

`hpt-validator` checks a CMS Hospital Price Transparency machine-readable file (CSV, tall
or wide, or JSON) against a data dictionary version and reports errors and alerts.

## Run

```sh
hpt-validator <file> <version> -o json -e 0 --extended
```

- `<version>`: `v2.1`, `v2.2` or `v3.0`.
- `-o json`: one JSON object on standard output, nothing else on it.
- `-e 0`: every error. The default stops at 1000 errors and keeps at most 1000 alerts.
- `--extended`: adds `fileType`, `extended` and `line` (below) and changes nothing else.
- `-` as `<file>` reads standard input; give `-f csv` or `-f json` with it.
- A `.gz` file is decompressed. The format comes from the extension unless `-f` is given.

## Exit codes

| Code | Meaning | Output |
| --- | --- | --- |
| 0 | Valid: no errors. Alerts may be present. | the JSON object |
| 1 | Invalid: at least one error. | the JSON object |
| 2 | Usage error | a message on standard error |
| 2 | Neither CSV nor JSON, by extension or `-f` | `{"error": "..."}` on standard output with `-o json` |
| 3 | The input could not be read | a message on standard error |

## Result

| Key | Type | Meaning |
| --- | --- | --- |
| `file` | string | Absolute path of the file, `-` for standard input |
| `validatorVersion` | string | Version of the `hpt-validator` crate |
| `dataDictionaryVersion` | string | `2.1.0`, `2.2.0` or `3.0.0` |
| `timestamp` | string | When validation finished, ISO 8601 UTC |
| `valid` | boolean | `errorCount == 0` |
| `errorCount`, `alertCount` | number | Lengths of `errors` and `alerts` |
| `errors`, `alerts` | array of findings | In upstream's order |
| `fileType` | string | Extended only: `json`, `tall`, `wide`, or `unknown` for a CSV whose shape could not be determined |
| `extended` | array of findings | Extended only: findings upstream drops or never reports |

## Findings

| Key | Present | Meaning |
| --- | --- | --- |
| `path` | always | CSV: a cell (`"D5"`) or a row (`"row 5"`); JSON: a JSON pointer (`"/standard_charge_information/3"`) |
| `message` | always | Upstream's text, for people. Its wording can change; do not parse it. |
| `field` | JSON | Last segment of the path within the item |
| `columnName`, `value` | CSV, some codes | The column or header and the offending value |
| `allowedValues` | `HPT_CSV_ALLOWED_VALUES` | Values the cell may take |
| `versions` | `HPT_CSV_INVALID_VERSION` | Versions that can be validated against |
| `column` | `HPT_CSV_PLACEHOLDER` | Zero-based column of the placeholder |
| `originalError` | `HPT_JSON_INVALID` | Always `{}` |
| `code` | always | Stable code, the key to act on |
| `keyword` | `HPT_JSON_SCHEMA`, `HPT_JSON_ESTIMATED_AMOUNT_DEPRECATED`, `HPT_JSON_NINE_NINES` | The JSON Schema keyword that failed |
| `line` | Extended, CSV findings tied to a row | Physical line, from 1, where the finding's row starts |

Codes start with `HPT_CSV_` or `HPT_JSON_`, and `HPT_X_` for Extended findings. The full
list, with the upstream class each code stands for, is in
[`hpt-validator`'s documentation](https://github.com/digitalashes/hpt-validator-rs#codes).
Renaming a code is a breaking change.

## Reading a result

- Fix errors first; alerts are warnings and never make a file invalid.
- Row numbers in `path` count CSV rows, which can differ from lines when a quoted field
  spans lines; use `line` from `--extended` to open the file at the right place.
- Some errors hide others. A blank row 3, fewer than four rows or a JSON syntax error
  replaces every error found before it; with `--extended` those earlier errors are kept
  in `extended`. Rerun after fixing.
- With a limit (`-e n`, default 1000) the lists stop at `n` and `valid` still reflects
  whether any error was found.
