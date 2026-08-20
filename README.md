# 图文档管理（tuwendang）

轻量级个人图纸/文档管理系统，专为 TrueNAS 用户设计。Docker Compose 部署到 NAS，Windows 原生桌面客户端（Tauri）连接使用。

- **后端**：Rust + Axum + **PostgreSQL**，单静态二进制，镜像约 20MB，7×24 常驻 NAS 零负担
- **客户端**：Tauri 2 + Vue 3 + Three.js，原生窗口、安装包小、启动快
- **存储**：图纸 Blob 直接存在 NAS 目录（目录即数据，备份就是拷目录），PostgreSQL 存元数据与版本；软件状态（`/config`）与图纸数据（`/data`）分开挂载

## 功能

### 基础
- 文件夹树 + 文件列表
- 上传（拖拽）、下载、删除、重命名、移动
- **版本管理**：同名图纸重复上传自动成为新版本，可查看历史、下载任意旧版、附版本备注
- 预览（见下表）
- 文件名搜索
- 可选 API Token（局域网访问保护）

### 预览支持矩阵
| 格式 | 方案 |
|---|---|
| DWG | 服务端 `libredwg` 转 DXF → 客户端 Three.js 渲染 |
| DXF | 客户端直接渲染（支持 LINE/多段线/圆/圆弧/椭圆/样条/文字标注点） |
| STEP / STP / IGES / IGS | 客户端 `occt-import-js`（OpenCascade WASM）直接解析，完整曲面/装配体 |
| STL | Three.js 直接渲染 |
| PDF | pdf.js 渲染 |
| 图片 / 文本 | 内建视图 |

所有 2D/3D 预览都支持鼠标旋转、缩放、平移。

## 目录结构

```
tuwendang/
├── server/                 # Rust 后端
│   ├── src/                #   main.rs / db.rs / api.rs / storage.rs / convert.rs
│   ├── Dockerfile          #   multi-stage，含 libredwg（DWG→DXF）
│   └── Cargo.toml
├── client/                 # Tauri + Vue3 客户端
│   ├── src/                #   Vue 前端（UI + 预览）
│   └── src-tauri/          #   Tauri 壳
├── docker-compose.yml       # SSH 部署用
├── truenas-compose.yml      # TrueNAS Custom App 直接粘贴用
├── truenas/                 # PC 一键构建推送脚本 build-push.ps1
├── .env.example
└── README.md
```

## 一、部署到 TrueNAS Scale

> 首选方式：**PC 上构建镜像 → 推到 Docker Hub → TrueNAS Web UI 粘贴 compose 部署**。
> 全程不用 SSH、不改系统。TrueNAS 的 Apps 是原生功能，重启后自动恢复拉起容器，数据随数据池持久化。

### 方式 A：Docker Hub + Web UI（推荐，无需 SSH）

**第一步：在 PC 上构建并推送镜像**（只装 Docker Desktop 到 PC，不涉及 NAS）

```powershell
# 仓库已提供一键脚本（truenas/build-push.ps1）
cd truenas
.\build-push.ps1
# 按提示：docker login → 输入 Docker Hub 用户名
# 脚本会构建 tuwendang-server 并 push 到 <用户名>/tuwendang-server:latest
```

**第二步：TrueNAS Web UI 部署**

1. （可选但推荐）Storage → Create Dataset 建一个数据目录，例如 `pool1/apps/tuwendang`（这样有独立 dataset，方便快照备份）。
2. Apps → Discover → **Custom App** → 勾选 **Use docker-compose**。
3. 粘贴 [truenas-compose.yml](truenas-compose.yml) 的内容，替换四处：
   - `<DOCKERHUB_USER>` → 你的 Docker Hub 用户名
   - `<你的随机TOKEN...>` → 一个随机字符串（客户端连接要用）
   - `<你的数据库密码>` → 一个随机字符串（数据库容器的密码，DATABASE_URL 里要用同一个）
   - `/mnt/pool1/apps/tuwendang/data|config|pgdata` → 你的数据/配置/数据库路径
4. 保存 / 部署，容器状态变为 Running 即可。

**升级方法**：PC 上重新 `docker build + push`，然后在 TrueNAS Apps 里 Update / Redeploy 拉取新镜像。

### 方式 B：SSH + docker compose（需要 SSH，需上传源码）

在 NAS 上创建目录并上传 `server/`、`docker-compose.yml`、`.env.example`：

```bash
cd /mnt/pool1/apps/tuwendang
cp .env.example .env   # 设置 API_TOKEN / PORT
docker compose up -d --build
```

### 方式 C：离线 tar 导入（不出网）

