# Local reproducible spike

**IMPLEMENTED / TESTED**, laboratorio; **WAN NOT TESTED**. Decisión provisional
antes de código: [decision](TRANSPORT-DECISION.md). No migración QUIC/WebRTC.

## What runs

`cine-transfer::carrier::Carrier` = Read+Write+bounded I/O configuration+graceful
close. TCP implementation and an outer Rustls client stream; the exact existing
peer TLS/grant/chunks/SHA/Partial implementation runs inside either. Existing
TCP wrapper and `receive_observed` API preserved. Peer auth observer measures
TLS+credential+ACK, not just cryptographic handshake. No parallel chunk workers.

`relay::serve_pair` finite local session accepts two OUTBOUND peer connections,
checks separate scoped one-use tickets via existing Authorization, then forwards
inner TLS ciphertext. No file cache/storage, remote dial/CONNECT, WSS binary
payloads or change in authority. Provisioning in lab is in-memory/out-of-band,
not a RoomService-integrated allocation service. Tickets are distinct from inner
grant; relay Identity is distinct from peer Identity. Buffer16KiB and TLS64KiB,
max128MiB aggregate ciphertext/300s/rate8MiB/s; default64MiB/60s/idle3s. Two
admissions only; bad admission ends session. Listener loopback or RFC1918 only.
Kernel backlog/resources add memory; this is not a public DDoS solution.

The bin provisions synthetic corpus8KiB–32MiB, certificates/grants only in
memory; file/workspace private and deleted on success/error. Default listens
loopback ephemeral ports. Netns arguments are opt-in Linux lab workers only;
other platforms compile default loopback without setns. Direct test listener
exists in sender netns; NAT gateway drops unsolicited TCP1730, with counters.
Receiver tries direct5s, then lab explicitly authorizes relay. Pause after1MiB
or raw socket shutdown; new peer grant+two new relay tickets, revalidate blocks,
transfer missing bytes and final SHA. No automatic product fallback claim.

## Reproduce

```sh
cargo build -p cine-transfer --bins --locked
cargo test -p cine-transfer -p cine-client -p cine-server --locked
# Loopback finite pair, default8MiB, pause after1MiB and new-grant resume:
target/debug/cine-wan-spike
# Small corpus; cryptographic setup dominates, not a throughput benchmark:
target/debug/cine-wan-spike 8192
# Isolated user/network namespaces; NO sudo, host links/firewall untouched:
python3 experiments/14-p2p-wan/netlab.py --launch \
  --output experiments/14-p2p-wan/results-simulated-nat.json
python3 experiments/14-p2p-wan/netlab.py --launch --netem \
  --output experiments/14-p2p-wan/results-simulated-netem.json
CINE_WAN_LAB_DISCONNECT=1 python3 experiments/14-p2p-wan/netlab.py --launch \
  --output experiments/14-p2p-wan/results-recovery.json
```

Linux needs unprivileged user namespaces, iproute2/ip/tc, nsenter/unshare, nft,
bridge/veth/NAT/netem kernel support. The launcher records parent netns and
refuses setup in that namespace or initial user namespace; children and rules
vanish when processes exit. Helper terminates four holders in finally. No host
sysctl, global firewall or router changes. Namespace addresses are invented lab
addresses, not personal/public IP evidence. Reproducible impairments specify
40ms delay±10ms jitter,0.5% random loss,10Mbit/s at EACH NAT external egress;
random loss/jitter samples are stochastic, not a claim of identical benchmarks.

Separate real LAN and existing product pipeline:

```sh
# ADB-connected ARMv7 device must already be on same WLAN. No network switching.
# Uses current sender and existing Phase1 release Android carrier.
python3 experiments/14-p2p-wan/baseline.py
# WSS/TLS/libmpv/Ready plus grant lifecycle; no Android fake presented as SDK.
python3 experiments/14-p2p-wan/product_baseline.py
```

Those scripts only overwrite experiment14 results. Private transport configs
use temp/ADB mode0600 and are deleted; no `adb reverse` or forwarded sockets.
Android loopback native relay experiment is separately recorded in
results-android-local-relay.json; build with existing NDK28.2 ARMv7 toolchain and
push only the binary. No consent UI skipped in product: shell corpus is an
explicit engineering test authorized by this task, not a user download.

## Limits / timing

Direct connect5s; peer TLS/grant5s absolute; socket idle3s unchanged. Relay admission
TLS5s per socket within10s total rendezvous, connect5s, client admission/pair10s
absolute (individual blocking read≤3s); allocation60s default/300s hard max,
idle3s, cooperative checks at I/O/pacing/accept. These are provisional bounded
lab defaults, not WAN tuning results. Cancellation owner can shutdown raw socket;
CSPRNG grant expiry/revocation checked by existing auth. `Stats` EOF means relay
session ended, never means file/SHA PASS: only receiver can verify completion.
No attempt/list/buffer growth beyond these limits. No ICE gathering/check times:
fields null because ICE is not implemented. Errors still transfer Error enum in
spike; product WAN error mapping/new diagnostics remain pending.

## Corrections discovered

Product baseline exposed old revoked grant in sender's pending slot after Host
resume. Socket now waits for available unconsumed/unexpired/unrevoked grant within
existing2s; credential TLS check still consumes it. A real socket regression test
holds new grant delivery for200ms using notification timeout; existing old code
rejects early. Physical small-payload lab also exposed an intermittent Io at
termination; inner/outer close_notify now flush before pipe drop. Exact cause of
that original Android Io was not isolated, and the FAIL is preserved. New test
checks8KiB relay termination on IPv4 and IPv6. No idle/deadline relaxation.

## Android opt-in local reproduction

Build `cargo build -p cine-transfer --bin cine-wan-spike --release --locked
--target armv7-linux-androideabi` with the pinned NDK linker environment used by
[CI](../../docs/CI.md); for ARM64 use aarch64-linux-android and its linker.
Then, with exactly one authorized ADB device:

```sh
python3 experiments/14-p2p-wan/android_local.py --binary target/armv7-linux-androideabi/release/cine-wan-spike
```

Two on-device loopback fixtures8KiB/8MiB; private binary removed afterward.
It does not switch Wi-Fi/data or bind a mobile interface. Physical ARM64 testing
requires its actual device; compilation is not a runtime claim.
