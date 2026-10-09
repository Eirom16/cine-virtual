# Remaining WAN/product gates

1. Research/local spike completed; transport choice still PROVISIONAL. Keep LAN
   native TCP/TLS and current protocol1. Do not migrate all traffic to QUIC/WebRTC.
2. Implement additive, explicitly negotiated private connectivity exchange ONLY
   after accepted transfer: room/epoch/Host/receiver/media/authority/transfer/
   manifest/generation, TTL, bounded candidates/attempts and revocation. Direct
   endpoints include real IPv4/IPv6 semantics; no pseudo srflx or ICE. Keep WAN
   IPs/relay tickets out of public v1 TransferSnapshot and nonauthorized members.
3. Add actual relay allocation service/private provisioning, global quotas,
   operator key/identity lifecycle and trusted IPC; current one-pair lab is not
   that service. Do not use peer grant as relay password. Reauthorize each route.
4. Client route state machine with direct budget, relay opt-in, typed errors,
   explicit retry/network change, verified checkpoint preservation. No auth failure
   downgrade to another endpoint or bypass. Fresh grant for fallback after an
   authenticated route consumed its grant. Mid-transfer route change needs new
   grant/relay tickets; policy for renewed consent/approval remains explicit.
5. Flutter minimal card/Developer route/timing/retry/errors only when real Rust
   state exists. Mobile consent detects metered network when available; receiver
   size/permission remains explicit. Android NetworkCallback should invalidate
   old sockets/routes/generation and request new authorization after Wi-Fi/cell
   change; no reuse of vanished interface. No ICE restart claim without ICE.
6. Operator approves host/domain/ports/private bootstrap and public WSS; configure
   self-hosting templates after security/resource review. No public service was
   deployed and no money was spent. Native manual pins remain; PKI-default trust
   and proxy termination are separately reviewed changes if chosen.
7. Physical two independent networks: both securely reach WSS,8KiB direct attempt,
   record real route/auth/SHA;8MiB only with mobile-data approval. If IPv4 listener
   unavailable behind CGNAT, use authorized relay with both connections outbound.
8. Force direct failure, validate relay peer E2E/auth/SHA/cancel/new-grant resume.
   Reproduce intermittent current LAN slowdown with controlled RF/device/build
   conditions; do not treat historical3MiB/s as an equivalent target.
9. Integrate Host offer/participant consent/progress/SHA/LocalMedia/Player/Ready,
   chat/reactions/fullscreen during real WAN. Product gate requires real SDKs.
10. Measure real WAN CPU/RAM/FDs/threads/rates/recovery/bandwidth cost and expand
    networks/platforms. ARM64 phone absent; Windows/macOS builds are not runtime,
    iOS Player remains absent. No process-death/background/swarm features implied.

External resource blocker is confirmed by user, not an inferred authorization.
The minimum next decision is an approved WSS host/domain and relay resource;
no need to replace carrier to authorize a controlled relay experiment. If direct
NAT success rates/costs justify WebRTC later, request architectural approval and
first cross-build ARMv7/ARM64 plus validate TURN TCP/TLS API and memory bounds.
