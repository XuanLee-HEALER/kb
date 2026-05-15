#!/usr/bin/env bash
# Sediment hook · Claude Code PreCompact / SessionEnd → POST candidates to kb.
#
# 3-stage pipeline:
#   1) bash+jq sparsify transcript JSONL to high-signal turns
#   2) claude -p --model haiku semantically picks 0-N moments mapping to a
#      KB entry kind, emits {kind_hint, start_uuid, end_uuid, why}
#   3) bash extracts the verbatim turn range by uuid, POSTs to
#      https://kb.lan/api/candidates
#
# Silent on every failure: exit 0 always, never block compact / exit.
# Recursion guard: KB_SEDIMENT_RECURSION_GUARD=1 short-circuits at the top;
# we set it when invoking `claude -p` below so a nested hook fire is a no-op.

set +e
set -u

# ── 0. recursion guard ─────────────────────────────────────────────────────
[ "${KB_SEDIMENT_RECURSION_GUARD:-0}" = "1" ] && exit 0

# ── 0. read hook input ─────────────────────────────────────────────────────
input=$(cat)
transcript_path=$(printf '%s' "$input" | jq -r '.transcript_path // empty' 2>/dev/null)
session_id=$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null)
cwd=$(printf '%s' "$input" | jq -r '.cwd // empty' 2>/dev/null)
event=$(printf '%s' "$input" | jq -r '.hook_event_name // empty' 2>/dev/null)

[ -z "${transcript_path:-}" ] && exit 0
[ -f "$transcript_path" ] || exit 0

