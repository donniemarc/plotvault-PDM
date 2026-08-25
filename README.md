# PlotVault PDM

轻量级自建图纸文档管理系统 —— 让你的模型和设计文件不再散落各处。

English version: [README.en.md](README.en.md)

---

## 为什么做这个

作为一个 3D 打印爱好者，硬盘里堆积了上百个模型文件——同一款支架改了五六个版本，不知道哪个是最终版；想找一个月前下载的零件，翻遍文件夹也找不到。设计迭代时更头疼：改了一版尺寸后，旧版直接被覆盖，想回退只能重画。

PlotVault 就是为解决这些问题而生的。它让每个文件都有版本历史，同名上传自动归档旧版；图纸直接存在你 NAS 的真实目录里，不用打开软件也能浏览备份。有了它，我终于能安心地打印，不用担心文件管理的混乱了。

## 技术亮点

- **后端**：Rust + Axum + PostgreSQL，单个静态二进制，镜像约 20MB，7×24 常驻零负担
- **客户端**：Tauri 2 + Vue 3 + Three.js，原生窗口、安装包小、启动快
- **存储**：图纸文件直接以真实目录存在磁盘上（目录即数据，备份就是拷贝目录），元数据与版本历史存 PostgreSQL；软件缓存与图纸数据分开挂载

## 功能特性

### 基础功能
- 文件夹树 + 文件列表
- 上传（拖拽）、下载、删除、重命名、移动
- **版本管理**：同名文件重复上传自动成为新版本，可查看历史、下载任意旧版、附版本备注
- 文件名搜索
- 可选 API Token（局域网访问保护）

### 预览支持

| 格式 | 方案 |
|---|---|
| DWG | 服务端 libredwg 转 DXF → 客户端 Three.js 渲染 |
| DXF | 客户端直接渲染（LINE/多段线/圆/圆弧/椭圆/样条/文字标注） |
| STEP / STP / IGES / IGS | 客户端 occt-import-js（OpenCascade WASM）直接解析，完整曲面/装配体 |
| STL | Three.js 直接渲染 |
| PDF | pdf.js 渲染 |
| 图片 / 文本 | 内建视图 |

所有 2D/3D 预览都支持鼠标旋转、缩放、平移。

## 目录结构

```
plotvault-pdm/
├── server/                 # Rust 后端
│   ├── src/                #   main.rs / db.rs / api.rs / storage.rs / convert.rs
│   ├── Dockerfile          #   多阶段构建，含 libredwg（DWG→DXF）
│   └── Cargo.toml
├── client/                 # Tauri + Vue3 桌面客户端
│   ├── src/                #   Vue 前端（UI + 预览）
│   └── src-tauri/          #   Tauri 壳
├── docker-compose.yml       # 一键部署（用 Docker Hub 镜像，占位符 <...>）
├── build-push.ps1           # Windows 一键构建 + 推送到 Docker Hub
├── .env.example
└── README.md
```

## 一、部署后端

### 方式 A：Docker Compose（最简单）

在任意装有 Docker 的机器上：

1. 复制仓库里的 `docker-compose.yml`。
2. 替换所有 `<...>` 占位符：
   - `<DOCKERHUB_USER>` — 你的 Docker Hub 用户名（镜像前缀）
   - `<数据库密码>` — 数据库密码（`POSTGRES_PASSWORD` 与 `DATABASE_URL` 两处必须一致）
   - `<API_TOKEN>` — 客户端连接用的访问令牌
   - `<HOST_PORT>` — 对外端口，例如 `8642` 或 `38642`
   - `<DATA_HOST_PATH>` / `<CONFIG_HOST_PATH>` / `<PGDATA_HOST_PATH>` — 图纸数据、软件缓存、数据库数据的宿主目录
3. 部署：

```bash
docker compose up -d
```

一次启动三个容器：
- `db` — PostgreSQL 18（元数据）
- `plotvault-pdm` — 后端 API，端口 8642
- `converter`（可选）— STEP/IGES 服务端转换，端口 8000

