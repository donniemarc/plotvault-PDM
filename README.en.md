# PlotVault PDM

A lightweight, self-hosted drawing and document management system. Deploy the backend anywhere Docker runs (NAS, home server, VPS, or your own machine) and manage your files from a native Windows desktop client.

中文版本: [README.md](README.md)

---

## Why I Built This

As a 3D printing enthusiast, I had hundreds of model files scattered across my hard drive — the same bracket redesigned five or six times, with no idea which was the final version. Finding a part I downloaded a month ago meant digging through endless folders. Design iterations were even worse: after resizing a model, the old version was simply overwritten, and I had to redraw from scratch if I wanted to go back.

PlotVault was built to solve these problems. Every file gets version history — uploading the same name automatically archives the old version. Drawings live as real directories on your NAS, so you can browse and back up without even opening the app. Now I can print with peace of mind, without worrying about file chaos.

## Tech Stack

- **Backend**: Rust + Axum + PostgreSQL, single static binary, ~20 MB Docker image, runs 24/7 with minimal footprint
- **Client**: Tauri 2 + Vue 3 + Three.js, native window, small installer, fast startup
- **Storage**: drawing files are stored directly on your disk as real directories (your data is the directory — backup by copying the folder), metadata and versions live in PostgreSQL; software cache and drawing data are mounted separately

## Features

### Core
- Folder tree + file list
- Upload (drag & drop), download, delete, rename, move
- **Versioning**: uploading a file with the same name automatically creates a new version; view history, download any old version, add per-version notes
- Filename search
- Optional API token (LAN access protection)

### Preview matrix

| Format | How |
|---|---|
| DWG | Server-side `libredwg` converts to DXF → client renders with Three.js |
| DXF | Rendered directly in the client (LINE/polyline/circle/arc/ellipse/spline/text annotations) |
| STEP / STP / IGES / IGS | Parsed client-side with `occt-import-js` (OpenCascade WASM), full surfaces/assemblies |
| STL | Three.js directly |
| PDF | pdf.js |
| Image / Text | Built-in viewer |

All 2D/3D previews support mouse rotate, zoom, and pan.

## Project Layout

```
plotvault-pdm/
├── server/                 # Rust backend
│   ├── src/                #   main.rs / db.rs / api.rs / storage.rs / convert.rs
│   ├── Dockerfile          #   multi-stage, includes libredwg (DWG→DXF)
│   └── Cargo.toml
├── client/                 # Tauri + Vue3 desktop client
│   ├── src/                #   Vue frontend (UI + preview)
│   └── src-tauri/          #   Tauri shell
├── docker-compose.yml       # one-file deployment (uses Docker Hub images, placeholders <...>)
├── build-push.ps1           # Windows one-click build + push to Docker Hub
├── .env.example
└── README.md
```

## 1. Deploy the Backend

### Option A: Docker Compose (easiest)

On any machine with Docker:

1. Copy `docker-compose.yml` from this repo.
2. Replace all `<...>` placeholders:
   - `<DOCKERHUB_USER>` — your Docker Hub username (image prefix)
   - `<数据库密码>` — the database password (must match in `POSTGRES_PASSWORD` and `DATABASE_URL`)
   - `<API_TOKEN>` — the access token the client uses to connect
   - `<HOST_PORT>` — the host port, e.g. `8642` or `38642`
   - `<DATA_HOST_PATH>` / `<CONFIG_HOST_PATH>` / `<PGDATA_HOST_PATH>` — host paths for drawings data, software cache, and PostgreSQL data
3. Deploy:

```bash
docker compose up -d
```

The stack starts three containers:
- `db` — PostgreSQL 18 (metadata)
- `plotvault-pdm` — the backend API on port 8642
- `converter` (optional) — server-side STEP/IGES conversion on port 8000

### Option B: Build the image and push to Docker Hub (for remote hosts)

On your Windows PC (requires Docker Desktop):

```powershell
.\build-push.ps1        # prompts for your Docker Hub username
```

Then on the target host, point `docker-compose.yml` at `<your-user>/plotvault-pdm-server:latest` and run `docker compose up -d`.

### Option C: Offline tar import (air-gapped)

```bash
# On your PC:
docker build -t plotvault-pdm-server ./server
docker save plotvault-pdm-server | gzip > plotvault-pdm-server.tar.gz
# Transfer the archive, then load it on the target host:
docker load < plotvault-pdm-server.tar.gz
docker compose up -d    # using a compose file with the locally loaded image
```

### Verify

```bash
curl http://<host>:8642/api/health
# {"service":"plotvault-pdm","status":"ok"}
```

## 2. Build the Windows Client

Build once on a Windows PC (the installer can be distributed to other machines).

Requirements: Rust (MSVC toolchain + Visual Studio Build Tools C++ workload), Node.js 18+, WebView2 runtime (usually preinstalled on Win10/11).

```powershell
cd client
npm install
npm run tauri build        # produces an NSIS/MSI installer
# Output in client/src-tauri/target/release/bundle/
```

Development:

```powershell
cd client
npm run tauri dev
```

## 3. Usage

1. Start the client → **Settings** → enter server address (`http://<host>:8642`) and API token → Test connection → Save.
2. On the **Files** page, browse the folder tree, click a file to preview, and use the buttons for download / versions / rename / move / delete.
3. Upload: click **Upload** in the top right, or simply drag files into the window. Same-named files in the same folder become new versions automatically.
4. Versions: select a file → **Versions** to view history, download old versions, or upload a new version with a note.
5. Close preview via the button in the top-right of the preview pane.

## 4. Configuration

| Env var | Default | Description |
|---|---|---|
| `DATA_DIR` | `/data` | Drawing data directory (library real dirs + blobs archive), browsable directly on the host |
| `CONFIG_DIR` | `/config` | Software cache (dxf_cache + tmp) |
| `DATABASE_URL` | `postgres://postgres:postgres@localhost:5432/plotvault_pdm` | PostgreSQL connection string (points at the `db` container in compose) |
| `BIND` | `0.0.0.0:8642` | Listen address |
| `API_TOKEN` | empty | When set, all APIs require `Authorization: Bearer <token>` |

> Even on a private LAN, it is recommended to set `API_TOKEN`.

## 5. FAQ

**DWG preview reports "dwg2dxf not available"?**
The container lacks `dwg2dxf` — this happens when running the binary bare-metal (the Docker image compiles libredwg from source). Use Docker; conversion runs once on first preview and is cached in `CONFIG_DIR/dxf_cache`.

**How to back up?**
Copy `data/` (library real dirs + blobs archive) for your drawings; also back up the `pgdata/` volume to preserve metadata (folder structure, version notes). Drawings themselves remain on disk even without the DB.

**Want to reset?**
Delete `config/` + clear the database to reset metadata; deleting `data/` also removes all drawings.

**Can't connect to the database?**
Make sure the `db` container starts first and is healthy (`pg_isready`), and that `DATABASE_URL` user/password/dbname match the `POSTGRES_*` env vars.

## Roadmap

- Thumbnails (server-rendered STL/STEP previews)
- Version diff / DWG→PDF export
- Batch download (zip)
- Tags / categories / full-text search
- Audit log, multi-user permissions

## License

[MIT License](LICENSE) © 2026 donniemarc
