# Connectivity / transport comparison

D=DOCUMENTED upstream; I=INFERRED integration; M=MEASURED here; NT=NOT TESTED.
Platform support describes candidates, **not** unperformed Cine Virtual builds.
Versions and activity in [research](dependency-research.json); choice and sources
in [decision](TRANSPORT-DECISION.md). No alternative benchmarks invented.

| Criterion | A: TCP/TLS | B: ICE/WebRTC DataChannels | C: Quinn + external traversal | D: TCP/TLS + byte relay |
| --- | --- | --- | --- | --- |
| LAN | Existing physical Linux↔ARMv7 PASS | D; Cine NT | D; Cine NT | Existing path preserved; physical small PASS |
| WAN direct | Reachable listener only | ICE pairs if viable | UDP reachability + separate traversal | Same limits as A; product WAN NT |
| Symmetric NAT | No general traversal | Direct may fail; TURN | External checks/relay required | Outbound relay expected viable (I); NAT simulation M |
| CGNAT | Home forwarding insufficient | TURN may be required | External traversal/relay | Outbound relay, real CGNAT NT |
| IPv4 | Kernel TCP; product restricts private | D host/srflx/relay; library caveats | D IPv4 UDP | Kernel TCP, public product exchange pending |
| IPv6 | Socket supports; v1 offer rejects | D; gather bugs/actual target NT | D; firewall still applies | Loopback v6 candidate; physical global IPv6 NT |
| Restrictive firewalls | Reachability varies | TURN TCP/TLS only if library implements it | UDP blocked means fallback needed | TLS TCP may work; HTTP-only proxy may still block |
| UDP blocked | Does not require UDP | Inspected webrtc TURN TCP/TLS not demonstrated | Quinn unavailable without another carrier | TCP/TLS tunnel does not require UDP |
| Relay | Additional tunnel required | TURN infrastructure | Separate relay/library e.g. Iroh | Local finite one-pair implementation; public service pending |
| Encryption | TLS1.3 peer-peer | DTLS peer-peer through TURN | QUIC TLS1.3 | Inner peer TLS1.3; outer TLS admission |
| Authentication | Existing WSS pin + bearer grant | Authenticated DTLS fingerprint + application grant | Pinned certificate/peer identity + grant | Same peer authentication + separate relay tickets |
| MITM | Pin/SAN/secret; real negative tests | Fingerprint MUST be WSS authenticated | Trust policy needed; never insecure verifier | Wrong outer/inner pin negative tests; relay has no inner key |
| ARMv7 | Phase1 physical; new compile/runtime evidence separate | Ring plausible I; not cross-built here | Ring plausible I; not cross-built here | No new dependencies/minimum; new ARMv7 lab evidence separate |
| ARM64 | Existing CI; no new physical phone here | I; NT | I; NT | Final CI build; physical NT |
| Windows | Existing native hosted tests/build | Candidate rust/runtime, NT | D platform, Cine NT | Hosted build/tests; product relay runtime NT |
| macOS Intel | Existing hosted build/tests | Candidate NT | D platform, Cine NT | Hosted build/tests; Player runtime NT |
| macOS ARM64 | Existing hosted build/tests | Candidate NT | D platform, Cine NT | Hosted build/tests; Player runtime NT |
| Linux | Existing runtime | Tokio/Sans-I/O candidate | Tokio runtime available | Local TLS/NAT/netem runtime M |
| Android | SAF/native Rust already | Native Rust viable I; no Kotlin ICE authority | Native Rust viable I; interface change lifecycle | Existing SAF/Media3 preserved; no background service |
| Future iOS | Rust API exists; Player absent | Upstream libdatachannel supports iOS; Rust stack NT | Rust targets plausible I; NT | Build gate only; Player NOT IMPLEMENTED |
| CPU | Phase1 data; current physical time measured | NOT MEASURED | NOT MEASURED | Lab CPU all peers+relay, not isolated per-device cost |
| RAM | Bounded app buffers; RSS is larger | NOT MEASURED; configure windows/queues | NOT MEASURED; flow windows matter | 16KiB splice + TLS/kernel; total process samples M |
| Throughput | Phase1 ~3MiB/s LAN under its conditions | NOT MEASURED | NOT MEASURED | Local/NAT/netem measurements; WAN NT |
| Binary size | Existing binary | Increment NOT MEASURED | Increment NOT MEASURED | No new crate; ARMv7 executable size recorded separately |
| Congestion | Kernel TCP | SCTP implementation | QUIC implementation | Kernel TCP on both outbound legs |
| Native dependencies | Ring C + NDK existing | webrtc ring C; str0m default AWS-LC; libdatachannel C++/CMake/SCTP | Ring option; provider dependent | Same existing ring C; no new native stack |
| Licenses | Rustls Apache2/ISC/MIT, ring/rcgen retained | webrtc MIT/Apache; str0m MIT/Apache; wrapper/lib MPL2 | Quinn MIT/Apache; Iroh MIT/Apache | New Cine code shares pending project license; retained deps |
| Runtime | Blocking owned worker | webrtc0.21 Tokio/smol; str0m Sans-I/O driver needed | Quinn Tokio/smol/async-std | Existing worker + lab relay owner |
| Threads | Existing owners | NOT MEASURED; reactive pool configurable | NOT MEASURED | Lab observed count, not product per-peer claim |
| Complexity | Lowest; limited direct reachability | ICE+DTLS+SCTP+SDP+fragmentation | QUIC+traversal+relay | One stream interface; custom relay operation/security remains |
| Maintenance | Existing carrier/security | Recent API rewrite and transport edge cases | Mature carrier; integration still substantial | Small addition; custom relay requires audit before public service |
| Self-hosting | Public WSS and reachable listener | Public WSS + STUN/TURN | Public WSS + selected relay/traversal | Public WSS + separate authenticated byte relay |
| Operational cost | Signaling/control; host upload | TURN traffic when direct fails | Relay usage depends on external stack | Higher relay incidence expected; egress quota mandatory |
| Backpressure | One requested chunk | BufferedAmount/send windows MUST be bounded | Stream flow control | Same chunks; blocking bounded 16KiB splice |
| Recovery | Fresh grant + in-memory checkpoints | ICE restart + application reauthorization | Migration/reconnect + application reauthorization | Tested socket shutdown + missing blocks/new grants in lab |
| Old clients | p2p_transfer_v1 | New capability/adapter required | New capability/adapter required | V1 untouched; product relay capability not implemented |
| Direct vs relay status | Today LAN only | Selected ICE pair actual candidate type | Actual path from carrier/relay | Lab records RELAYED; no false UI direct claim |
| Privacy | V1 offer IP visible to all room opt-in | Candidates private after permission required | Endpoints/lookup private policy required | New WAN exchange MUST be private; product exchange pending |
| NAT success evidence | Physical same LAN only | Internet traversal NOT TESTED here | NOT TESTED here | Two isolated NATs with counters; not real CGNAT |

## Networks actually available

| Network | Direct | Relay | Evidence |
| --- | --- | --- | --- |
| Same physical Wi-Fi Linux→Android9 ARMv7 | PASS TLS/chunks/SHA | NT | results-lan.json |
| Loopback product WSS/libmpv/Ready/cancel/resume | PASS | NT | results-product-lan-loopback.json |
| Two rootless namespaces behind two NATs | Expected direct timeout | LOCAL_SIMULATION_PASS | results-simulated-nat.json |
| Same simulation + netem | Expected direct timeout | LOCAL_SIMULATION_PASS | results-simulated-netem.json |
| Different homes | NOT TESTED | NOT TESTED | No public WSS/relay authorized |
| Home↔mobile / mobile↔mobile | NOT TESTED | NOT TESTED | No mobile network changed or mobile payload sent |
| Global IPv6 / real CGNAT / operator symmetric NAT | NOT TESTED | NOT TESTED | No physical reachability evidence |
| UDP restricted physical | NOT TESTED | NOT TESTED | TCP lab does not establish firewall policy success |
