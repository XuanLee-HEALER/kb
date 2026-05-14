# Good Fact

```json
{
  "kind": "Fact",
  "title": "Kernel 5.15 xt_TPROXY sets SO_ORIGINAL_DST_ADDR4",
  "claim": "On Linux kernel 5.15 with iptables xt_TPROXY, the original dst IP after redirection is available via getsockopt(SOL_IP, SO_ORIGINAL_DST_ADDR4)",
  "evidence": [
    {
      "kind": "SelfVerification",
      "content": "Verified via strace on my-proxy 0.4.2 on 2026-04-12, confirmed against kernel source net/ipv4/netfilter/nf_tproxy_ipv4.c at v5.15",
      "captured_at": "2026-04-12T10:00:00Z"
    },
    {
      "kind": "Url",
      "content": "https://www.kernel.org/doc/Documentation/networking/tproxy.txt",
      "captured_at": "2026-04-12T10:00:00Z"
    }
  ],
  "tags": ["network/tproxy", "kernel/5.15"]
}
```

**Why it's a Fact, not something else:**

- Verified objective claim — there's a kernel function call, it returns a specific value.
- Not a Lesson — no "common misconception" being corrected.
- Not a ProblemSolution — no symptoms / no environment-specific failure.

**Why it deserves to be in the KB:**

Generic LLMs are often confused about TPROXY vs. REDIRECT and what each
exposes. The specific socket option name + kernel version is exact enough
that LLM training data may have it wrong or vague. Worth storing.
