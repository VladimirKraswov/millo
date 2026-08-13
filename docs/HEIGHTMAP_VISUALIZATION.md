# Heightmap visualization

The heightmap preview separates measurement state from camera state. New probe
samples rebuild the measured points, labels, and interpolation mesh, but keep
the operator's current orbit, pan, and zoom while the probe area and view mode
remain unchanged.

The camera is reframed only when the measured perimeter changes or the operator
switches between Top and 3D views. This keeps incoming points visible without
interrupting inspection of a specific part of the surface.
