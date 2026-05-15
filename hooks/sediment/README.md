# sediment hook

Claude Code 的 `PreCompact` / `SessionEnd` 钩子 — session 即将压缩或退出前,
扫一遍 transcript JSONL,挑出可对应 KB 五种 entry kind 之一的瞬间,
POST 到 `https://kb.lan/api/candidates` 进未蒸馏候选池。

## 工作流

```
PreCompact / SessionEnd ──> sediment.sh ──> kb /api/candidates
                                │
                                ├── stage 1: bash + jq 稀疏化 transcript(thinking / WebSearch /
                                │            WebFetch / ExitPlanMode / 长 text / tool_result 含 error)
                                │
                                ├── stage 2: claude -p --model haiku 语义筛 + kind 标注
                                │            haiku 输出 [{kind_hint, start_uuid, end_uuid, why}, ...]
                                │
                                └── stage 3: 按 uuid 锚抽 transcript 原文 → POST 每条 candidate
```

错误处理:**全静默,永远 exit 0**。任何环节失败(transcript 不存在 / token 拿不到 /
haiku 输出非合法 JSON / 网络挂)都只是这一次 fire 不入池,绝不阻断 compact 或 exit。

## 依赖

- `jq` ≥ 1.6(macOS `brew install jq`)
- `curl`
- `claude` CLI(Claude Code)— 用于 stage 2 子进程
- `~/.claude.json` 里有 `mcpServers.kb.headers.Authorization`(`claude mcp add` 后会有)
- 网络可达 `https://kb.lan`(etmesh 内)

## 安装

跑 `https://kb.lan/skill/install`(`curl ... | jq` 或让 Claude Code 自己执行)— plan 里
有完整步骤:下载 `sediment.sh` 到 `~/.claude/hooks/sediment.sh`,paste `settings.snippet.json`
进 `~/.claude/settings.json` 的 `.hooks` 下。

## 手动测试(写完 hook 之后必跑一次)

```bash
# 选一个真实的 transcript jsonl(随便挑一个长 session 的)
TR=$(ls -t ~/.claude/projects/*/*.jsonl | head -1)

# 模拟 hook 调用
echo "{\"transcript_path\":\"$TR\",\"session_id\":\"test\",\"cwd\":\"$PWD\",\"hook_event_name\":\"PreCompact\"}" \
  | bash ~/.claude/hooks/sediment.sh

# 看日志
ls -lt ~/.cache/kb-sediment/ | head -5
cat ~/.cache/kb-sediment/$(ls -t ~/.cache/kb-sediment/ | head -1)

# 看池子(本机能否访问 kb.lan)
TOKEN=$(jq -r '.mcpServers.kb.headers.Authorization' ~/.claude.json | sed 's/^Bearer //')
curl -sS -H "Authorization: Bearer $TOKEN" https://kb.lan/api/candidates | jq '[.[] | {id, snippet: (.content | .[0:80])}]'
```

预期:`~/.cache/kb-sediment/*.log` 里能看到三段输出(stage 1 byte count → stage 2
candidate count → stage 3 POST 状态),`/api/candidates` 多出 0-N 条记录。

## 卸载

```bash
rm ~/.claude/hooks/sediment.sh
# 编辑 ~/.claude/settings.json,把 "hooks.PreCompact" 和 "hooks.SessionEnd"
# 里 sediment.sh 那两个对象删掉(或整个键删掉,如果只配了这一个 hook)。
```

## 已知局限

- **transcript 跨机器不可读** — hook 把 jsonl 顶层 uuid 锚作为 source-of-truth,
  candidate.content 里嵌的"原文段"是 hook fire 时本地 transcript 的快照,但如果将来在
  另一台机器做沉淀,**回不到原 jsonl 做更深的提取**。接受这条限制 — 跨机器沉淀的代价
  是少一份原文 context,池子里的概要(kind_hint + why + verbatim 快照)够用。
- **haiku 误判** — 跟所有 LLM 一样,可能漏掉 delta knowledge,可能把 noise 当 signal。
  这就是为什么 candidate 仍然是"未蒸馏"的 — 真正写入 Entry 时由你/Claude 在新 session 里
  二次过滤(`mcp__kb__promote_candidate` / `mcp__kb__discard_candidate`)。
- **频繁 auto-compact 触发**:长会话里 auto-compact 频率不低,每次都跑一次 haiku
  + N 次 POST。haiku 成本忽略不计,但池子膨胀速度可能比你 promote/discard 的速度快。
  目前没有 retention,池子会持续长大 — 用一阵子如果真撑不住再加 GC。
