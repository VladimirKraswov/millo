# Third-Party Notices

Generated from Cargo.lock and package-lock.json. Regenerate with
`npm run notices:generate`; CI verifies reproducibility with `npm run test:notices`.

The desktop distribution includes `third-party-notices.json` in its resources:
package metadata and deduplicated, verbatim license/copyright/notice files.
Rust build/test and all resolved platform dependencies are included conservatively;
the list does not imply every package is linked on every platform. OS frameworks
and user-installed plugins are outside this inventory. External plugin authors
must provide their own licenses. This inventory does not assign a license to Millo.

The GTK-compatible glib patch is documented in [vendor/README.md](vendor/README.md).
Dependencies without a license-text file in their packaged source are listed below;
declared SPDX metadata alone is not a substitute for distribution legal review.

## Packages Without Packaged Notice Text

- cargo: alloc-stdlib 0.2.4 (BSD-3-Clause)
- cargo: block2 0.6.2 (MIT)
- cargo: defmt-parser 1.0.0 (MIT OR Apache-2.0)
- cargo: dispatch2 0.3.1 (Zlib OR Apache-2.0 OR MIT)
- cargo: dlopen2 0.8.2 (MIT)
- cargo: dlopen2_derive 0.4.3 (MIT)
- cargo: flo_curves 0.3.1 (Apache-2.0)
- cargo: jni-sys-macros 0.4.1 (MIT OR Apache-2.0)
- cargo: lazy-regex-proc_macros 3.6.1 (MIT)
- cargo: libappindicator-sys 0.9.0 (Apache-2.0 OR MIT)
- cargo: ndk-sys 0.6.0+11769913 (MIT OR Apache-2.0)
- cargo: ndk 0.9.0 (MIT OR Apache-2.0)
- cargo: objc2-app-kit 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-cloud-kit 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-core-data 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-core-foundation 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-core-graphics 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-core-image 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-core-location 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-core-text 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-encode 4.1.0 (MIT)
- cargo: objc2-exception-helper 0.1.1 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-foundation 0.3.2 (MIT)
- cargo: objc2-io-surface 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-quartz-core 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-ui-kit 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-user-notifications 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2-web-kit 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- cargo: objc2 0.6.4 (MIT)
- cargo: r-efi 5.3.0 (MIT OR Apache-2.0 OR LGPL-2.1-or-later)
- cargo: r-efi 6.0.0 (MIT OR Apache-2.0 OR LGPL-2.1-or-later)
- cargo: rhai_codegen 3.2.0 (MIT OR Apache-2.0)
- cargo: selectors 0.36.1 (MPL-2.0)
- cargo: tauri-plugin 2.6.3 (Apache-2.0 OR MIT)
- cargo: unic-char-property 0.9.0 (MIT/Apache-2.0)
- cargo: unic-char-range 0.9.0 (MIT/Apache-2.0)
- cargo: unic-common 0.9.0 (MIT/Apache-2.0)
- cargo: unic-ucd-ident 0.9.0 (MIT/Apache-2.0)
- cargo: unic-ucd-version 0.9.0 (MIT/Apache-2.0)
- cargo: vtracer 1.0.0-alpha.3 (MIT OR Apache-2.0)
- cargo: webview2-com-macros 0.8.1 (MIT)
- cargo: webview2-com-sys 0.38.2 (MIT)
- cargo: webview2-com 0.38.2 (MIT)
- cargo: winapi-i686-pc-windows-gnu 0.4.0 (MIT/Apache-2.0)
- cargo: winapi-x86_64-pc-windows-gnu 0.4.0 (MIT/Apache-2.0)

## Dependency Inventory

