# led-wire-driver

`led-wire-driver` is a `no_std` Rust crate for hardware-neutral LED output
drivers.

It separates setup validation, runtime channel writes, backend event ingress,
and wire-format packing so firmware can keep transport-specific code behind a
small backend boundary.

The crate currently targets fixed-capacity multi-channel drivers and supports
exclusive temporal and spatial packing-policy feature combinations.
