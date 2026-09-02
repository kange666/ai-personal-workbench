# 星枢工作台（ASTRION）开发约定

本文件适用于仓库根目录及全部子目录。目标是让新的 Codex 对话先理解项目边界，再以最小改动完成需求。若用户在当前对话中给出更具体的要求，以用户最新要求为准。

## 1. 开始前必须做

1. 全程使用中文，先说明理解、完成标准和可能存在的歧义；能够从代码确认的内容不要反问用户。
2. 先执行 `git status --short`，记录已有修改。工作区经常包含用户或其他对话尚未提交的工作，禁止覆盖、丢弃、回退或顺手格式化这些修改。
3. 只阅读当前需求相关的页面、前端接口、Rust 模块和测试，不做无关重构。
4. 修改前先搜索现有实现、公共样式和同类页面，优先复用已有模式，不重复造一套。
5. 用户只要求诊断时，只查原因并给证据，不直接修改；用户要求修改时，完成代码和相应验证。
6. 未经明确要求，不提交、不推送、不合并、不打包、不发布，也不修改单独的下载页仓库。

## 2. 产品定位与不可破坏的原则

星枢工作台（ASTRION）是一个面向 Windows 的本地优先个人数字空间，用来汇总 Codex、Git、本地项目、任务、工时、报告、测试、TAPD、接口文档、知识和视频生产信息。品牌宣传语为“一个整合项目、知识、自动化和工作状态的个人数字空间”。核心目标是帮助用户回答：

- 今天和本周完成了什么；
- 哪些事项正在进行、等待确认或存在异常；
- 每个项目投入了多少时间和 Token；
- 过去的实现、决策和问题如何再次复用。

修改时必须保持以下业务口径：

- 本地数据优先，不因为新增功能要求用户部署服务端。
- 自动生成的报告、任务和总结只能来自已有证据，不得虚构完成内容。
- 自动工时始终是估算值，必须允许人工修正，不能描述为精确考勤。
- Git、Codex、TAPD 和项目目录需要通过项目身份映射保持同一项目口径。
- 浏览器开发模式只用于界面调试；扫描本机、系统托盘、Windows 凭据库、项目进程和更新功能必须在 Tauri 桌面环境验证。

## 3. 技术栈和目录

- 前端：Vue 3、TypeScript、Pinia、Vue Router、Vite、Vitest。
- 桌面端：Tauri 2。
- 后端：Rust。
- 数据：本机 SQLite，数据库位于 Windows 应用数据目录的 `com.local.ai-personal-workbench/workbench.sqlite3`。
- 外部通信：`reqwest`；邮件使用 `lettre`；秘密信息使用 Windows Credential Manager（Windows 凭据管理器）。

主要目录：

- `src/views/`：各业务页面。
- `src/components/`：应用外壳、通知、快捷记录、图表和通用交互组件。
- `src/services/backend.ts`：前端类型和全部 Tauri 命令封装，是前后端契约入口。
- `src/router/index.ts`：路由、页面标题和 VIP 路由保护。
- `src/styles/main.css`：全局布局、设计变量和大部分通用组件样式。
- `src/styles/features.css`：业务页面样式。
- `src-tauri/src/lib.rs`：Tauri 初始化、托盘、后台任务和命令注册。
- `src-tauri/src/database.rs`：数据库结构、迁移、查询基础和备份保护。
- `src-tauri/src/*.rs`：按业务拆分的后端模块。
- `scripts/`：开发快捷方式和签名发布脚本。
- `prototype/`：设计参考和原型，不是正式业务数据源。
- `dist/`、`src-tauri/target/`、`release/`、`output/`、日志文件：构建或运行产物，不作为普通功能修改目标。

## 4. 业务模块定位

修改前按下表确定主要入口，避免把逻辑散落到无关文件：

| 功能 | 前端入口 | Rust 入口 |
| --- | --- | --- |
| 全局壳层、头部、菜单、右栏、加载状态 | `src/components/AppShell.vue`、`src/styles/main.css` | `lib.rs`、`maintenance.rs`、`notifications.rs` |
| 工作台和工作记录 | `DashboardView.vue`、`WorkRecordsView.vue` | `worktime.rs`、`reports.rs`、`database.rs` |
| 待处理收件箱 | `InboxView.vue` | `inbox.rs` |
| 项目资产、项目运行、Git | `ProjectsView.vue` | `git.rs`、`project_identity.rs` |
| 项目身份映射 | `ProjectMappingView.vue` | `project_identity.rs` |
| 接口文档中心 | `ApiDocsView.vue` | `apifox.rs` |
| 工作日历和任务 | `CalendarView.vue`、`TasksView.vue` | `database.rs`、`suggestions.rs` |
| 报告中心 | `ReportsView.vue` | `reports.rs`、`ai.rs` |
| 测试中心 | `TestingView.vue`、`TestReportDialog.vue` | `testing.rs` |
| Token 与 Codex 数据 | `TokensView.vue` | `codex.rs` |
| TAPD 和自动处理 | `TapdView.vue`、`TapdAutomationView.vue` | `tapd.rs` |
| 知识库 | `KnowledgeView.vue` | `knowledge.rs`、`ai.rs` |
| 内容工坊和视频中心 | `ContentView.vue`、`VideoCenterView.vue` | `content.rs`、`videos.rs`、`codex_video.rs` |
| 设置、备份、更新、外部服务 | `SettingsView.vue` | `maintenance.rs`、`email.rs`、`vip.rs`、`toolchain.rs` |