# ── 0. fetch KB token from ~/.claude.json (mcpServers.kb.headers) ──────────
# `claude mcp add` writes the bearer there as `Authorization: Bearer <hex>`.
token=$(jq -r '
  ( .mcpServers.kb.headers.Authorization // empty ),
  ( [ .projects[]?.mcpServers.kb.headers.Authorization // empty ][0] // empty )
  | select(. != "")
' "$HOME/.claude.json" 2>/dev/null \
  | head -1 \
  | sed 's/^Bearer //')
[ -z "$token" ] && exit 0

# Logfile for hook debugging (silent on missing dir).
log_dir="$HOME/.cache/kb-sediment"
mkdir -p "$log_dir" 2>/dev/null
log="$log_dir/$(date -u +%Y%m%dT%H%M%S)-${event:-unknown}.log"
exec 3>>"$log" 2>/dev/null
say() { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*" >&3 2>/dev/null; }
say "sediment hook start · event=$event · session=$session_id · transcript=$transcript_path"

# ── stage 1. structural sparsification ─────────────────────────────────────
signal_file=$(mktemp)
trap 'rm -f "$signal_file" "${claude_in:-/dev/null}" "${claude_out:-/dev/null}"' EXIT

# Signal selectors (assistant turns only — user-side tool_result errors are
# mostly noisy stderr dumps, rarely the locus of delta knowledge):
#   - any `thinking` block (claude's internal reasoning, often "wait actually X")
#   - any `tool_use` of WebSearch / WebFetch / ExitPlanMode (external knowledge / decisions)
#   - any `text` block > 500 chars (substantive output, often summaries)
jq -c '
  select(
    .type == "assistant"
    and ((.message.content // []) | type == "array")
    and (
      ((.message.content // []) | any(.type == "thinking")) or
      ((.message.content // []) | any(.type == "tool_use" and (
        .name == "WebSearch" or .name == "WebFetch" or .name == "ExitPlanMode"
      ))) or
      ((.message.content // []) | any(.type == "text" and ((.text // "") | length) > 500))
    )
  )
' "$transcript_path" > "$signal_file" 2>/dev/null

signal_bytes=$(wc -c < "$signal_file" 2>/dev/null || echo 0)
signal_lines=$(wc -l < "$signal_file" 2>/dev/null || echo 0)
say "stage 1: ${signal_lines} signal turns, ${signal_bytes} bytes"
[ "$signal_bytes" -lt 100 ] && { say "stage 1: nothing to filter, exit"; exit 0; }

# Cap the signal file at ~150KB. claude-code injects ~45K tokens of
# system prompt + memory + tool defs before our content (measured: 40K
# cache_read + 6.7K cache_creation for an empty user message). The
# remaining ~155K of haiku's 200K context has to hold our prompt + the
# transcript. Chinese/code-mixed UTF-8 averages ~2 bytes per token, so
# 150KB ≈ 75K tokens, leaving 80K headroom for prompt + response.
# Empirically 300KB tripped "Prompt is too long" 400.
SIGNAL_CAP_BYTES=150000
if [ "$signal_bytes" -gt "$SIGNAL_CAP_BYTES" ]; then
  say "stage 1: signal $signal_bytes bytes > $SIGNAL_CAP_BYTES, truncating to tail"
  tail -c "$SIGNAL_CAP_BYTES" "$signal_file" > "${signal_file}.tail" && mv "${signal_file}.tail" "$signal_file"
fi

# ── stage 2. haiku semantic filter ─────────────────────────────────────────
claude_in=$(mktemp)
claude_out=$(mktemp)

cat > "$claude_in" <<'PROMPT'
你正在审阅一段 Claude Code session 的高信号片段(已经做过结构化去噪)。
每一行是一个 jsonl turn,顶层有 uuid 字段。

任务:找出可映射到下列五种 KB entry kind 之一的瞬间:
- Fact:通用 LLM 没有 / 会答错的客观事实
- ProblemSolution:有 environment + symptoms + root-cause + verified-fix 的具体问题修复
- Lesson:"以为 X 实际 Y" 的思考陷阱 + 纠正
- Decision:带明确 tradeoff 的架构选择
- Heuristic:针对一类问题的思考模式

排除:闲聊、读书笔记、生活管理、不完整的观察、常识。
只挑 DELTA knowledge — 没有文档检索的通用 LLM 会答错的内容。
宁缺勿滥 — 没把握就不要输出。

输出一个 JSON array,每项严格:
  {"kind_hint":"...", "start_uuid":"...", "end_uuid":"...", "why":"<= 30 字"}

start_uuid / end_uuid 是顶层 uuid 字段(每行最外层 .uuid),用来锚定一段相邻的 turn(可以是同一 uuid 代表单 turn)。
没有可挑的,输出:[]
只输出 JSON array,不要 markdown 围栏,不要任何说明文字。

--- TRANSCRIPT SIGNAL TURNS ---
PROMPT
cat "$signal_file" >> "$claude_in"

say "stage 2: calling claude -p haiku ($(wc -c < "$claude_in") bytes input)"

# Portable timeout: `timeout` is GNU coreutils only — on macOS without
# `brew install coreutils` it's missing. Use whichever exists; if neither,
# rely on Claude Code's hook-level timeout (settings.json: "timeout": 120s)
# to kill us if claude -p hangs.
if command -v gtimeout >/dev/null 2>&1; then
    TIMEOUT_CMD="gtimeout 120"
elif command -v timeout >/dev/null 2>&1; then
    TIMEOUT_CMD="timeout 120"
else
    TIMEOUT_CMD=""
fi

# `--output-format json` wraps the assistant reply under .result so we can
# robustly extract it. We deliberately don't set `--permission-mode` /
# `--allowed-tools` — combining those with large inputs has misbehaved in
# testing, and the prompt itself instructs haiku to output JSON only.
KB_SEDIMENT_RECURSION_GUARD=1 $TIMEOUT_CMD \
  claude -p \
    --model claude-haiku-4-5 \
    --output-format json \
    < "$claude_in" > "$claude_out" 2>>"$log_dir/claude-stderr.log"

claude_exit=$?
claude_out_bytes=$(wc -c < "$claude_out" 2>/dev/null || echo 0)
say "stage 2: claude exit=$claude_exit, output=$claude_out_bytes bytes"
[ "$claude_exit" -ne 0 ] || [ "$claude_out_bytes" -lt 2 ] && exit 0

# Extract the assistant reply text (claude -p --output-format json wraps it).
result_text=$(jq -r '.result // empty' "$claude_out" 2>/dev/null)
[ -z "$result_text" ] && { say "stage 2: empty .result"; exit 0; }

# Strip optional markdown fences ```json ... ``` that haiku might wrap with.
# Crude but robust: remove every triple-backtick (with or without `json` tag).
result_text=$(printf '%s' "$result_text" | sed -E 's/```(json)?//g')

# Parse the candidate array.
candidates_count=$(printf '%s' "$result_text" | jq -r 'if type == "array" then length else 0 end' 2>/dev/null)
[ -z "$candidates_count" ] || [ "$candidates_count" = "0" ] && { say "stage 2: 0 candidates"; exit 0; }
say "stage 2: $candidates_count candidate(s) proposed"

# ── stage 3. extract verbatim by uuid, POST ────────────────────────────────
source_json=$(jq -nc \
  --arg sid "$session_id" --arg cwd "$cwd" \
  '{type:"ClaudeCode", session_id:$sid, project:"sediment-hook", cwd:$cwd}')

posted=0
printf '%s' "$result_text" | jq -c '.[]?' 2>/dev/null | while IFS= read -r cand; do
  kind=$(printf '%s' "$cand"  | jq -r '.kind_hint  // empty')
  start=$(printf '%s' "$cand" | jq -r '.start_uuid // empty')
  end=$(printf '%s' "$cand"   | jq -r '.end_uuid   // .start_uuid // empty')
  why=$(printf '%s' "$cand"   | jq -r '.why        // empty')
  [ -z "$kind" ] || [ -z "$start" ] && continue

  # Extract jsonl line range whose .uuid runs from $start..$end (inclusive)
  excerpt=$(jq -c --arg s "$start" --arg e "$end" '
    . as $row
    | select(true)
  ' "$transcript_path" 2>/dev/null \
    | awk -v start="$start" -v end="$end" '
        BEGIN { inrange = 0; n = 0 }
        {
          if (index($0, "\"uuid\":\""start"\"") > 0) inrange = 1
          if (inrange) { lines[++n] = $0 }
          if (index($0, "\"uuid\":\""end"\"") > 0 && inrange) { exit }
        }
        END { for (i = 1; i <= n; i++) print lines[i] }
      ')
  [ -z "$excerpt" ] && { say "stage 3: uuid $start..$end not found, skip"; continue; }

  content=$(printf '[kind_hint: %s]\n[why: %s]\n[from: session %s · event %s]\n\n%s' \
    "$kind" "$why" "$session_id" "$event" "$excerpt")

  # Server enforces 16KB cap; trim before send to avoid 400.
  content_trim=$(printf '%s' "$content" | head -c 15000)

  payload=$(jq -nc --arg c "$content_trim" --argjson s "$source_json" \
    '{content: $c, source: $s}')

  http=$(curl -sS --max-time 10 -o /dev/null -w '%{http_code}' \
    -X POST -H "Authorization: Bearer $token" -H 'Content-Type: application/json' \
    -d "$payload" "https://kb.lan/api/candidates" 2>/dev/null)
  say "stage 3: POST kind=$kind anchor=${start:0:8}..${end:0:8} → http=$http"
done

say "sediment hook done"
exit 0
