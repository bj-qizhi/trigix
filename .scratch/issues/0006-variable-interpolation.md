# Variable Interpolation in Node Configs

## Status

done

## Category

enhancement

## What to build

Allow node configs to reference prior node outputs and workflow input using `{{expr}}` template syntax. Makes multi-node workflows actually composable.

## Template syntax

| Expression | Resolves to |
|---|---|
| `{{input}}` | raw `input_json` string |
| `{{input.field}}` | dot-path field from `input_json` |
| `{{node_id}}` | raw output_json of that completed node |
| `{{node_id.field.nested}}` | dot-path field from node output |

## Acceptance criteria

- [x] `resolve_template(template, context)` replaces all `{{...}}` patterns; unresolvable → empty string.
- [x] `resolve_config_strings(config, context)` recurses into JSON objects/arrays to resolve all string values.
- [x] HTTP node: URL, body, and header values resolve templates before making the request.
- [x] Condition node: `field` supports `{{expr}}` syntax (resolves to a value for direct comparison); `equals` also resolves templates.
- [x] AI Runtime: `_resolve_template()` handles full `{{node_id.field}}` syntax using `node_outputs`; replaces the old single-pattern `str.replace("{{input}}", ...)`.
- [x] Frontend: `TemplateHint` component shows `{{input.field}}` · `{{node_id.field}}` below HTTP URL, HTTP body, Agent prompt, and Condition equals fields.
- [x] 5 new Rust tests: `template_resolver_replaces_input_field`, `template_resolver_replaces_node_output_field`, `template_resolver_handles_missing_keys_gracefully`, `http_node_resolves_url_template`, `condition_node_uses_template_in_field`.
- [x] 51 Rust tests passing, AI Runtime syntax-clean, TypeScript zero errors.

## Notes

- Condition `field` backward compatible: plain field name (no `{{`) still looks up key in `input_json`.
- Missing node/field resolves to empty string (not an error) — matches mustache convention.
