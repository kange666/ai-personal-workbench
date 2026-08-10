# AI 个人工作台

面向 Windows 的本地优先桌面工作台，采用已确认的 B「深色指挥中心」布局，并可切换为 C 的暖色主题。正式数据保存在本机 SQLite 数据库，不需要部署服务器。

## 下载使用

- 项目介绍与下载：[AI 个人工作台下载页](https://kange666.github.io/ai-personal-workbench-download/)
- 源码仓库：[kange666/ai-personal-workbench](https://github.com/kange666/ai-personal-workbench)
- 正式版本：[GitHub Releases](https://github.com/kange666/ai-personal-workbench-download/releases)
- 开发热更新：先执行 `scripts/create-hidden-dev-shortcut.ps1` 生成本机快捷方式，再双击 `AI-Workbench-Dev.lnk`；需要查看编译日志时使用 `启动开发版.cmd`

安装版适合长期使用；便携版可直接运行。两者读取同一份本机工作台数据。

## 已实现

- 每日、每周、项目任务：新增、编辑、完成、删除、逾期状态、本地持久化。
- 首页概览：今天/本周完成内容、每日与每周任务、项目投入、未完成事项、最近测试和近 7 天 Codex 活跃趋势；趋势图带纵坐标、悬停数据和日期点击。
- 日历与甘特：每日任务落到具体日期，每周任务占整周，项目任务按起止日期和进度展示；日历同时显示当天农历，点击可查看宜忌、冲煞、方位等传统黄历信息。
- Codex 数据：扫描当前 Windows 用户的 `.codex/sessions` 与 `.codex/archived_sessions`，索引会话、对话消息和精确 Token 事件。
- Git 数据：从 Codex 会话工作目录识别仓库，读取提交统计和未提交改动快照；扫描过程不会修改仓库。
- 工作记录：按日或周查看从使用 Codex 至今的项目成果，并关联归档对话、Git、测试、知识、报告和 Token。
- 轻量工时：根据 Codex、Git、任务、测试、报告与知识编辑活动自动估算工作区间；默认相邻活动不超过 45 分钟归为同一区间。
- 工时修正：支持补录或修改日期、起止时间、实际分钟、项目、工作类型和备注；重叠时手工记录优先，但原始估算始终保留用于对比。
- Token 分析：输入、缓存、输出、推理和总量，可按天/周/月切换，包含纵坐标、悬停提示、日期点击、模型/项目/会话排行、普通/归档、上下文占用和可调整参考单价的成本估算。
- 日报、周报、月报：根据任务、Codex 对话、Git 提交、测试结果和轻量工时生成，按项目回答“做了什么功能”，过滤停止服务、忽略测试文件等低价值过程；可编辑、锁定、重新生成、AI 润色、查看来源及导出 Word。
- 自动报告：程序在托盘运行时每天 22:00 生成日报，并在周日和月末同时生成周报、月报；正式安装版首次启动会启用开机启动，可在设置中关闭。
- 知识库：从普通和归档 Codex 对话自动提炼技术决策、实现方案、问题解决方法、规范与避坑点；自动内容统一包含适用场景、实现方法、操作步骤、注意事项、参考文件和来源记录，并支持项目/类型筛选、来源跳转和问答。
- 内容工坊：每天分别生成 5 个“小众科技探索”标题和 5 个“每日推理案例”；均包含口播、分镜、AI 画面提示词和剪辑指导，可选择、淘汰、重新生成并复制内容。
- 视频中心：自动读取本机“视频创作”及其 `videos` 子目录中的成片，支持搜索、项目筛选、工作台内直接播放、系统播放器打开和资源管理器定位；本地视频不会上传。
- DeepSeek：API Key 保存在 Windows 凭据库，不写入 SQLite；用于报告 AI 润色和已确认知识问答。
- 测试中心：列出 `client` 已有菜单用例和 `APP/pages.json` 注册页面，可筛选测试状态、复用项目现有功能/样式用例、发起自动测试；报告以概览、检查项、发现与原始输出分区展示。

## 测试中心

- `client`：支持模拟接口功能测试、真实接口功能测试、源码/样式检查和浏览器样式测试，测试脚本来自本机 client 项目的 `e2e` 目录。
- `APP`：项目当前没有菜单级自动化用例，因此只提供路由、页面文件和源码结构静态检查；报告不会冒充真实接口或浏览器测试结果。
- 默认适配作者本机的 client/APP 项目；其他电脑可通过 `AI_WORKBENCH_CLIENT_ROOT` 和 `AI_WORKBENCH_APP_ROOT` 环境变量指定项目根目录。目录不存在时测试中心显示为空，不影响任务、报告、Token、知识和内容功能。
- 真实接口测试默认关闭。启用时可读取 Windows 用户环境变量 `HLZT_TOKEN`，也可输入只用于当次子进程的临时 Token；账号和 Token 都不会保存到 SQLite。
- 真实接口用例可能创建、修改和清理 E2E 前缀测试数据，界面会在运行前明确提醒。

## 数据与隐私

- SQLite 文件位于 Windows 应用数据目录下的 `com.local.ai-personal-workbench/workbench.sqlite3`。
- Codex 和 Git 扫描只读取本机文件，不会自动上传。
- 点击“AI 润色”时，会把当前报告草稿和同期 Codex 对话摘录发送给 DeepSeek；知识问答只发送已确认知识。
- 关闭主窗口只会隐藏到系统托盘；需要彻底退出时，在托盘菜单选择“退出”。
- 自动工时只用于个人工作复盘，界面和报告始终标注为“估算工时”，不等同于精确考勤数据。

## 开发运行

首次克隆源码后，先生成适配当前目录的无终端快捷方式：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\create-hidden-dev-shortcut.ps1
```

之后双击根目录的 `AI-Workbench-Dev.lnk`。它会隐藏终端、自动加载 MSVC 环境并启动桌面开发版；需要排查编译错误时再使用 `启动开发版.cmd`：

- 修改 Vue、TypeScript 或 CSS 后，桌面窗口自动热更新。
- 修改 Rust 后，Tauri 自动增量编译并重启。
- 开发过程无需反复生成安装包。

也可以在已经加载 MSVC 环境的终端执行：

```powershell
npm install
npm run dev:desktop
```

只调界面时可以使用更快的网页开发模式：

```powershell
npm run dev
```

浏览器访问 `http://localhost:1420`。网页模式使用演示数据，不能扫描本机 Codex、Git 或访问 Windows 凭据库。

构建安装包：

```powershell
npm run tauri build
```

## 一键更新发布

- 设置页使用 Tauri Updater 检查签名更新，安装前自动备份数据库，并显示下载进度。
- 更新签名私钥保存在当前 Windows 用户的 `.tauri` 目录，不进入 Git；公钥可以随应用公开。
- 确认版本号和工作区内容后，生成安装版、便携版、签名及兼容下载页的 `release.json`：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-signed-release.ps1 -ReleaseNotes "本次更新说明"
```

- 将生成目录中的安装版、便携版上传到同版本 GitHub Release，并用其中的 `release.json` 更新下载页。
- 私钥或对应密码丢失后，已安装版本将无法验证后续更新。本机密码文件使用 Windows DPAPI 加密，只能由当前 Windows 用户解密；正式发布前还应把私钥和密码分别配置为 GitHub Actions Secret，不能仅依赖复制该密码文件到其他电脑。

## 验证命令

```powershell
npm test
npm run build
cargo fmt --manifest-path .\src-tauri\Cargo.toml -- --check
```

`prototype/` 保留产品原型、两套主题参考和当前实现截图。