新增或修改 Tauri 命令时，通常需要同步完成三处：

1. 在对应 Rust 模块实现命令和序列化类型；
2. 在 `src-tauri/src/lib.rs` 注册命令；
3. 在 `src/services/backend.ts` 增加对应 TypeScript 类型和调用封装。

不要让页面直接散落调用字符串形式的 `invoke`。

## 5. 数据和迁移规则

- 已有数据库必须可原地升级，优先使用向后兼容的增量迁移。
- 修改表结构前先读 `database.rs` 中当前迁移方式和迁移测试；涉及重建表时必须保留旧数据。
- 迁移前备份机制不能绕过；恢复、升级和备份属于高风险功能，必须增加或更新 Rust 测试。
- 不直接修改用户真实数据库来验证。测试使用临时目录和临时数据库，结束后仅清理本次创建的临时数据。
- 列表查询应有确定排序，时间统一使用现有 ISO/RFC3339 口径，避免依赖本机区域格式。
- 页面加载失败与“确实没有数据”必须分开显示；异步读取时复用全局加载状态，不能先闪出空数据结论。

## 6. 关键业务约束

### Codex 与 Token

- 扫描 `.codex/sessions` 和 `.codex/archived_sessions` 时只读，不改写 Codex 文件。
- Token 必须来自实际事件，不用会话文本长度反推精确数值。
- 顶部剩余额度只使用通用 Codex 限额，即 `limit_id=codex`；不要把 GPT-5.3-Codex-Spark 等模型专属额度当成通用额度。
- 扫描、缓存和刷新应保持可重复执行，不能重复累计同一事件。

### Git 与项目资产

- 扫描仓库默认只读，不自动 `add`、提交、拉取、推送、切换分支、合并、回退或清理。
- “提交修改”只分析和提交已经暂存（staged）的文件；未 `add` 的未跟踪/未暂存文件不参与判断，也不能被自动暂存。
- 用户可能只暂存同一文件的一部分。必须保留部分暂存和其余未暂存内容，必要时使用临时 Git index，不能用会扩大范围的 `git add -A`。
- 提交建议使用简洁中文 Conventional Commit：`类型(作用域/功能): 描述`，例如 `feat(workflow): 新增通用提交审核页面`。
- 默认“全部合成一次”；只有用户选择时才按功能关联分组。提交前必须可编辑并由用户确认，不自动推送。
- Git 凭据不得写入仓库、SQLite、命令参数日志或前端状态；使用 Windows 凭据管理器，优先尊重仓库已有 Git 配置。
- 启动项目时复用项目资产中的运行配置；Windows 下后台启动不要弹出 CMD 窗口。应用退出前只停止由工作台启动并记录的项目进程。

### TAPD 自动处理

- 支持多个 TAPD 项目时，数据主键和队列必须包含项目标识，不能只按缺陷 ID 去重。
- 当前产品只同步和展示缺陷，不把任务、需求等其他工作项混入。
- 自动处理规则按项目独立配置；关闭自动执行时只能入队，不能悄悄启动 Codex。
- 同一项目/工作区串行执行，防止多个 Codex 同时修改同一目录。
- Codex 产出必须经过验证和人工确认。未确认前不回写 TAPD；确认归档后才按配置更新为已解决。
- 自动流程不得擅自提交、推送、重置、清理或删除 Git 内容。

### 测试中心和接口测试

- 测试中心默认只读目标项目。新增测试文件、执行真实接口、创建/修改外部测试数据前必须在界面明确说明并取得授权。
- 真实接口凭据只用于当次请求或 Windows 凭据库，不写入 SQLite、报告、截图或日志。
- 测试能力必须按项目真实可用的运行器展示；缺少运行器时说明“不可执行”，不要伪装成零失败。
- PDF 等交付文件先写入临时文件，确认文件头、结尾和稳定大小后再原子替换正式文件。

### 报告、知识和 AI

- 报告按“功能成果”总结，不堆砌逐日流水；项目周报只使用选定范围内有证据的数据。
- 去除无关任务、测试噪音、依赖安装等低价值过程，但保留影响结论的失败、风险和未验证项。
- AI 生成内容必须有本地规则降级方案，并明确标注 AI 不可用或输出不完整，不能静默返回看似成功的内容。
- 向外部 AI 服务发送数据前遵守页面已有确认范围，只发送完成当前功能必需的最小内容。

## 7. 安全和隐私

