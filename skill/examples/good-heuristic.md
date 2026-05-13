# Good Heuristic

```json
{
  "kind": "Heuristic",
  "title": "Network connectivity diagnosis: layered ascent",
  "problem_class": "Network connectivity broken; you don't know what layer is faulty",
  "pattern": "Ascend the stack layer by layer until something fails:\n1. Physical link: cable, NIC, carrier. `ip link show`, `ethtool`.\n2. Link layer: ARP/NDP. `ip neigh`.\n3. IP layer: reachability + routing. `ip addr`, `ip route`, ping the gateway.\n4. Transport: open the listening port from another host. `nc -zv`, `ss -tnlp`.\n5. Firewall: per-direction rules. `iptables -L -n -v`, `nft list ruleset`.\n6. App layer: protocol-specific (TLS handshake, HTTP request, etc).\n\nDon't skip layers — the lowest broken layer hides higher ones, so 'app fails' could be IP-level.",
  "limits": "Doesn't cover BGP / routing protocol issues — for those, escalate to looking at peer state and prefix advertisements directly. Also assumes single-host diagnosis; multi-host needs traceroute / mtr at step 3.",
  "tags": ["network/diagnosis"]
}
```

**Why Heuristic and not Playbook:**

The pattern is "how to *think* about the problem" — what order, what to
check, why the order matters. It's not "run these specific commands" (that
would be a Playbook, which doesn't belong in the KB).

The commands listed are *examples* of what each layer's check might look
like, not a script to follow.

**Why `problem_class` matters:**

It's the entry point for retrieval. When you next hit "connectivity broken",
this is what you'd search for. The pattern is the meat; the class is the
handle.
