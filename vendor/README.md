# Maintained Dependency Patch

`glib/` is the crates.io source for `glib 0.18.5`, including its original
MIT license and copyright notices. Upstream crate SHA-256:
`233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`.

The only source change is in `src/variant_iter.rs`: the C out-pointer is
declared mutable and passed as `&mut p`, matching
[upstream PR 1343](https://github.com/gtk-rs/gtk-rs-core/pull/1343).
This backports the fix for
[RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html).

Tauri's GTK3/WebKitGTK adapter requires the 0.18 API. Upgrading only glib
to 0.20 would not upgrade that adapter and would leave a second old copy.
The root `[patch.crates-io]` makes the fix apply to the entire dependency graph.

Verification:

- `node scripts/check-vendor.mjs` checks the pinned package and patched source.
- Linux CI runs `cargo test -p millo-platform-tests --release --locked`.
  Optimizations are essential: the original UB may be invisible in debug builds.
- RustSec version matching alone does not certify a local source patch.

Remove this patch when the desktop adapter no longer resolves glib 0.18.
This fixes the listed UB, not GTK3's upstream maintenance status.