| Ecosystem | Package | Version | Declared License | Notice Files |
| --- | --- | --- | --- | --- |
| cargo | adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | 3 |
| cargo | ahash | 0.8.12 | MIT OR Apache-2.0 | 2 |
| cargo | aho-corasick | 1.1.5 | Unlicense OR MIT | 2 |
| cargo | aliasable | 0.1.3 | MIT | 1 |
| cargo | alloc-no-stdlib | 2.0.4 | BSD-3-Clause | 1 |
| cargo | alloc-stdlib | 0.2.4 | BSD-3-Clause | 0 |
| cargo | android_system_properties | 0.1.6 | MIT OR Apache-2.0 | 2 |
| cargo | anyhow | 1.0.104 | MIT OR Apache-2.0 | 2 |
| cargo | arrayref | 0.3.9 | BSD-2-Clause | 1 |
| cargo | arrayvec | 0.7.8 | MIT OR Apache-2.0 | 2 |
| cargo | async-trait | 0.1.92 | MIT OR Apache-2.0 | 2 |
| cargo | atk-sys | 0.18.2 | MIT | 1 |
| cargo | atk | 0.18.2 | MIT | 2 |
| cargo | atomic-waker | 1.1.2 | Apache-2.0 OR MIT | 3 |
| cargo | autocfg | 1.5.1 | Apache-2.0 OR MIT | 2 |
| cargo | base64 | 0.21.7 | MIT OR Apache-2.0 | 2 |
| cargo | base64 | 0.22.1 | MIT OR Apache-2.0 | 2 |
| cargo | bit-set | 0.8.0 | Apache-2.0 OR MIT | 2 |
| cargo | bit-vec | 0.6.3 | MIT/Apache-2.0 | 2 |
| cargo | bit-vec | 0.8.0 | Apache-2.0 OR MIT | 2 |
| cargo | bitflags | 1.3.2 | MIT/Apache-2.0 | 2 |
| cargo | bitflags | 2.13.1 | MIT OR Apache-2.0 | 2 |
| cargo | block-buffer | 0.10.4 | MIT OR Apache-2.0 | 2 |
| cargo | block2 | 0.6.2 | MIT | 0 |
| cargo | brotli-decompressor | 5.0.3 | BSD-3-Clause/MIT | 1 |
| cargo | brotli | 8.0.4 | BSD-3-Clause AND MIT | 2 |
| cargo | bs58 | 0.5.1 | MIT/Apache-2.0 | 2 |
| cargo | bumpalo | 3.20.3 | MIT OR Apache-2.0 | 2 |
| cargo | bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | 3 |
| cargo | byteorder-lite | 0.1.0 | Unlicense OR MIT | 1 |
| cargo | byteorder | 1.5.0 | Unlicense OR MIT | 2 |
| cargo | bytes | 1.12.1 | MIT | 1 |
| cargo | cairo-rs | 0.18.5 | MIT | 2 |
| cargo | cairo-sys-rs | 0.18.2 | MIT | 1 |
| cargo | camino | 1.2.5 | MIT OR Apache-2.0 | 2 |
| cargo | cargo-platform | 0.1.9 | MIT OR Apache-2.0 | 2 |
| cargo | cargo_metadata | 0.19.2 | MIT | 1 |
| cargo | cargo_toml | 0.22.3 | Apache-2.0 OR MIT | 1 |
| cargo | cc | 1.4.2 | MIT OR Apache-2.0 | 2 |
| cargo | cesu8 | 1.1.0 | Apache-2.0/MIT | 1 |
| cargo | cfb | 0.7.3 | MIT | 1 |
| cargo | cfg-expr | 0.15.8 | MIT OR Apache-2.0 | 2 |
| cargo | cfg-if | 1.0.4 | MIT OR Apache-2.0 | 2 |
| cargo | cfg_aliases | 0.2.2 | MIT | 1 |
| cargo | chrono | 0.4.45 | MIT OR Apache-2.0 | 1 |
| cargo | clipper2 | 0.6.0 | MIT OR Apache-2.0 | 2 |
| cargo | clipper2c-sys | 0.2.0 | MIT OR Apache-2.0 | 2 |
| cargo | combine | 4.6.7 | MIT | 1 |
| cargo | const-random-macro | 0.1.16 | MIT OR Apache-2.0 | 2 |
| cargo | const-random | 0.1.18 | MIT OR Apache-2.0 | 2 |
| cargo | convert_case | 0.10.0 | MIT | 1 |
| cargo | cookie | 0.18.2 | MIT OR Apache-2.0 | 2 |
| cargo | core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 | 2 |
| cargo | core-foundation | 0.10.1 | MIT OR Apache-2.0 | 2 |
| cargo | core-graphics-types | 0.2.0 | MIT OR Apache-2.0 | 2 |
| cargo | core-graphics | 0.25.0 | MIT OR Apache-2.0 | 3 |
| cargo | cpufeatures | 0.2.17 | MIT OR Apache-2.0 | 2 |
| cargo | crc32fast | 1.5.0 | MIT OR Apache-2.0 | 2 |
| cargo | crossbeam-channel | 0.5.16 | MIT OR Apache-2.0 | 3 |
| cargo | crossbeam-utils | 0.8.22 | MIT OR Apache-2.0 | 2 |
| cargo | crunchy | 0.2.4 | MIT | 1 |
| cargo | crypto-common | 0.1.7 | MIT OR Apache-2.0 | 2 |
| cargo | cssparser-macros | 0.6.1 | MPL-2.0 | 1 |
| cargo | cssparser | 0.36.0 | MPL-2.0 | 1 |
| cargo | ctor-proc-macro | 0.0.7 | Apache-2.0 OR MIT | 2 |
| cargo | ctor | 0.8.0 | Apache-2.0 OR MIT | 2 |
| cargo | darling | 0.23.0 | MIT | 1 |
| cargo | darling_core | 0.23.0 | MIT | 1 |
| cargo | darling_macro | 0.23.0 | MIT | 1 |
| cargo | data-url | 0.3.2 | MIT OR Apache-2.0 | 2 |
| cargo | dbus | 0.9.12 | Apache-2.0/MIT | 2 |
| cargo | defmt-macros | 1.1.1 | MIT OR Apache-2.0 | 2 |
| cargo | defmt-parser | 1.0.0 | MIT OR Apache-2.0 | 0 |
| cargo | defmt | 1.1.1 | MIT OR Apache-2.0 | 2 |
| cargo | deranged | 0.5.8 | MIT OR Apache-2.0 | 2 |
| cargo | derive_more-impl | 2.1.1 | MIT | 1 |
| cargo | derive_more | 2.1.1 | MIT | 1 |
| cargo | digest | 0.10.7 | MIT OR Apache-2.0 | 2 |
| cargo | dirs-sys | 0.5.0 | MIT OR Apache-2.0 | 2 |
| cargo | dirs | 6.0.0 | MIT OR Apache-2.0 | 2 |
| cargo | dispatch2 | 0.3.1 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | displaydoc | 0.2.7 | MIT OR Apache-2.0 | 2 |
| cargo | dlopen2 | 0.8.2 | MIT | 0 |
| cargo | dlopen2_derive | 0.4.3 | MIT | 0 |
| cargo | dom_query | 0.27.0 | MIT | 1 |
| cargo | dpi | 0.1.2 | Apache-2.0 AND MIT | 2 |
| cargo | dtoa-short | 0.3.5 | MPL-2.0 | 1 |
| cargo | dtoa | 1.0.11 | MIT OR Apache-2.0 | 2 |
| cargo | dtor-proc-macro | 0.0.6 | Apache-2.0 OR MIT | 2 |
| cargo | dtor | 0.3.0 | Apache-2.0 OR MIT | 2 |
| cargo | dunce | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | 1 |
| cargo | dyn-clone | 1.0.20 | MIT OR Apache-2.0 | 2 |
| cargo | either | 1.17.0 | MIT OR Apache-2.0 | 2 |
| cargo | embed-resource | 3.0.11 | MIT | 1 |
| cargo | embed_plist | 1.2.2 | MIT OR Apache-2.0 | 2 |
| cargo | equivalent | 1.0.2 | Apache-2.0 OR MIT | 2 |
| cargo | erased-serde | 0.4.10 | MIT OR Apache-2.0 | 2 |
| cargo | errno | 0.3.14 | MIT OR Apache-2.0 | 2 |
| cargo | euclid | 0.22.14 | MIT OR Apache-2.0 | 3 |
| cargo | fastrand | 2.5.0 | Apache-2.0 OR MIT | 2 |
| cargo | fdeflate | 0.3.7 | MIT OR Apache-2.0 | 2 |
| cargo | field-offset | 0.3.6 | MIT OR Apache-2.0 | 2 |
| cargo | find-msvc-tools | 0.1.10 | MIT OR Apache-2.0 | 2 |
| cargo | flate2 | 1.1.9 | MIT OR Apache-2.0 | 2 |
| cargo | flo_curves | 0.3.1 | Apache-2.0 | 0 |
| cargo | flo_curves | 0.8.0 | Apache-2.0 | 1 |
| cargo | float-cmp | 0.9.0 | MIT | 1 |
| cargo | fnv | 1.0.7 | Apache-2.0 / MIT | 2 |
| cargo | foldhash | 0.2.0 | Zlib | 1 |
| cargo | foreign-types-macros | 0.2.4 | MIT/Apache-2.0 | 2 |
| cargo | foreign-types-shared | 0.3.1 | MIT/Apache-2.0 | 2 |
| cargo | foreign-types | 0.5.0 | MIT/Apache-2.0 | 2 |
| cargo | form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | 2 |
| cargo | futures-channel | 0.3.33 | MIT OR Apache-2.0 | 2 |
| cargo | futures-core | 0.3.33 | MIT OR Apache-2.0 | 2 |
| cargo | futures-executor | 0.3.33 | MIT OR Apache-2.0 | 2 |
| cargo | futures-io | 0.3.33 | MIT OR Apache-2.0 | 2 |
| cargo | futures-macro | 0.3.33 | MIT OR Apache-2.0 | 2 |
| cargo | futures-sink | 0.3.33 | MIT OR Apache-2.0 | 2 |
| cargo | futures-task | 0.3.33 | MIT OR Apache-2.0 | 2 |
| cargo | futures-util | 0.3.33 | MIT OR Apache-2.0 | 2 |
| cargo | gdk-pixbuf-sys | 0.18.0 | MIT | 1 |
| cargo | gdk-pixbuf | 0.18.5 | MIT | 2 |
| cargo | gdk-sys | 0.18.2 | MIT | 1 |
| cargo | gdk | 0.18.2 | MIT | 2 |
| cargo | gdkwayland-sys | 0.18.2 | MIT | 1 |
| cargo | gdkx11-sys | 0.18.2 | MIT | 1 |
| cargo | gdkx11 | 0.18.2 | MIT | 2 |
| cargo | generic-array | 0.14.7 | MIT | 1 |
| cargo | gerber-types | 0.7.0 | MIT OR Apache-2.0 | 2 |
| cargo | gerber_parser | 0.5.0 | MIT OR Apache-2.0 | 2 |
| cargo | getrandom | 0.2.17 | MIT OR Apache-2.0 | 2 |
| cargo | getrandom | 0.3.4 | MIT OR Apache-2.0 | 2 |
| cargo | getrandom | 0.4.3 | MIT OR Apache-2.0 | 2 |
| cargo | gio-sys | 0.18.1 | MIT | 1 |
| cargo | gio | 0.18.4 | MIT | 2 |
| cargo | glib-macros | 0.18.5 | MIT | 2 |
| cargo | glib-sys | 0.18.1 | MIT | 1 |
| cargo | glib | 0.18.5 | MIT | 2 |
| cargo | glob | 0.3.4 | MIT OR Apache-2.0 | 2 |
| cargo | gobject-sys | 0.18.0 | MIT | 1 |
| cargo | gtk-sys | 0.18.2 | MIT | 1 |
| cargo | gtk3-macros | 0.18.2 | MIT | 2 |
| cargo | gtk | 0.18.2 | MIT | 2 |
| cargo | hashbrown | 0.12.3 | MIT OR Apache-2.0 | 2 |
| cargo | hashbrown | 0.17.1 | MIT OR Apache-2.0 | 2 |
| cargo | heck | 0.4.1 | MIT OR Apache-2.0 | 2 |
| cargo | heck | 0.5.0 | MIT OR Apache-2.0 | 2 |
| cargo | hex | 0.4.3 | MIT OR Apache-2.0 | 2 |
| cargo | html5ever | 0.38.0 | MIT OR Apache-2.0 | 2 |
| cargo | http-body-util | 0.1.4 | MIT | 1 |
| cargo | http-body | 1.1.0 | MIT | 1 |
| cargo | http | 1.5.0 | MIT OR Apache-2.0 | 2 |
| cargo | httparse | 1.10.1 | MIT OR Apache-2.0 | 2 |
| cargo | hyper-util | 0.1.20 | MIT | 1 |
| cargo | hyper | 1.11.0 | MIT | 1 |
| cargo | iana-time-zone-haiku | 0.1.2 | MIT OR Apache-2.0 | 2 |
| cargo | iana-time-zone | 0.1.65 | MIT OR Apache-2.0 | 2 |
| cargo | ico | 0.5.0 | MIT | 1 |
| cargo | icu_collections | 2.2.0 | Unicode-3.0 | 1 |
| cargo | icu_locale_core | 2.2.0 | Unicode-3.0 | 1 |
| cargo | icu_normalizer | 2.2.0 | Unicode-3.0 | 1 |
| cargo | icu_normalizer_data | 2.2.0 | Unicode-3.0 | 1 |
| cargo | icu_properties | 2.2.0 | Unicode-3.0 | 1 |
| cargo | icu_properties_data | 2.2.0 | Unicode-3.0 | 1 |
| cargo | icu_provider | 2.2.0 | Unicode-3.0 | 1 |
| cargo | ident_case | 1.0.1 | MIT/Apache-2.0 | 1 |
| cargo | idna | 1.1.0 | MIT OR Apache-2.0 | 2 |
| cargo | idna_adapter | 1.2.2 | Apache-2.0 OR MIT | 2 |
| cargo | image | 0.25.8 | MIT OR Apache-2.0 | 2 |
| cargo | imagesize | 0.13.0 | MIT | 1 |
| cargo | indexmap | 1.9.3 | Apache-2.0 OR MIT | 2 |
| cargo | indexmap | 2.14.0 | Apache-2.0 OR MIT | 2 |
| cargo | infer | 0.19.0 | MIT | 1 |
| cargo | io-kit-sys | 0.4.1 | MIT / Apache-2.0 | 2 |
| cargo | ipnet | 2.12.1 | MIT OR Apache-2.0 | 2 |
| cargo | itertools | 0.11.0 | MIT OR Apache-2.0 | 2 |
| cargo | itertools | 0.8.2 | MIT/Apache-2.0 | 2 |
| cargo | itoa | 1.0.18 | MIT OR Apache-2.0 | 2 |
| cargo | javascriptcore-rs-sys | 1.1.1 | MIT | 1 |
| cargo | javascriptcore-rs | 1.1.2 | MIT | 1 |
| cargo | jiff-core | 0.1.0 | Unlicense OR MIT | 2 |
| cargo | jiff-static | 0.2.35 | Unlicense OR MIT | 2 |
| cargo | jiff-tzdb-platform | 0.1.3 | Unlicense OR MIT | 2 |
| cargo | jiff-tzdb | 0.1.8 | Unlicense OR MIT | 2 |
| cargo | jiff | 0.2.35 | Unlicense OR MIT | 2 |
| cargo | jni-sys-macros | 0.4.1 | MIT OR Apache-2.0 | 0 |
| cargo | jni-sys | 0.3.1 | MIT OR Apache-2.0 | 2 |
| cargo | jni-sys | 0.4.1 | MIT OR Apache-2.0 | 2 |
| cargo | jni | 0.21.1 | MIT/Apache-2.0 | 2 |
| cargo | js-sys | 0.3.104 | MIT OR Apache-2.0 | 2 |
| cargo | json-patch | 3.0.1 | MIT/Apache-2.0 | 2 |
| cargo | jsonptr | 0.6.3 | MIT OR Apache-2.0 | 2 |
| cargo | keyboard-types | 0.7.0 | MIT OR Apache-2.0 | 2 |
| cargo | kurbo | 0.11.3 | Apache-2.0 OR MIT | 2 |
| cargo | lazy-regex-proc_macros | 3.6.1 | MIT | 0 |
| cargo | lazy-regex | 3.6.1 | MIT | 1 |
| cargo | lazy_static | 1.5.0 | MIT OR Apache-2.0 | 2 |
| cargo | lib_gerber_edit | 0.5.8 | See included license file | 1 |
| cargo | libappindicator-sys | 0.9.0 | Apache-2.0 OR MIT | 0 |
| cargo | libappindicator | 0.9.0 | Apache-2.0 OR MIT | 2 |
| cargo | libc | 0.2.189 | MIT OR Apache-2.0 | 2 |
| cargo | libdbus-sys | 0.2.7 | Apache-2.0/MIT | 2 |
| cargo | libloading | 0.7.4 | ISC | 1 |
| cargo | libredox | 0.1.19 | MIT | 1 |
| cargo | litemap | 0.8.2 | Unicode-3.0 | 1 |
| cargo | lock_api | 0.4.14 | MIT OR Apache-2.0 | 2 |
| cargo | log | 0.4.33 | MIT OR Apache-2.0 | 2 |
| cargo | mach2 | 0.4.3 | BSD-2-Clause OR MIT OR Apache-2.0 | 3 |
| cargo | markup5ever | 0.38.0 | MIT OR Apache-2.0 | 2 |
| cargo | memchr | 2.8.3 | Unlicense OR MIT | 2 |
| cargo | memoffset | 0.9.1 | MIT | 1 |
| cargo | mime | 0.3.17 | MIT OR Apache-2.0 | 2 |
| cargo | miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | 4 |
| cargo | mio-serial | 5.0.7 | MIT | 1 |
| cargo | mio | 1.2.2 | MIT | 1 |
| cargo | moxcms | 0.7.11 | BSD-3-Clause OR Apache-2.0 | 2 |
| cargo | muda | 0.19.3 | Apache-2.0 OR MIT | 3 |
| cargo | ndk-sys | 0.6.0+11769913 | MIT OR Apache-2.0 | 0 |
| cargo | ndk | 0.9.0 | MIT OR Apache-2.0 | 0 |
| cargo | new_debug_unreachable | 1.0.6 | MIT | 1 |
| cargo | nix | 0.26.4 | MIT | 1 |
| cargo | nix | 0.31.3 | MIT | 1 |
| cargo | no-std-compat | 0.4.1 | MIT | 1 |
| cargo | num-bigint | 0.4.8 | MIT OR Apache-2.0 | 2 |
| cargo | num-conv | 0.2.2 | MIT OR Apache-2.0 | 2 |
| cargo | num-integer | 0.1.47 | MIT OR Apache-2.0 | 2 |
| cargo | num-rational | 0.4.2 | MIT OR Apache-2.0 | 2 |
| cargo | num-traits | 0.2.19 | MIT OR Apache-2.0 | 2 |
| cargo | num_enum | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | 3 |
| cargo | num_enum_derive | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | 3 |
| cargo | objc2-app-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-cloud-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-core-data | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-core-foundation | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-core-graphics | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-core-image | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-core-location | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-core-text | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-encode | 4.1.0 | MIT | 0 |
| cargo | objc2-exception-helper | 0.1.1 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-foundation | 0.3.2 | MIT | 0 |
| cargo | objc2-io-surface | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-quartz-core | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-ui-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-user-notifications | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2-web-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 0 |
| cargo | objc2 | 0.6.4 | MIT | 0 |
| cargo | once_cell | 1.21.4 | MIT OR Apache-2.0 | 2 |
| cargo | option-ext | 0.2.0 | MPL-2.0 | 1 |
| cargo | ouroboros | 0.17.2 | MIT OR Apache-2.0 | 2 |
| cargo | ouroboros_macro | 0.17.2 | MIT OR Apache-2.0 | 2 |
| cargo | pango-sys | 0.18.0 | MIT | 1 |
| cargo | pango | 0.18.3 | MIT | 2 |
| cargo | parking_lot | 0.12.5 | MIT OR Apache-2.0 | 2 |
| cargo | parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | 2 |
| cargo | percent-encoding | 2.3.2 | MIT OR Apache-2.0 | 2 |
| cargo | phf | 0.13.1 | MIT | 1 |
| cargo | phf_codegen | 0.13.1 | MIT | 1 |
| cargo | phf_generator | 0.13.1 | MIT | 1 |
| cargo | phf_macros | 0.13.1 | MIT | 1 |
| cargo | phf_shared | 0.13.1 | MIT | 1 |
| cargo | pico-args | 0.5.0 | MIT | 1 |
| cargo | pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | 2 |
| cargo | pkg-config | 0.3.33 | MIT OR Apache-2.0 | 2 |
| cargo | plist | 1.10.0 | MIT | 1 |
| cargo | png | 0.17.16 | MIT OR Apache-2.0 | 2 |
| cargo | png | 0.18.1 | MIT OR Apache-2.0 | 2 |
| cargo | portable-atomic-util | 0.2.7 | Apache-2.0 OR MIT | 2 |
| cargo | portable-atomic | 1.15.0 | Apache-2.0 OR MIT | 2 |
| cargo | potential_utf | 0.1.5 | Unicode-3.0 | 1 |
| cargo | powerfmt | 0.2.0 | MIT OR Apache-2.0 | 2 |
| cargo | precomputed-hash | 0.1.1 | MIT | 1 |
| cargo | proc-macro-crate | 1.3.1 | MIT OR Apache-2.0 | 2 |
| cargo | proc-macro-crate | 2.0.2 | MIT OR Apache-2.0 | 2 |
| cargo | proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 | 2 |
| cargo | proc-macro-error-attr | 1.0.4 | MIT OR Apache-2.0 | 2 |
| cargo | proc-macro-error | 1.0.4 | MIT OR Apache-2.0 | 2 |
| cargo | proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | 2 |
| cargo | pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 | 2 |
| cargo | quick-xml | 0.41.0 | MIT | 1 |
| cargo | quote | 1.0.47 | MIT OR Apache-2.0 | 2 |
| cargo | r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | 0 |
| cargo | r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | 0 |
| cargo | raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | 3 |
| cargo | redox_syscall | 0.5.18 | MIT | 1 |
| cargo | redox_users | 0.5.2 | MIT | 1 |
| cargo | ref-cast-impl | 1.0.26 | MIT OR Apache-2.0 | 2 |
| cargo | ref-cast | 1.0.26 | MIT OR Apache-2.0 | 2 |
| cargo | regex-automata | 0.4.18 | MIT OR Apache-2.0 | 2 |
| cargo | regex-syntax | 0.8.11 | MIT OR Apache-2.0 | 2 |
| cargo | regex | 1.13.1 | MIT OR Apache-2.0 | 2 |
| cargo | reqwest | 0.13.4 | MIT OR Apache-2.0 | 2 |
| cargo | rfd | 0.16.0 | MIT | 1 |
| cargo | rhai | 1.25.1 | MIT OR Apache-2.0 | 2 |
| cargo | rhai_codegen | 3.2.0 | MIT OR Apache-2.0 | 0 |
| cargo | roots | 0.0.6 | BSD-2-Clause | 1 |
| cargo | roots | 0.0.8 | BSD-2-Clause | 1 |
| cargo | roxmltree | 0.20.0 | MIT OR Apache-2.0 | 2 |
| cargo | rustc-hash | 2.1.3 | Apache-2.0 OR MIT | 2 |
| cargo | rustc_version | 0.4.1 | MIT OR Apache-2.0 | 2 |
| cargo | rustversion | 1.0.23 | MIT OR Apache-2.0 | 2 |
| cargo | same-file | 1.0.6 | Unlicense/MIT | 2 |
| cargo | schemars | 0.8.22 | MIT | 1 |
| cargo | schemars | 0.9.0 | MIT | 1 |
| cargo | schemars | 1.2.2 | MIT | 1 |
| cargo | schemars_derive | 0.8.22 | MIT | 1 |
| cargo | scopeguard | 1.2.0 | MIT OR Apache-2.0 | 2 |
| cargo | selectors | 0.36.1 | MPL-2.0 | 0 |
| cargo | semver | 1.0.28 | MIT OR Apache-2.0 | 2 |
| cargo | serde-untagged | 0.1.9 | MIT OR Apache-2.0 | 2 |
| cargo | serde | 1.0.229 | MIT OR Apache-2.0 | 2 |
| cargo | serde_core | 1.0.229 | MIT OR Apache-2.0 | 2 |
| cargo | serde_derive | 1.0.229 | MIT OR Apache-2.0 | 2 |
| cargo | serde_derive_internals | 0.29.1 | MIT OR Apache-2.0 | 2 |
| cargo | serde_json | 1.0.151 | MIT OR Apache-2.0 | 2 |
| cargo | serde_repr | 0.1.21 | MIT OR Apache-2.0 | 2 |
| cargo | serde_spanned | 0.6.9 | MIT OR Apache-2.0 | 2 |
| cargo | serde_spanned | 1.1.1 | MIT OR Apache-2.0 | 2 |
| cargo | serde_with | 3.22.0 | MIT OR Apache-2.0 | 2 |
| cargo | serde_with_macros | 3.22.0 | MIT OR Apache-2.0 | 2 |
| cargo | serialize-to-javascript-impl | 0.1.2 | MIT OR Apache-2.0 | 2 |
| cargo | serialize-to-javascript | 0.1.2 | MIT OR Apache-2.0 | 2 |
| cargo | serialport | 4.9.0 | MPL-2.0 | 1 |
| cargo | servo_arc | 0.4.3 | MIT OR Apache-2.0 | 2 |
| cargo | sha2 | 0.10.9 | MIT OR Apache-2.0 | 2 |
| cargo | shlex | 2.0.1 | MIT OR Apache-2.0 | 2 |
| cargo | signal-hook-registry | 1.4.8 | MIT OR Apache-2.0 | 2 |
| cargo | simd-adler32 | 0.3.10 | MIT | 1 |
| cargo | simplecss | 0.2.2 | Apache-2.0 OR MIT | 2 |
| cargo | siphasher | 1.0.3 | MIT/Apache-2.0 | 1 |
| cargo | slab | 0.4.12 | MIT | 1 |
| cargo | smallvec | 1.15.2 | MIT OR Apache-2.0 | 2 |
| cargo | smartstring | 1.0.1 | MPL-2.0+ | 1 |
| cargo | socket2 | 0.6.5 | MIT OR Apache-2.0 | 2 |
| cargo | softbuffer | 0.4.8 | MIT OR Apache-2.0 | 2 |
| cargo | soup3-sys | 0.5.0 | MIT | 1 |
| cargo | soup3 | 0.5.0 | MIT | 1 |
| cargo | spin | 0.5.2 | MIT | 1 |
| cargo | stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | 2 |
| cargo | static_assertions | 1.1.0 | MIT OR Apache-2.0 | 2 |
| cargo | strict-num | 0.1.1 | MIT | 1 |
| cargo | string_cache | 0.9.0 | MIT OR Apache-2.0 | 2 |
| cargo | string_cache_codegen | 0.6.1 | MIT OR Apache-2.0 | 2 |
| cargo | strsim | 0.11.1 | MIT | 1 |
| cargo | strum | 0.27.2 | MIT | 1 |
| cargo | strum_macros | 0.27.2 | MIT | 1 |
| cargo | svgtypes | 0.15.3 | Apache-2.0 OR MIT | 2 |
| cargo | swift-rs | 1.0.7 | MIT OR Apache-2.0 | 2 |
| cargo | syn | 1.0.109 | MIT OR Apache-2.0 | 2 |
| cargo | syn | 2.0.119 | MIT OR Apache-2.0 | 2 |
| cargo | syn | 3.0.3 | MIT OR Apache-2.0 | 2 |
| cargo | sync_wrapper | 1.0.2 | Apache-2.0 | 1 |
| cargo | synstructure | 0.13.2 | MIT | 1 |
| cargo | system-deps | 6.2.2 | MIT OR Apache-2.0 | 2 |
| cargo | tao-macros | 0.1.4 | MIT OR Apache-2.0 | 3 |
| cargo | tao | 0.35.3 | Apache-2.0 | 2 |
| cargo | target-lexicon | 0.12.16 | Apache-2.0 WITH LLVM-exception | 1 |
| cargo | tauri-build | 2.6.3 | Apache-2.0 OR MIT | 2 |
| cargo | tauri-codegen | 2.6.3 | Apache-2.0 OR MIT | 2 |
| cargo | tauri-macros | 2.6.3 | Apache-2.0 OR MIT | 2 |
| cargo | tauri-plugin-dialog | 2.7.2 | Apache-2.0 OR MIT | 3 |
| cargo | tauri-plugin-fs | 2.5.1 | Apache-2.0 OR MIT | 3 |
| cargo | tauri-plugin | 2.6.3 | Apache-2.0 OR MIT | 0 |
| cargo | tauri-runtime-wry | 2.11.4 | Apache-2.0 OR MIT | 2 |
| cargo | tauri-runtime | 2.11.3 | Apache-2.0 OR MIT | 2 |
| cargo | tauri-utils | 2.9.3 | Apache-2.0 OR MIT | 2 |
| cargo | tauri-winres | 0.3.6 | MIT | 1 |
| cargo | tauri | 2.11.5 | Apache-2.0 OR MIT | 2 |
| cargo | tendril | 0.5.1 | MIT OR Apache-2.0 | 2 |
| cargo | thin-vec | 0.2.19 | MIT OR Apache-2.0 | 2 |
| cargo | thiserror-impl | 1.0.69 | MIT OR Apache-2.0 | 2 |
| cargo | thiserror-impl | 2.0.20 | MIT OR Apache-2.0 | 2 |
| cargo | thiserror | 1.0.69 | MIT OR Apache-2.0 | 2 |
| cargo | thiserror | 2.0.20 | MIT OR Apache-2.0 | 2 |
| cargo | time-core | 0.1.9 | MIT OR Apache-2.0 | 2 |
| cargo | time-macros | 0.2.32 | MIT OR Apache-2.0 | 2 |
| cargo | time | 0.3.55 | MIT OR Apache-2.0 | 2 |
| cargo | tiny-keccak | 2.0.2 | CC0-1.0 | 1 |
| cargo | tiny-skia-path | 0.11.4 | BSD-3-Clause | 1 |
| cargo | tinystr | 0.8.3 | Unicode-3.0 | 1 |
| cargo | tinyvec | 1.12.0 | Zlib OR Apache-2.0 OR MIT | 3 |
| cargo | tinyvec_macros | 0.1.1 | MIT OR Apache-2.0 OR Zlib | 3 |
| cargo | tokio-macros | 2.7.2 | MIT | 1 |
| cargo | tokio-serial | 5.5.0 | MIT | 1 |
| cargo | tokio-util | 0.7.19 | MIT | 1 |
| cargo | tokio | 1.53.1 | MIT | 1 |
| cargo | toml | 0.8.2 | MIT OR Apache-2.0 | 2 |
| cargo | toml | 0.9.12+spec-1.1.0 | MIT OR Apache-2.0 | 2 |
| cargo | toml | 1.1.4+spec-1.1.0 | MIT OR Apache-2.0 | 2 |
| cargo | toml_datetime | 0.6.3 | MIT OR Apache-2.0 | 2 |
| cargo | toml_datetime | 0.7.5+spec-1.1.0 | MIT OR Apache-2.0 | 2 |
| cargo | toml_datetime | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 | 2 |
| cargo | toml_edit | 0.19.15 | MIT OR Apache-2.0 | 2 |
| cargo | toml_edit | 0.20.2 | MIT OR Apache-2.0 | 2 |
| cargo | toml_edit | 0.25.13+spec-1.1.0 | MIT OR Apache-2.0 | 2 |
| cargo | toml_parser | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | 2 |
| cargo | toml_writer | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | 2 |
| cargo | tower-http | 0.6.11 | MIT | 1 |
| cargo | tower-layer | 0.3.3 | MIT | 1 |
| cargo | tower-service | 0.3.3 | MIT | 1 |
| cargo | tower | 0.5.3 | MIT | 1 |
| cargo | tracing-core | 0.1.36 | MIT | 1 |
| cargo | tracing | 0.1.44 | MIT | 1 |
| cargo | tray-icon | 0.24.2 | MIT OR Apache-2.0 | 3 |
| cargo | try-lock | 0.2.5 | MIT | 1 |
| cargo | typeid | 1.0.3 | MIT OR Apache-2.0 | 2 |
| cargo | typenum | 1.20.1 | MIT OR Apache-2.0 | 3 |
| cargo | unescaper | 0.1.10 | MIT OR GPL-3.0-only | 2 |
| cargo | unic-char-property | 0.9.0 | MIT/Apache-2.0 | 0 |
| cargo | unic-char-range | 0.9.0 | MIT/Apache-2.0 | 0 |
| cargo | unic-common | 0.9.0 | MIT/Apache-2.0 | 0 |
| cargo | unic-ucd-ident | 0.9.0 | MIT/Apache-2.0 | 0 |
| cargo | unic-ucd-version | 0.9.0 | MIT/Apache-2.0 | 0 |
| cargo | unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 | 3 |
| cargo | unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | 3 |
| cargo | unicode-xid | 0.2.6 | MIT OR Apache-2.0 | 3 |
| cargo | url | 2.5.8 | MIT OR Apache-2.0 | 2 |
| cargo | urlpattern | 0.3.0 | MIT | 1 |
| cargo | usvg | 0.45.1 | Apache-2.0 OR MIT | 2 |
| cargo | utf8_iter | 1.0.4 | Apache-2.0 OR MIT | 3 |
| cargo | uuid | 1.24.0 | Apache-2.0 OR MIT | 2 |
| cargo | version-compare | 0.2.1 | MIT | 1 |
| cargo | version_check | 0.9.5 | MIT/Apache-2.0 | 2 |
| cargo | visioncortex | 0.9.2 | MIT OR Apache-2.0 | 2 |
| cargo | vswhom-sys | 0.1.3 | MIT | 1 |
| cargo | vswhom | 0.1.0 | MIT | 1 |
| cargo | vtracer | 1.0.0-alpha.3 | MIT OR Apache-2.0 | 0 |
| cargo | walkdir | 2.5.0 | Unlicense/MIT | 2 |
| cargo | want | 0.3.1 | MIT | 1 |
| cargo | wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 3 |
| cargo | wasip2 | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 3 |
| cargo | wasm-bindgen-futures | 0.4.77 | MIT OR Apache-2.0 | 2 |
| cargo | wasm-bindgen-macro-support | 0.2.127 | MIT OR Apache-2.0 | 2 |
| cargo | wasm-bindgen-macro | 0.2.127 | MIT OR Apache-2.0 | 2 |
| cargo | wasm-bindgen-shared | 0.2.127 | MIT OR Apache-2.0 | 2 |
| cargo | wasm-bindgen | 0.2.127 | MIT OR Apache-2.0 | 2 |
| cargo | wasm-streams | 0.5.0 | MIT OR Apache-2.0 | 2 |
| cargo | web-sys | 0.3.104 | MIT OR Apache-2.0 | 2 |
| cargo | web-time | 1.1.0 | MIT OR Apache-2.0 | 2 |
| cargo | web_atoms | 0.2.6 | MIT OR Apache-2.0 | 2 |
| cargo | webkit2gtk-sys | 2.0.2 | MIT | 1 |
| cargo | webkit2gtk | 2.0.2 | MIT | 1 |
| cargo | webview2-com-macros | 0.8.1 | MIT | 0 |
| cargo | webview2-com-sys | 0.38.2 | MIT | 0 |
| cargo | webview2-com | 0.38.2 | MIT | 0 |
| cargo | winapi-i686-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 | 0 |
| cargo | winapi-util | 0.1.11 | Unlicense OR MIT | 2 |
| cargo | winapi-x86_64-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 | 0 |
| cargo | winapi | 0.3.9 | MIT/Apache-2.0 | 2 |
| cargo | window-vibrancy | 0.6.0 | Apache-2.0 OR MIT | 3 |
| cargo | windows-collections | 0.2.0 | MIT OR Apache-2.0 | 2 |
| cargo | windows-core | 0.61.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows-core | 0.62.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows-future | 0.2.1 | MIT OR Apache-2.0 | 2 |
| cargo | windows-implement | 0.60.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows-interface | 0.59.3 | MIT OR Apache-2.0 | 2 |
| cargo | windows-link | 0.1.3 | MIT OR Apache-2.0 | 2 |
| cargo | windows-link | 0.2.1 | MIT OR Apache-2.0 | 2 |
| cargo | windows-numerics | 0.2.0 | MIT OR Apache-2.0 | 2 |
| cargo | windows-result | 0.3.4 | MIT OR Apache-2.0 | 2 |
| cargo | windows-result | 0.4.1 | MIT OR Apache-2.0 | 2 |
| cargo | windows-strings | 0.4.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows-strings | 0.5.1 | MIT OR Apache-2.0 | 2 |
| cargo | windows-sys | 0.45.0 | MIT OR Apache-2.0 | 2 |
| cargo | windows-sys | 0.52.0 | MIT OR Apache-2.0 | 2 |
| cargo | windows-sys | 0.59.0 | MIT OR Apache-2.0 | 2 |
| cargo | windows-sys | 0.60.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows-sys | 0.61.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows-targets | 0.42.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows-targets | 0.52.6 | MIT OR Apache-2.0 | 2 |
| cargo | windows-targets | 0.53.5 | MIT OR Apache-2.0 | 2 |
| cargo | windows-threading | 0.1.0 | MIT OR Apache-2.0 | 2 |
| cargo | windows-version | 0.1.7 | MIT OR Apache-2.0 | 2 |
| cargo | windows | 0.61.3 | MIT OR Apache-2.0 | 2 |
| cargo | windows_aarch64_gnullvm | 0.42.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | 2 |
| cargo | windows_aarch64_gnullvm | 0.53.1 | MIT OR Apache-2.0 | 2 |
| cargo | windows_aarch64_msvc | 0.42.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 | 2 |
| cargo | windows_aarch64_msvc | 0.53.1 | MIT OR Apache-2.0 | 2 |
| cargo | windows_i686_gnu | 0.42.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 | 2 |
| cargo | windows_i686_gnu | 0.53.1 | MIT OR Apache-2.0 | 2 |
| cargo | windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 | 2 |
| cargo | windows_i686_gnullvm | 0.53.1 | MIT OR Apache-2.0 | 2 |
| cargo | windows_i686_msvc | 0.42.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 | 2 |
| cargo | windows_i686_msvc | 0.53.1 | MIT OR Apache-2.0 | 2 |
| cargo | windows_x86_64_gnu | 0.42.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 | 2 |
| cargo | windows_x86_64_gnu | 0.53.1 | MIT OR Apache-2.0 | 2 |
| cargo | windows_x86_64_gnullvm | 0.42.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | 2 |
| cargo | windows_x86_64_gnullvm | 0.53.1 | MIT OR Apache-2.0 | 2 |
| cargo | windows_x86_64_msvc | 0.42.2 | MIT OR Apache-2.0 | 2 |
| cargo | windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 | 2 |
| cargo | windows_x86_64_msvc | 0.53.1 | MIT OR Apache-2.0 | 2 |
| cargo | winnow | 0.5.40 | MIT | 1 |
| cargo | winnow | 0.7.15 | MIT | 1 |
| cargo | winnow | 1.0.4 | MIT | 1 |
| cargo | winreg | 0.55.0 | MIT | 1 |
| cargo | wit-bindgen | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 3 |
| cargo | writeable | 0.6.3 | Unicode-3.0 | 1 |
| cargo | wry | 0.55.1 | Apache-2.0 OR MIT | 3 |
| cargo | x11-dl | 2.21.0 | MIT | 1 |
| cargo | x11 | 2.21.0 | MIT | 1 |
| cargo | xmlwriter | 0.1.0 | MIT | 1 |
| cargo | yoke-derive | 0.8.2 | Unicode-3.0 | 1 |
| cargo | yoke | 0.8.3 | Unicode-3.0 | 1 |
| cargo | zerocopy-derive | 0.8.56 | BSD-2-Clause OR Apache-2.0 OR MIT | 3 |
| cargo | zerocopy | 0.8.56 | BSD-2-Clause OR Apache-2.0 OR MIT | 3 |
| cargo | zerofrom-derive | 0.1.7 | Unicode-3.0 | 1 |
| cargo | zerofrom | 0.1.8 | Unicode-3.0 | 1 |
| cargo | zerotrie | 0.2.4 | Unicode-3.0 | 1 |
| cargo | zerovec-derive | 0.11.3 | Unicode-3.0 | 1 |
| cargo | zerovec | 0.11.6 | Unicode-3.0 | 1 |
| cargo | zmij | 1.0.23 | MIT | 1 |
| npm | @tauri-apps/api | 2.11.1 | Apache-2.0 OR MIT | 2 |
| npm | lucide-react | 1.31.0 | ISC | 1 |
| npm | react-dom | 19.2.8 | MIT | 1 |
| npm | react | 19.2.8 | MIT | 1 |
| npm | scheduler | 0.27.0 | MIT | 1 |
| npm | three | 0.185.1 | MIT | 1 |