### 方式 B：构建镜像并推送 Docker Hub（用于远程主机）

在 Windows PC 上（需 Docker Desktop）：

```powershell
.\build-push.ps1        # 按提示输入 Docker Hub 用户名
```

然后在目标主机上把 `docker-compose.yml` 的镜像指向 `<你的用户名>/plotvault-pdm-server:latest`，执行 `docker compose up -d`。

### 方式 C：离线 tar 导入（无外网环境）

```bash
# PC 上：
docker build -t plotvault-pdm-server ./server
docker save plotvault-pdm-server | gzip > plotvault-pdm-server.tar.gz
# 传送到目标主机后：
docker load < plotvault-pdm-server.tar.gz
docker compose up -d    # 使用已加载本地镜像的 compose 文件
```

### 验证部署

```bash
curl http://<主机IP>:8642/api/health
# {"service":"plotvault-pdm","status":"ok"}
```

## 二、构建 Windows 客户端

在一台 Windows PC 上构建一次即可（安装包可分发给其他机器）。

需要：Rust（MSVC 工具链 + Visual Studio Build Tools C++ 工作负载）、Node.js 18+、WebView2 运行时（Win10/11 一般自带）。

```powershell
cd client
npm install
npm run tauri build        # 生成 NSIS/MSI 安装包
# 产物在 client/src-tauri/target/release/bundle/
```

开发调试：

```powershell
cd client
npm run tauri dev
```

## 三、使用说明

1. 启动客户端 → **设置** → 填入服务器地址（`http://<主机IP>:8642`）和 API Token → 测试连接 → 保存。
2. **文件**页浏览文件夹树，单击文件预览，按钮执行下载/版本/重命名/移动/删除。
3. 上传：右上角「上传」或直接把文件拖进窗口。同目录同名文件自动作为新版本。
4. 版本：选中文件点「版本」，可查看历史、下载旧版、上传新版本并附备注。
5. 预览右上角可关闭。

## 四、配置

| 环境变量 | 默认值 | 说明 |
|---|---|---|
| `DATA_DIR` | `/data` | 图纸数据目录（library 真实目录 + blobs 版本归档），主机上可直接浏览 |
| `CONFIG_DIR` | `/config` | 软件缓存目录（dxf_cache + tmp） |
| `DATABASE_URL` | `postgres://postgres:postgres@localhost:5432/plotvault_pdm` | PostgreSQL 连接串（compose 里指向 `db` 容器） |
| `BIND` | `0.0.0.0:8642` | 监听地址 |
| `API_TOKEN` | 空 | 设置后所有 API 需 `Authorization: Bearer <token>` |

> 即使是局域网私有环境，也建议设置 `API_TOKEN`。

## 五、FAQ

**DWG 预览报错「dwg2dxf not available」？**
说明容器内没有 `dwg2dxf`（本机裸跑二进制时常见，Docker 镜像会从源码编译 libredwg）。请用 Docker 部署；转换只在首次预览时执行并缓存到 `CONFIG_DIR/dxf_cache`。

**如何备份？**
备份图纸拷贝 `data/` 目录（library 真实目录 + blobs 归档）即可；建议一并备份 `pgdata/` 卷（目录结构、版本备注等元数据，丢了不影响图纸文件本身）。

**想清空重建？**
删除 `config/` + 清空数据库即可重置元数据；删除 `data/` 则连图纸一起清空。

**数据库连不上？**
确认 compose 中 `db` 容器先启动且健康（`pg_isready`），`DATABASE_URL` 的用户/密码/库名与 `POSTGRES_*` 一致。

## 后续计划

- 缩略图（STL/STEP 服务端渲染小图）
- 版本对比 / DWG 导出 PDF
- 批量打包下载（zip）
- 标签 / 分类 / 全文检索
- 审计日志、多用户权限

## 许可

[MIT License](LICENSE) © 2026 donniemarc
