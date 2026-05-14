# Good ProblemSolution

```json
{
  "kind": "ProblemSolution",
  "title": "WireGuard PMTU black hole on kernel 5.x via PPPoE",
  "problem": "TCP traffic over WireGuard stalls after ~3 RTTs when the path includes a PPPoE link with non-1500 MTU",
  "environment": "Linux 5.15, wireguard-tools 1.0.20210914, PPPoE upstream (1492 MTU), no MSS clamping configured",
  "symptoms": [
    "Small TCP requests succeed",
    "Large requests hang after the first few packets",
    "tcpdump shows ICMP fragmentation-needed being dropped by upstream"
  ],
  "root_cause": "PPPoE MTU of 1492 plus WireGuard 60-byte overhead exceeds 1432; without MSS clamping or PMTU discovery cooperation, TCP picks a too-large segment size and the packets are silently dropped by the PPPoE peer",
  "solution": "Add `PostUp = iptables -t mangle -A FORWARD -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu` to the WireGuard config",
  "verification": "After the rule is added, large HTTP responses complete and tcpdump no longer shows fragmentation-needed",
  "tags": ["network/wireguard", "network/mtu"]
}
```

**Why ProblemSolution and not Lesson:**

The problem is environment-bound (kernel + PPPoE + WireGuard combination).
Lessons are about thinking traps, not specific diagnoses.

**Why all fields matter:**

- `environment` distinguishes this from cases where MSS clamping doesn't
  help (e.g. UDP-heavy traffic, or non-PPPoE links).
- `root_cause` is the *why*, not "MSS clamping fixes it" — that's the *what*.
- `verification` is what convinced you. Without it, this is a guess.
