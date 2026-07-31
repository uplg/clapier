# The Violet shelf

Original documentation and sources of the Nabaztag:tag and its Metal
language, mirrored here so they outlive any single website. Everything
in this folder is the work of [Sylvain Huet](https://www.sylvain-huet.com)
(Metal, the VM, the boot and nominal bytecodes) and the Violet team;
the originals live on his site, notably the
[Nabaztag:tag section](https://www.sylvain-huet.com/#nabv2). Mirrored
for preservation, with gratitude.

## metal/

- [`doc/Metal.html`](metal/doc/Metal.html) - the original Metal
  reference, 676 pages, one per function of the language and its
  runtime. Open it locally; it is self-contained.
- [`DT_metal_03_01_13_grammaire.pdf`](metal/DT_metal_03_01_13_grammaire.pdf) -
  the language grammar specification (2003).
- [`DT_metal_02_12_18_gc.pdf`](metal/DT_metal_02_12_18_gc.pdf) - the
  garbage collector design note (2002).
- [`examples/`](metal/examples/) - `tetris.txt` and `queen.txt`, small
  Metal programs from the author.

## tagtag/

- [`DT_violet4_lisp-wifi-driver_revE.pdf`](tagtag/DT_violet4_lisp-wifi-driver_revE.pdf) -
  the WiFi driver documentation (Sébastien Bourdeauducq): how the
  rt2501 is driven and what the `net*` natives really do.
- [`SY_nabaztagtagvm_native.xls`](tagtag/SY_nabaztagtagvm_native.xls) -
  the VM native instruction table.
- [`boot.0.0.0.11.txt`](tagtag/boot.0.0.0.11.txt) - the original boot
  bytecode source (WiFi association, config portal, firmware upload);
  the ancestor of the boot our firmware ships.
- [`nominal.010115.txt`](tagtag/nominal.010115.txt) - the original
  nominal application: what a Violet rabbit actually ran.

## Where this meets the present

Our working notes distilled from these and from the firmware sources
live next door in [`nabaztag-mtl-abi.md`](../nabaztag-mtl-abi.md); the
modern application built on that ABI is [`garenne/`](../../garenne/),
and the toolchain that still compiles all of it is vendored in
[`vendor/metal/`](../../vendor/metal/).
