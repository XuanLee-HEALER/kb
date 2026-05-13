# kb-skill

Claude Code skill for the personal KB delta knowledge service.

## Install

```sh
curl -fsSL https://kb.your-domain.com/skill/download | tar xz -C ~/.claude/skills/
# replace placeholders
$EDITOR ~/.claude/skills/kb-skill/mcp.json
# merge the kb section into ~/.config/claude-code/config.json
```

## Upgrade

```sh
diff <(curl -s https://kb.your-domain.com/skill/version) \
     ~/.claude/skills/kb-skill/VERSION
# re-run install if they differ
```

## Contents

- `SKILL.md` — behavioral guide (when to write, type choice, anti-patterns)
- `mcp.json` — MCP server connection template (URL + bearer token placeholders)
- `examples/` — annotated good/bad examples per kind
- `VERSION` — git short hash for upgrade detection
