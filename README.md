# KB

个人知识库系统 · delta-oriented · schema-first · MCP-native

按 [docs/design-v2.2.html](docs/design-v2.2.html) 的 v2.2 设计实现。五种 entry kind (Fact / ProblemSolution / Lesson / Decision / Heuristic),写入走三层去重 (hash → FTS5 jieba → embedding[v3]),读出走三通道 (structured / FTS / semantic[v3])。对外两面:MCP (给 Claude Code) 与 REST (给人 + Web UI)。

## 布局

```
crates/
  kb-core/      schema、normalize_nk、Layer 1/2 类型 (13 unit tests)
  kb-server/    axum + rmcp + r2d2_sqlite (12 integration tests)
  kb-cli/       kb 二进制
web/            Hono on Bun · TSX SSR (列表/搜索/详情/新建/编辑/废弃/duplicates 流程)
skill/          SKILL.md + 7 examples + mcp.json 模板
vendor/libsimple/  wangfenjin/simple 源码 vendor (build.rs 自动编)
scripts/        build-skill.sh / stage-release.sh
.github/workflows/  ci + release
```

## 上手

依赖:`rustup` (stable), `bun`, `cmake`, `just`, `sqlite3` (可选)。

```sh
just setup          # 一次性:校验依赖 + bun install + 建 data/
just build          # cargo + web 全 release 编一遍 (首次包含 libsimple cmake)
just test           # 25 个测试全跑
just check          # 完整 CI 镜像:fmt-check + clippy + test + web check
just dev            # kb-server + hono web (bun --hot) + 客户端 watch 并行,Ctrl+C 全杀
```

更多 recipe:`just` (无参数)。

## 配置 (env / .env)

| Var               | 默认                          | 说明                                  |
| ----------------- | ----------------------------- | ------------------------------------- |
| `KB_BIND`         | `127.0.0.1:5100`              | kb-server 监听地址                    |
| `KB_DB`           | `data/kb.sqlite`              | SQLite 路径 (WAL 模式)               |
| `KB_TOKEN`        | (dev 占位符)                  | bearer token,生产强烈建议设          |
| `KB_URL`          | `http://127.0.0.1:5100`      | Web UI / CLI 指向的 kb-server         |
| `PORT`            | `5101`                        | web UI 监听端口                       |
| `KB_SKILL_DIR`    | `./skill`                     | `/skill/*` 端点服务的目录             |
| `KB_LIBSIMPLE_DIR`| 编译期嵌入                    | 运行时覆盖 libsimple.so 路径          |
| `KB_LIBSIMPLE_DICT`| 编译期嵌入                   | 运行时覆盖 jieba dict 目录            |

## 部署

x86_64-linux 制品由 CI 在打 tag 时产出。本地复现:`just build-release` → `kb-release-darwin-arm64.tar.gz` (Mac) 或同名 linux-x86_64 (Ubuntu)。

设计文档第 8 节给了 nginx 反代 + Aliyun ECS 拓扑。

## Claude Code skill + MCP 安装

KB 走 Claude Code 的标准 skill + MCP 机制。两件事分开:**skill**(SKILL.md + examples,文档级)和 **MCP server**(write/get/search/purge 等工具)。

### A. 让 Claude 自动装(内网推荐)

在 Claude Code 里说一句:

> 帮我装 kb,指引在 https://kb.lan/skill/install

Claude 会 GET 这个 endpoint,拿到一个结构化 plan,先问你 USER scope 还是 PROJECT scope,然后:
- 下载 + 解压 skill bundle 到对应位置(`~/.claude/skills/` 或 `./.claude/skills/`)
- 跑 `claude mcp add` 注册 MCP server(`--scope user` 或 `--scope local`)

两种 scope 都不会把 bearer token 写入 git。

### B. 手动

下载 skill:

```sh
mkdir -p ~/.claude/skills && curl -fsSL https://kb.lan/skill/download | tar xz -C ~/.claude/skills/
# 或 project scope:
# mkdir -p ./.claude/skills && curl -fsSL https://kb.lan/skill/download | tar xz -C ./.claude/skills/
```

注册 MCP server(用 `claude mcp add`,**不要手编** `~/.claude.json`):

```sh
# 全机器可用
claude mcp add --transport http kb https://kb.lan/mcp \
  --header "Authorization: Bearer <YOUR_TOKEN>" --scope user

# 或仅当前项目(token 仍在 ~/.claude.json,不入 git)
claude mcp add --transport http kb https://kb.lan/mcp \
  --header "Authorization: Bearer <YOUR_TOKEN>" --scope local
```

**绝不要用 `--scope project`** — 那会写 `.mcp.json` 到 project root,token 会被 commit 进 git。Skill 内的 `mcp.json` 是占位文档,Claude Code **不**从 skill 目录读 MCP 配置,改它没用。

重启 Claude Code 让新 MCP server 生效。

### 升级检测

```sh
diff <(curl -s https://kb.lan/skill/version) \
     ~/.claude/skills/kb-skill/VERSION
# 不同 → 重跑上面的下载步骤;MCP 配置不用动
```

## 测试

```sh
just test              # workspace 全部
just test-one layer2   # 名字含 layer2 的
```

集成测试在 `crates/kb-server/tests/storage.rs`,验证 Layer 1+2 去重、supersede 事务原子性、deprecated 默认过滤、FTS 排序等。