- 禁止在代码、测试、文档、提交信息、截图或日志中写入真实 API Key、TAPD 令牌、Git 密码、SMTP 授权码、Cookie 或个人访问令牌。
- DeepSeek、TAPD、Git、邮箱、Apifox/接口测试等秘密信息继续使用 Windows Credential Manager；前端只展示“已配置/未配置”。
- 日志和错误信息必须脱敏，URL 中的用户名、密码、Token 查询参数也需要移除。
- 不把真实 Codex 对话、真实工作报告或用户本地文件复制成测试夹具；使用最小匿名样例。
- 不扩大 Tauri 文件访问范围。新增本地路径能力时，限制到业务确实需要的目录并验证路径。
- 不运行或修改项目资产中被扫描到的其他仓库，除非用户明确指定那个仓库和操作。

## 8. UI 与交互约定

- 保持“深色指挥中心”和“暖色”两套主题，不新增第三套视觉语言。
- 优先复用 `main.css` 中的颜色变量、卡片、按钮、输入框、Tag、抽屉和加载样式；不要在单个页面复制一套近似样式。
- 同一行按钮保持相同高度、圆角、图标尺寸和间距。标准操作使用 `.button`，纯图标操作使用 `.icon-button`。
- 页面主体沿用 `.page-area`、`.view`、`.page-header` 和卡片间距；抽屉操作放在标题区右上角，标题保持短而可识别。
- 长标题、路径和日志必须处理换行或省略，不能撑破卡片、表格或右侧抽屉。
- 列表状态使用语义明确且一致的 Tag；加载中、空数据、错误和权限不足是四种不同状态。
- 窗口头部包含拖动和双击最大化逻辑，新增按钮必须排除拖动命中，不能破坏最小化、最大化和关闭到托盘。
- 修改全局按钮、滚动条、菜单、头部或右侧栏时，至少检查常用分辨率和两套主题，避免只修截图中的单页。

## 9. 推荐开发流程

1. 阅读本文件、`README.md`、相关页面和对应 Rust 模块。
2. 执行 `git status --short`，区分任务前已有修改和本次修改。
3. 用 `rg` 搜索已有类型、命令、样式、测试和相似业务。
4. 明确最小实现范围，再用 `apply_patch` 修改；不要批量重写无关文件。
5. 先运行最贴近改动的测试，再运行合理范围的回归测试。
6. 检查 `git diff --check`、本次 diff 和最终 `git status --short`，确认没有把已有修改混入。
7. 向用户说明：完成内容、验证证据、未验证项、现有无关修改，以及下一步是否需要提交/发布。

常用命令：

```powershell
# 安装依赖
npm install

# 桌面开发版；Vue/CSS 热更新，Rust 修改后自动重编译
npm run dev:desktop

# 仅调试前端，使用演示数据
npm run dev

# 前端测试与类型/生产构建
npm test
npm run build

# Rust 测试与格式检查
cargo test --manifest-path .\src-tauri\Cargo.toml
cargo fmt --manifest-path .\src-tauri\Cargo.toml -- --check

# 查看本次改动质量
git diff --check
git status --short
```

验证规则：

- 改 Vue/TypeScript：至少运行相关 Vitest；涉及类型、路由或公共契约时再运行 `npm run build`。
- 改 Rust：至少运行相关模块测试；涉及数据库、命令注册、托盘、更新或公共结构时运行完整 `cargo test`。
- 改前后端契约：同时验证 TypeScript 构建和 Rust 测试。
- 改 UI：代码测试之外，需要在桌面开发版检查真实 Tauri 数据；浏览器截图不能代替桌面运行证据。
- 若完整检查存在任务前就有的失败，保留原状，清楚区分“既有失败”和“本次引入”。
- 不要把“代码能编译”表述为“功能运行通过”，也不要把“本地构建完成”表述为“已发布”。

## 10. Git、版本和发布

- 不自动暂存。用户说“提交”时，也要先列出将提交的确切文件并保护其他暂存/未暂存内容。
- 提交标题沿用仓库格式：简洁中文 Conventional Commit，如 `fix(updater): 修复更新包下载失败`。
- 发布前版本号需要同步检查：`package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/tauri.conf.json`。
- 只有用户明确说“推送发布”时，才执行完整发布流程。发布不是只推源码，至少包括：
  1. 确认工作区范围和版本号；
  2. 运行前端测试、构建和 Rust 测试；
  3. 提交并推送源码仓库；
  4. 运行 `scripts/build-signed-release.ps1` 生成签名安装版、便携版、SHA-256 和 `release.json`；
  5. 在下载仓库创建同版本 GitHub Release 并上传资产；
  6. 更新下载页的 `release.json` 和下载信息；
  7. 在线检查下载页、安装包、便携版、签名和更新清单均可访问。
- 签名私钥和密码只使用本机安全存储或 CI Secret，不读取到回复、不提交 Git。
- 未完成线上 Release 和实际下载检查时，只能说“已构建”或“已推送源码”，不能说“已发布”。

## 11. 完成交付格式

最终回复应简洁包含：

- 已完成：用户可见结果；
- 主要文件：可点击的绝对路径；
- 验证：运行了哪些命令及结果；
- 未验证/风险：只列真实存在的事项；
- Git 状态：是否提交、推送、发布，以及保留了哪些任务前已有修改。

如果需求仍存在会改变实现方向的歧义，停止在安全边界内并向用户提一个明确问题，不自行扩大范围。
