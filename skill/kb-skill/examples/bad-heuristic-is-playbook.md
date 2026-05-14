# Bad Heuristic — actually a Playbook

```json
{
  "kind": "Heuristic",
  "title": "How to deploy kb-server to ECS",
  "problem_class": "Deploying kb-server",
  "pattern": "1. ssh ecs-prod\n2. cd /opt/kb-server\n3. systemctl stop kb-server\n4. wget https://example.com/kb-server-v2.tar.gz\n5. tar xzf kb-server-v2.tar.gz\n6. systemctl start kb-server\n7. journalctl -u kb-server -f to verify",
  "tags": ["ops/kb"]
}
```

**Why this is wrong:**

This is a step-by-step recipe with specific paths and commands. It will go
stale the moment the deploy script changes, the file layout changes, or you
switch to a different ECS instance. Worse — when Claude Code retrieves this
later, it'll treat the stale steps as authoritative.

**What to do instead:**

- Keep the actual steps in a deploy script (`deploy.sh`) that's version-
  controlled with the rest of the code.
- If there's a *thinking pattern* worth capturing (e.g. "always check
  systemd unit state before assuming the binary is the problem"), that's a
  Heuristic. But the steps themselves are not.

The general rule: if you'd have to update the entry every time the env
changes, it's a Playbook. KB rejects Playbooks.
