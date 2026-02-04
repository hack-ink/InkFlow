# Submodule Updates

Purpose: Provide a repeatable procedure for updating the `sherpa-onnx` submodule and the related FFI bindings.

Scope: This guidance applies to `third_party/sherpa-onnx` and the `packages/sherpa-onnx-sys` vendored C header.

## When to update

Update the submodule only when you need upstream fixes, models, or API changes. Avoid routine updates without a reason.

## Procedure

1. Start from a clean working tree.
2. Record the current submodule revision with `git submodule status`.
3. Update the submodule to the latest remote commit with `git submodule update --init --remote third_party/sherpa-onnx`.
4. Capture the new revision with `git submodule status`.
5. If you need a specific commit or tag, run `git -C third_party/sherpa-onnx checkout <sha-or-tag>` and then `git add third_party/sherpa-onnx`.
6. Check whether the C API header changed by running `git -C third_party/sherpa-onnx diff <old-sha>..<new-sha> -- sherpa-onnx/c-api/c-api.h`.
7. If the header changed, update the vendored header with `cp third_party/sherpa-onnx/sherpa-onnx/c-api/c-api.h packages/sherpa-onnx-sys/vendor/sherpa_onnx_c_api.h`.
8. Rebuild the bindings to confirm they still generate with `cargo build -p sherpa-onnx-sys`.
9. Commit the updates with a signed, schema-compliant message. Example:

```text
{"schema":"cmsg/1","type":"chore","scope":"third-party","summary":"Update sherpa-onnx submodule","intent":"Update the sherpa-onnx dependency reference.","impact":"Ensures the repository tracks the updated sherpa-onnx revision.","breaking":false,"risk":"low","refs":[]}
```

## Notes

- `cargo make setup-macos` will initialize or update the submodule, but you still need to commit the gitlink change.
- `packages/sherpa-onnx-sys` uses `vendor/sherpa_onnx_c_api.h` as the bindgen source of truth. Keep it in sync when the upstream header changes.
