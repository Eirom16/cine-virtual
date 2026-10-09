# Threat model and verified controls

Status IMPLEMENTED/TESTED in local spike; public security audit NOT PERFORMED.
Future allocation/private signaling requirements in [self-hosting](SELF-HOSTING.md).

| Threat | Control | Actual evidence / limit |
| --- | --- | --- |
| Unauthenticated relay use | Two preprovisioned tickets, CSPRNG256bit, existing scoped constant-time auth | Wrong secret/epoch negative real outer TLS tests; no public provisioning API |
| Reuse/replay/stale allocation | Independent grant IDs, one consumption, expiry | Old ticket/new verifier and duplicate consume tests; full candidate generation signaling pending |
| Wrong relay/MITM | Pinned outer certificate, SAN cine-relay.local | Wrong outer certificate test; endpoint allocation→relay binding via WSS pending |
| Relay impersonates Host | Separate pinned inner TLS1.3 SAN cine-transfer.local | Impostor inner certificate over admitted relay rejected; zero verified bytes |
| Relay decrypts movie | Relay has outer key only; inner Host key never provisioned to relay | Real nested TLS implementation, not double hop TLS without inner encryption |
| Wrong room/member/media/host | Existing Credential scope + manifest fingerprint including Host/media/authority | Phase1 room/codec/credential tests retained; production relay private exchange not implemented |
| Revoked/expired grant | Authorization checked at I/O; lifecycle socket shutdown | Tests before admission; live grant revoke/disconnect existing product baseline |
| Old revoked grant wins WSS delivery race | Wait only for available grant; fresh TLS consume still mandatory | New real-socket test and product resume baseline |
| Oversized admission / memory abuse | Length≤4096 before allocation,16KiB splice,TLS64KiB,2 admissions | Real outer TLS oversize test and byte budget before forward |
| Unlimited resource abuse | Hard local byte/time/rate limits and finite pair | Quota/invalid policy tests; global/origin production quotas NOT IMPLEMENTED |
| Slow handshake / cancellation | Absolute TLS/admission deadlines; observed socket shutdown | Existing inner trickle test, outer cancellation test; outer slowloris public load NOT TESTED |
| Corrupt/truncated/injected payload | Inner TLS then strict chunk/hash/final SHA | Existing malformed headers, pin, corruption and checkpoint tests; no insecure verifier |
| Wrong media on resume | Same manifest/identity; missing blocks revalidated, new grants | Local NAT pause/raw-disconnect and product revision invalidation |
| Endpoint scanning / SSRF | Relay never dials caller-provided target; only pairs accepted sockets | No arbitrary CONNECT; future candidate endpoint validation/private routing pending |
| Peer IP disclosure | No IP/config/cert/token in bin/progress exports | V1 offer privacy weakness remains; no new public WAN candidate exchange added |
| Data/mobile consent | No automatic product relay/data download | Mobile network untouched; public/mobile physical tests NOT TESTED |
| Partial leaks to Player | Existing Partial.commit/SHA/authorization/load/Ready | Real libmpv baseline; no progressive media/load from partial |

Relay can observe outer ticket scopes, client source IPs, lengths/timing and deny
service; secrecy of contents is not anonymity. It may alter ciphertext: inner
TLS must reject it. No auditor-grade active manipulation suite or DDoS proof is
claimed from the certificate substitution test. Authorized recipients can keep
received data. Room signaling server is trusted for identity/grants/pins.

Outer TLS protects relay admission credentials in transit. Inner bearer grant
never travels in clear outer application data; only encrypted peer TLS records
are forwarded. All grants/key material remain memory/private temp; no Debug on
Credential, no key output. Policy file contains no credentials. No TLS1.2,
0-RTT, permissive validation, tickets/resumption or untrusted-WS grants.
