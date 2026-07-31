# Nabaztag MTL VM: the ABI reference

Reference for writing an embedded application (`bc.jsp`) for the
Nabaztag:tag running the nabgcc firmware (uplg/nabgcc, `wpa23-gtk`).
The VM lives in the C firmware (`src/vm/vinterp.c` and friends) and is
frozen: this ABI will not move. The original Violet documentation this
distills from is preserved on [the Violet shelf](violet/README.md).

## Why hacking on bc.jsp is safe

`bc.jsp` is downloaded by the flash boot bytecode at every boot, never
flashed. A broken application means a reboot loop or a wedged VM; recovery
is fixing the file served by clapier and power-cycling the rabbit. The
config mode (head button held at boot) lives in flash and stays untouched.

## Toolchain

The Metal compiler and simulator are vendored in `vendor/metal/` and
run inside the `mtl-dev` Docker image, built on demand.
`garenne/build.sh` drives everything: `test` runs the golden suite in
the simulator, `sim` runs the application interactively (LEDs as ANSI
truecolor, ears as numbers), no argument produces the device build.

## The Metal language (VLISP / Sylvain Huet, 2005-2006)

ML-flavored, compiled to VM bytecode. Learned from the SN sources:

```
proto main 0;;                 // forward declaration, arity
var _leds_net_activity = 0;;   // global
const LEDS_OSC = { 0 1 2 ... };;  // table constant, dot-indexed: LEDS_OSC.x
fun _leds_osc x =
    let (x>>6)&3 -> q in       // let expr -> name in body
    if q==0 then LEDS_OSC.x
    else ...;;
```

- Statements end with `;;`. Comments `//` and `/* */`.
- C-style preprocessor (perl): `#include`, `#define`, `#ifdef`.
- Strings, lists (`hd`/`tl`), tables; GC in the VM (`gc` opcode).
- Protos live in `firmware/protos/*_protos.mtl`, one per module.

## VM natives (the hardware ABI, from compiler/vbc_str.h, 152 opcodes)

Beyond the usual VM core (arith, strings, tables, control flow):

| Domain   | Natives |
|----------|---------|
| LEDs     | `led` |
| Ears     | `motorset`, `motorget` |
| Buttons  | `button2`, `button3` |
| Audio out| `playStart`, `playFeed`, `playStop`, `playTime`, `sndVol`, `sndRefresh`, `sndWrite`, `sndRead`, `sndFeed`, `sndAmpli` |
| Audio in | `recStart`, `recStop`, `recVol`, `adp2wav`, `wav2adp`, `alaw2wav`, `wav2alaw` |
| Radio    | `netCb`, `netSend`, `netState`, `netMac`, `netChk`, `netSetmode`, `netScan`, `netAuth`, `netSeqAdd`, `netRssi`, `netPmk` |
| Sockets  | NOT IMPLEMENTED ON DEVICE, see below |
| Storage  | `envget`, `envset` (config sector), `load`, `save`, `bytecode` |
| RFID     | `rfidGet`, `rfidGetList`, `rfidRead`, `rfidWrite` |
| I2C      | `i2cRead`, `i2cWrite` |
| System   | `time`, `time_ms`, `loopcb`, `reboot`, `gc`, `corePP`, `corePush`, `corePull`, `coreBit0`, `crypt`, `uncrypt` |
| DANGER   | `flashFirmware`: the bytecode can reflash the C firmware. Never expose it in our application. |

## The network reality (verified in our own C firmware, vendor/nabgcc)

`grep 'case OPtcp|case OPudp' src/vm/vinterp.c` returns **zero**. The device
implements only raw 802.11 data frames plus radio control:

```
netCb #handler            handler is called as: fun handler frame macsrc
                          (both MTL strings; frame is the received payload)
netSend buf index len macdst indmac speed    -> int, sends buf[index..index+len]
netChk buf index len seed -> int             one's-complement checksum (IP/TCP/UDP)
netMac                    -> 6-byte string   our MAC
netState                  -> int             rt2501 link state
netRssi                   -> int             average RSSI
netScan ssid              -> list of tables  each: [ssid mac bssid rssi chn rateset encryption]
netAuth scan mac authmode encrypt key        associates (key = 32-byte PMK)
netPmk ssid passphrase    -> 32-byte string  PBKDF2, done in C (slow in MTL)
netSetmode mode ssid chn                     station / master (AP) mode
netSeqAdd seq n           -> 4-byte string   32-bit big-endian add, for TCP seq math
```

Consequences, embodied by garenne:

1. **The whole IP stack lives in MTL**: Ethernet-ish framing over the
   rt2501, ARP, IPv4, ICMP, UDP, DHCP, DNS, TCP. Garenne implements all
   of it; nothing below the radio natives comes from the firmware.
2. **The simulator cannot test it.** `mtl_simu` (linux_simunet.c) offers
   BSD-socket natives `tcpOpen/tcpListen/tcpSend/udpSend` and calls back on
   `SYS_CBTCP`; it does **not** emulate raw frames. So a simulator build
   must bind garenne's socket API to those natives, while the device build
   binds it to our own stack. One narrow seam, two backends: keep the
   application above `sock_*` identical, swap the layer below with `#ifdef
   SIMU`. That seam is the only place the two builds may differ.
3. `netChk` and `netSeqAdd` exist precisely because checksums and 32-bit
   sequence arithmetic are painful in the VM. Use them.

## Metal, learned the hard way

- The compiler is a real ML type checker with unification. `S cannot be
  unified with fun` means an argument order mismatch, not a syntax error.
- A `proto` is an arity declaration only, and every proto that reaches the
  compiler needs a body: pulling in one module drags its whole dependency
  cone (their LED module knows about audio, their scheduler formats JSON,
  their JSON depends on Forth). That is why garenne imports nothing.
  `preproc_remove_extra_protos.py` only strips duplicate protos.
- Records: `[field:value field2:value2]`, read `x.field`, write
  `set x.field = v`. Indirect call `call f [arg1 arg2]`, function
  reference `#name`.
- `match x with (Ctor -> e) | (Ctor arg -> e) | (_ -> e)`; sum types via
  `type T = A | B _ | C;;`.
- Loops: `for l = list; l != nil; tl l do body`. Lists: `hd`, `tl`, `::`.
- Statements end with `;;`. `let value -> name in body` binds.
- Strings are byte buffers (`strnew`, `strget`, `strset`, `strsub`,
  `strcat`, `strcatlist`): the natural packet buffer type.
- A page built as a `::` list of string pieces must end with `::nil`,
  or the type checker unifies the last string as the list's tail and
  refuses the lot with a message that points everywhere but there.
- `ifdef X` is satisfied by a `var X;;` anywhere in the source, and
  ifdefs nest fine, including inside `ifdef BOOT`.
