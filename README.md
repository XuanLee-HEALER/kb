# KB

个人知识库系统 · delta-oriented · schema-first · MCP-native

按 [docs/design-v2.2.html](docs/design-v2.2.html) 的 v2.2 设计实现。五种 entry kind (Fact / ProblemSolution / Lesson / Decision / Heuristic),写入走三层去重 (hash → FTS5 jieba → embedding[v3]),读出走三通道 (structured / FTS / semantic[v3])。对外两面:MCP (给 Claude Code) 与 REST (给人 + Web UI)。

## 布局

```
crates/
  kb-core/      schema、normalize_nk、Layer 1/2 类型 (13 unit tests)
  kb-server/    axum + rmcp + r2d2_sqlite (12 integration tests)
  kb-cli/       kb 二进制
web/            Astro SSR + bun (列表/搜索/详情/新建/编辑/废弃/duplicates 流程)
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
just dev            # kb-server + astro dev 并行,Ctrl+C 全杀
```

更多 recipe:`just` (无参数)。

## 配置 (env / .env)

| Var               | 默认                          | 说明                                  |
| ----------------- | ----------------------------- | ------------------------------------- |
| `KB_BIND`         | `127.0.0.1:5100`              | kb-server 监听地址                    |
| `KB_DB`           | `data/kb.sqlite`              | SQLite 路径 (WAL 模式)               |
| `KB_TOKEN`        | (dev 占位符)                  | bearer token,生产强烈建议设          |
| `KB_URL`          | `http://127.0.0.1:5100`      | Web UI / CLI 指向的 kb-server         |
| `PORT`            | `5101`                        | Astro web UI 监听端口                 |
| `KB_SKILL_DIR`    | `./skill`                     | `/skill/*` 端点服务的目录             |
| `KB_LIBSIMPLE_DIR`| 编译期嵌入                    | 运行时覆盖 libsimple.so 路径          |
| `KB_LIBSIMPLE_DICT`| 编译期嵌入                   | 运行时覆盖 jieba dict 目录            |

## 部署

x86_64-linux 制品由 CI 在打 tag 时产出。本地复现:`just build-release` → `kb-release-darwin-arm64.tar.gz` (Mac) 或同名 linux-x86_64 (Ubuntu)。

设计文档第 8 节给了 nginx 反代 + Aliyun ECS 拓扑。

## 测试

```sh
just test              # workspace 全部
just test-one layer2   # 名字含 layer2 的
```

集成测试在 `crates/kb-server/tests/storage.rs`,验证 Layer 1+2 去重、supersede 事务原子性、deprecated 默认过滤、FTS 排序等。
