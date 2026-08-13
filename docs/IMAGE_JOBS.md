# Image-generated jobs

Millo treats image conversion as an application-core service. Plugins may
provide different workflows, presets, nesting, or batch tools, but they do not
implement their own string-based sender path.

## Pipeline

```text
SVG ---------------------> usvg typed tree --+
                                               +-> normalized paths -> Millo CAM -> G-code
PNG -> luminance/threshold -> VTracer -> SVG --+                         |
                                                                         +-> millo-gcode reparse
```

- PNG input is decoded with strict `4096 x 4096`, 8-megapixel, allocation, and
  8 MB source limits. Alpha is composited against white before thresholding.
- VTracer runs its binary spline tracer. The operator can tune brightness
  threshold, inversion, and speckle filtering; curve topology remains an SVG
  intermediate that the UI previews.
- The VTracer SVG intermediate is capped at 8 MB before parsing, and aggregate
  geometry is stopped while collecting at 120,000 points rather than checked
  only after an unbounded allocation.
- `usvg` resolves shapes, transforms, and SVG path data. Quadratic and cubic
  segments are flattened against a tolerance expressed in physical
  millimetres, not source pixels.
- Paths are normalized to the requested width while preserving aspect ratio and
  converting screen-down Y into CNC-up Y.
- Millo emits explicit `G21 G90 G94 G17`, `M5/M9`, Safe Z, bounded plunge/feed,
  and a final shutdown. It does not emit `M3`, `M4`, probing, coolant-on, work
  offset mutation, or machine-coordinate motion.
- The generated source must pass the same `millo-gcode` parser before it can be
  returned. Parser limits therefore remain the final complexity boundary.

## Plugin contract

`jobs.create` exposes these host-owned operations:

1. `generateImage(request)` calls the Rust core and returns a deeply frozen job.
2. `save(job)` opens the native `.nc` save dialog after a fresh parser check.
3. `open(job)` publishes that exact core-issued object to Program workspace.

The same capability also exposes the typed `generateSurfacing(request)` core
operation. It resolves `toolId` from the Rust-owned library; plugins cannot pass
untrusted cutter geometry. See [Surfacing](SURFACING.md).

The host retains job identity in a `WeakSet`; plugins cannot fabricate a source
object and pass it to save/open. Unloading a plugin closes retained proxies.
Opening publishes only a program. GRBL Check, physical preflight, one-use run
authorization, recovery persistence, and the sender are unchanged and remain
unavailable through this capability.

The first bundled plugin is `io.millo.image-to-gcode`. It is linked into the
application and explicitly granted `ui.contribute` plus `jobs.create` at the
composition root. External-code loading, trust, signatures, and per-user grant
persistence remain future work.

## Operator access

The bundled plugin is enabled by default and opens from the top bar through
`Create -> Engraving from image` (`Создать -> Гравировка по изображению`). It
stays in this compact job-creation menu instead of adding another permanent
workspace button. React development StrictMode is covered by a deferred host
disposal lifecycle, so its temporary effect cleanup cannot unload bundled
plugins while the application is still mounted.
