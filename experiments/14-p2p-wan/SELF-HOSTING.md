# Self-hosting preparation — no deployment authorized or executed

**IMPLEMENTED**: native WSS --advertise and optional persistent DER identity.
**PROPOSED**: operator templates and future relay service policy. The local relay
is one finite pair, not a public daemon. No hidden STUN/TURN defaults, DNS change,
router/firewall change, deployment, paid account or external service was created.
The user confirmed no public endpoint/relay currently exists.

## Required resources and explicit decisions

One operator-controlled Linux host with public IPv4 or reachable global IPv6;
TCP egress from both clients. CGNAT on the clients is compatible in principle
with outbound relay sockets; real reachability still must be tested. A home
server behind operator CGNAT needs an authorized public host/tunnel; home port
forwarding alone cannot expose it. VPN/hotspot success is not a WAN product gate.

A real domain (e.g. rooms.your-domain and relay.your-domain, **placeholders**, no
DNS was configured), A/AAAA pointing to authorized host, certificates with exact
SANs, correct clocks and renewal policy. For initial controlled WAN experiment,
manual pinned native WSS is supported; obtain the public DER pin over a trusted
channel, verify independently before sharing invitations. Development identity
is NOT automatically trusted. Public PKI without a pin remains NOT IMPLEMENTED
in Client and requires trust-policy review, not disabling validation.

## Persistent WSS certificate preparation

These are operator instructions, not commands executed on infrastructure here.
Create a dedicated non-login service user and root-owned private directory;
keep key readable only by its service, certificate/public pin shareable. Example
for an explicitly pinned self-signed identity with a real configured DNS SAN:

```sh
umask 077
# Replace example.invalid BEFORE provisioning; private output stays outside Git.
install -d -m 0700 /etc/cine
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes \
  -keyout /etc/cine/room-key.pem -out /etc/cine/room-cert.pem -days 30 \
  -subj /CN=rooms.example.invalid \
  -addext subjectAltName=DNS:rooms.example.invalid
openssl x509 -in /etc/cine/room-cert.pem -outform DER -out /etc/cine/room-cert.der
openssl pkcs8 -topk8 -nocrypt -in /etc/cine/room-key.pem -outform DER \
  -out /etc/cine/room-key.pkcs8.der
chmod 0600 /etc/cine/room-key.pem /etc/cine/room-key.pkcs8.der
```

Set owner/group to the dedicated service without making keys world-readable.
Input certificate ≤2048bytes, PKCS8 key ≤8192bytes. Wrong key fails closed;
wrong/expired SAN fails at Client. A persisted identity keeps pin stable across
restart, but rooms/epochs/resume tokens still vanish on restart (in-memory store).
Renewal replaces the pin; distribute new pin by authenticated channel, never TOFU.
The tested preparation uses an explicitly pinned self-signed identity. A CA-issued
leaf can be loaded by the server, but client validation of a public CA chain PLUS
pin has NOT BEEN TESTED; do not assume a leaf placed in RootCertStore supplies
its issuer chain. Full WebPKI roots/chains plus explicit pin checking require
separate implementation/review. Never weaken TLS validation to make it connect.

Start native TLS on loopback behind an approved TCP pass-through ingress:

```sh
cine-server --bind 127.0.0.1:1729 --tls \
  --advertise wss://rooms.example.invalid:443/ \
  --tls-cert /etc/cine/room-cert.der --tls-key /etc/cine/room-key.pkcs8.der
```

The printed #tls is a public certificate, not a secret. Only operator/private
bootstrap should receive the full invitation/resume/grant. Startup errors redact
identity filenames. --advertise selects DNS SAN for generated ephemeral identity;
when loading DER, operator must provision matching SAN. Bind and advertised port
can differ. Neither flag discovers NAT or creates a reachable endpoint.

[service template](self-hosting/cine-room.service.example) and
[HAProxy TCP passthrough template](self-hosting/haproxy.cfg.example) are reviewable
preparation. The proxy **does not terminate TLS**: native serve_tls still sets
Server.secure, so P2P negotiation remains authenticated. An ordinary HTTPS proxy
forwarding plain WS to Server.serve would disable P2P; never falsely mark plain
upstream secure by trusting arbitrary forwarded headers. Public TLS-termination
support requires a separate authenticated trust boundary and review.