```bash
# PC 上：
docker build -t tuwendang-server ./server
docker save tuwendang-server | gzip > tuwendang-server.tar.gz
# 传到 TrueNAS Web UI：Apps → Manage Docker Images → Import 选择该 tar.gz
# 然后 Custom App 粘贴以下 compose（镜像已在本地，无需拉取）：
```

```yaml
services:
  db:
    image: postgres:18-alpine
    restart: unless-stopped
    environment:
      - POSTGRES_USER=tuwendang
      - POSTGRES_PASSWORD=<你的数据库密码>
      - POSTGRES_DB=tuwendang
    volumes:
      # PG 18+ 镜像数据存于 /var/lib/postgresql/18/docker（主版本子目录），
      # 因此挂载父目录 /var/lib/postgresql（挂载 /var/lib/postgresql/data 会误判为旧数据而拒绝启动）
      - /mnt/pool1/apps/tuwendang/pgdata:/var/lib/postgresql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U tuwendang -d tuwendang"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s

  tuwendang:
    image: tuwendang-server:latest
    restart: unless-stopped
    depends_on:
      - db
    ports:
      - "8642:8642"
    environment:
      - DATA_DIR=/data
      - CONFIG_DIR=/config
      - DATABASE_URL=postgres://tuwendang:<你的数据库密码>@db:5432/tuwendang
      - API_TOKEN=<你的随机TOKEN>
    volumes:
      - /mnt/pool1/apps/tuwendang/data:/data
      - /mnt/pool1/apps/tuwendang/config:/config
```

### 验证部署

```bash
curl http://<NAS-IP>:8642/api/health
# {"service":"tuwendang","status":"ok"}
```

## 二、构建 Windows 客户端

客户端在 **你自己的 Windows PC** 上构建一次即可（构建产物可分发/安装到其他机器）。

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

1. 启动客户端 → **设置** → 填入服务器地址（`http://<NAS-IP>:8642`）和 API Token → 测试连接 → 保存。
2. **文件**页浏览文件夹树，双击文件预览，按钮执行下载/版本/重命名/移动/删除。
3. 上传：右上角「上传」或直接把文件拖进窗口。同名文件在同一目录下会自动作为新版本。
4. 版本：选中文件点「版本」，可查看历史、下载旧版、上传新版本并附备注。
5. 预览右上角可关闭；`v` 版本号显示当前预览的是哪个版本。

## 四、配置

| 环境变量 | 默认值 | 说明 |
|---|---|---|
| `DATA_DIR` | `/data` | 图纸数据目录（library 真实目录 + blobs 版本归档），NAS 上可直接浏览 |
| `CONFIG_DIR` | `/config` | 软件缓存目录（dxf_cache + tmp） |
| `DATABASE_URL` | `postgres://postgres:postgres@localhost:5432/tuwendang` | PostgreSQL 连接串（compose 里指向 `db` 容器） |
| `BIND` | `0.0.0.0:8642` | 监听地址 |
| `API_TOKEN` | 空 | 设置后所有 API 需 `Authorization: Bearer <token>` |

> ⚠️ 个人 NAS 也不建议完全无鉴权暴露在局域网，请务必设置 `API_TOKEN`。

## 五、FAQ

**DWG 预览报错「dwg2dxf not available」？**
说明容器内没有 `dwg2dxf`（本机裸跑后端时常见，Docker 镜像会从源码编译 libredwg）。用 Docker 部署即可；转换只在首次预览时执行并缓存（`CONFIG_DIR/dxf_cache`）。

**如何备份？**
备份**图纸**拷贝 `data/` 目录（library 真实目录 + blobs 版本归档）即可；`pgdata/` 里的 PostgreSQL 元数据建议一并备份（丢了会丢失目录结构/版本备注，但图纸文件本身还在 NAS 上）。

**想清空重建？**
删掉 `config/` 目录 + 清空数据库即可重置元数据；删 `data/` 则连同图纸一起清空。

**数据库连不上？**
确认 compose 中 `db` 容器先启动且健康（`pg_isready`），`DATABASE_URL` 的用户/密码/库名与 `POSTGRES_*` 一致。

**大批量上传很慢？**
当前为单文件流式上传。大文件建议局域网千兆网络直接传。

## 后续可扩展方向
- 缩略图（STL/STEP 服务端渲染小图）
- 图纸版本对比 / DWG 导出 PDF
- 批量打包下载（zip）
- 标签 / 分类 / 全文检索
- 审计日志、多用户权限

## 许可

本软件以「个人免费、商用授权」模式发布（source-available，非 OSI 开源许可）：**个人可免费使用**，**商用需联系作者授权**。详见 [LICENSE](LICENSE)。商用授权联系邮箱：donald_1010@qq.com