## Ports and hardening before public exposure

Only WSS443/TCP at ingress; loopback1729 control backend. Admin access restricted
to operator policy; never expose key files, console, metrics or provisioning API.
Exact public port/domain/firewall/VM/DNS changes require user approval. Proxy
limits IP connections/rates; native app maintains session/message/room bounds.
Review origin policy if allowing browser clients; native clients have no browser
Origin trust. Establish create/join quotas per source and TLS handshake load
protection before general public use. Current TlsListener performs bounded TLS
accept before semaphore for WS sessions; public handshake flooding/DoS resilience
has not been measured. Service resource caps are proposals, not measured capacity.
Log retention minimal, no frame bodies, tokens, addresses or media descriptors;
restrict access to any operational source-IP data and make retention explicit.

## Relay service preparation

A separate data service, possibly on the same authorized host, with its own key,
TLS identity, private provisioning channel, and public TCP443 on another IP or
TCP4443 if clients permit. Same port443/IP needs a reviewed multiplexing/ingress
scheme; the lab has no ALPN or SNI router. No relay listener should receive chunks
via RoomService/WSS JSON. No transparent arbitrary-host CONNECT proxy.

[policy proposal](self-hosting/relay-policy.proposed.json) is NOT an executable
config. Missing implementation: RoomService-approved allocation API/IPC, private
v2 capability/route tickets, global/per-source quotas and daemon lifecycle.
Ticket roles get independent CSPRNG256bit secrets; bind all listed scopes, TTL,
generation and exact relay allocation/certificate. The old grant's secret stays
inside peer TLS, never goes to relay. Revoke on host/member/media/epoch changes.
Relay pairs two outbound sockets, forwards opaque TLS records, bounded buffers,
idle/duration/rate/global egress limits; no persistent bytes/cache. The local
spike restricts listeners to loopback/RFC1918, max128MiB, max300s, exactly two
admissions, 16KiB splice and max8MiB/s. It is not a public multiroom service.

Lab pause or network failure needs fresh relay tickets AND fresh peer grant,
revalidates checkpoint, reconnects both peers and requests only missing blocks.
The real product still needs explicit Host approval/resume; automatic fallback
and auto-reauthorization NOT IMPLEMENTED. Current peer grants expire after10min
throughout the connection: slow/large files may need repeated explicit grants.
Do not raise duration/quota or bypass expiry to finish a benchmark silently.

If choosing WebRTC later, coturn is the standard TURN option: TLS transport to
TURN does not itself provide E2E, DTLS peer↔peer does. Coturn config requires
realm, authenticated ephemeral users, allocation/user/global quotas, rate limit,
restricted relay port range, public advertised relay address, certificate and
forbidden peer address ranges. TURN/TCP/TLS must be supported by actual client
library; inspected webrtc versions do not prove it. No coturn was installed or
activated here. TCP relay is selected for this spike, not TURN protocol emulation.

## Costs and first authorized external test

No vendor selected, quote obtained or price invented. Size N usually produces
~N inbound+~N outbound at relay, plus TLS/TCP/control overhead, retries and
acknowledgments. Billing may count egress only or aggregate; review the chosen
provider's written tariff/quotas. Also account for VM, IPv4, domain renewal,
monitoring and operator updates. Enforce finite daily egress, pairs per room,
maximum file/allocation duration and user-visible consent before enabling relay.
No data-heavy mobile test without explicit approval; first real WAN corpus8KiB,
then8MiB only after reachability/authentication and mobile consent are confirmed.

Next resource request: approved host/domain and operator-controlled WSS ingress;
then approve implementing/provisioning the separate finite relay service. Verify
both devices reach pinned WSS from independent networks before file tests.
Record connection type/topology privately; publish only route types and redacted
metrics. Prove direct/relay/SHA/Ready/Player in real networks before claiming WAN.

## Template validation status

Startup/config/persisted identity tests PASS locally. HAProxy binary is absent:
its template syntax NOT TESTED here; run `haproxy -c -f` against the reviewed
config on the authorized host before enabling it. systemd template NOT INSTALLED.
No public provisioning, deployment or certificate trust transition performed.
