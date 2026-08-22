# PlotVault PDM — 应用图标实现规范

> 本文档供 AI 读取并执行。按以下规范将「Drawing Folder（图纸文件夹）」图标集成到 PlotVault PDM 应用中。

---

## 1. 设计概述

### 1.1 图标名称
Drawing Folder（图纸文件夹）— 方案 5

### 1.2 设计概念
文件夹造型 = **管理/归档**；内部图纸线条 = **Plot（图纸内容）**；底部钥匙孔 = **Vault（安全存储）**。三者合一，直接传达 PDM 核心功能。

### 1.3 两套版本

| 版本 | 用途 | 主题适配方式 |
|------|------|-------------|
| **彩色版** (icon-color) | 桌面图标、安装包图标、About 页 | 自带渐变背景，深/浅主题下外观一致，无需切换 |
| **单色版** (icon-mono) | 窗口标题栏、菜单栏、系统托盘 | 使用 `currentColor`，深色主题→白色，浅色主题→深色，自动适配 |

---

## 2. 颜色规范

### 2.1 彩色版调色板

| 用途 | 颜色 | HEX | RGB |
|------|------|-----|-----|
| 背景渐变-上 | Indigo | `#5145D7` | 81, 69, 215 |
| 背景渐变-下 | Deep Indigo | `#3B2FB5` | 59, 47, 181 |
| 主体（文件夹） | White | `#FFFFFF` | 255, 255, 255 |
| 图纸线条/钥匙孔 | Indigo | `#5145D7` | 81, 69, 215 |
| 图纸线条透明度 | — | — | opacity: 0.45 |

### 2.2 单色版

| 属性 | 值 |
|------|-----|
| 填充/描边 | `currentColor`（继承父元素 CSS `color` 属性） |
| 深色主题下 | 父元素 `color: #FFFFFF` |
| 浅色主题下 | 父元素 `color: #1A1A2E` |

---

## 3. SVG 源码

### 3.1 彩色版 SVG（icon-color.svg）

```xml
<svg viewBox="0 0 256 256" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="grad-folder" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#5145D7"/>
      <stop offset="100%" stop-color="#3B2FB5"/>
    </linearGradient>
  </defs>
  <!-- 圆角方形背景 -->
  <rect width="256" height="256" rx="56" fill="url(#grad-folder)"/>
  <!-- 文件夹主体 -->
  <path d="M56 84 L100 84 L116 100 L200 100 L200 200 L56 200 Z" fill="#FFFFFF"/>
  <!-- 文件夹标签页（半透明白色，增强层次） -->
  <path d="M56 84 L100 84 L116 100 L56 100 Z" fill="#FFFFFF" opacity="0.85"/>
  <!-- 图纸线条（3 条，表达图纸内容） -->
  <line x1="80" y1="128" x2="176" y2="128" stroke="#5145D7" stroke-width="3" opacity="0.45"/>
  <line x1="80" y1="144" x2="168" y2="144" stroke="#5145D7" stroke-width="3" opacity="0.45"/>
  <line x1="80" y1="160" x2="172" y2="160" stroke="#5145D7" stroke-width="3" opacity="0.45"/>
  <!-- 钥匙孔圆形部分 -->
  <circle cx="128" cy="182" r="9" fill="#5145D7"/>
  <!-- 钥匙孔梯形部分 -->
  <path d="M124 182 L128 198 L132 182 Z" fill="#5145D7"/>
</svg>
```

### 3.2 单色版 SVG（icon-mono.svg）

```xml
<svg viewBox="0 0 256 256" xmlns="http://www.w3.org/2000/svg">
  <!-- 文件夹轮廓 -->
  <path d="M56 84 L100 84 L116 100 L200 100 L200 200 L56 200 Z"
        fill="none" stroke="currentColor" stroke-width="8" stroke-linejoin="round"/>
  <!-- 钥匙孔圆形部分 -->
  <circle cx="128" cy="178" r="9" fill="currentColor"/>
  <!-- 钥匙孔梯形部分 -->
  <path d="M124 178 L128 194 L132 178 Z" fill="currentColor"/>
</svg>
```

> **注意**：单色版省略了图纸线条和文件夹标签页，仅保留文件夹轮廓 + 钥匙孔，确保在 16×16 像素下仍可辨识。

---

## 4. 图形结构详解

### 4.1 画布

- viewBox: `0 0 256 256`
- 坐标系：左上角 (0,0)，右下角 (256,256)
- 中心点：(128, 128)

### 4.2 背景圆角方形

```xml
<rect width="256" height="256" rx="56" fill="url(#grad-folder)"/>
```

- 尺寸：256 × 256
- 圆角半径：56px（约占边长 22%，符合现代应用图标风格）
- 填充：垂直线性渐变（上→下，#5145D7 → #3B2FB5）

### 4.3 文件夹主体路径

```
M56 84  → 起笔：左侧上部
L100 84 → 向右画到标签页右边缘
L116 100 → 斜向下到文件夹主体顶部（形成标签页斜角）
L200 100 → 向右画到文件夹主体右边缘
L200 200 → 向下画到底部右角
L56 200  → 向左画到底部左角
Z        → 闭合回起笔点
```

- 主体范围：X [56, 200] × Y [84, 200]，宽 144 × 高 116
- 标签页斜角：从 (100, 84) 到 (116, 100)，形成 45° 斜边
- 填充：纯白 #FFFFFF

### 4.4 图纸线条（仅彩色版）

3 条水平线，模拟图纸上的内容线：

| 线条 | x1 | y1 | x2 | y2 | 端点不齐 |
|------|----|----|----|----|---------|
| 第 1 条 | 80 | 128 | 176 | 128 | 右端最长 |
| 第 2 条 | 80 | 144 | 168 | 144 | 右端较短 |
| 第 3 条 | 80 | 160 | 172 | 160 | 右端中等 |

- 描边色：#5145D7
- 描边宽：3
- 透明度：0.45
- 端点故意不齐整，模拟手绘图纸效果

### 4.5 钥匙孔

由两部分组成，合在一起形成经典钥匙孔形状（上部圆形 + 下部梯形）：

**彩色版**（位于文件夹主体内部偏下位置）：
```xml
<circle cx="128" cy="182" r="9" fill="#5145D7"/>
<path d="M124 182 L128 198 L132 182 Z" fill="#5145D7"/>
```

**单色版**（位置略上移以补偿省略的图纸线条）：
```xml
<circle cx="128" cy="178" r="9" fill="currentColor"/>
<path d="M124 178 L128 194 L132 178 Z" fill="currentColor"/>
```

钥匙孔结构说明：
- 圆形部分：圆心 (128, 182/178)，半径 9
- 梯形部分：3 个顶点 — 左 (124, 182/178)、下 (128, 198/194)、右 (132, 182/178)
- 梯形从圆形底部向下逐渐收窄，形成钥匙孔经典造型

---

## 5. 尺寸输出要求

### 5.1 桌面图标（彩色版）

需要导出以下尺寸的 PNG 文件：

| 尺寸 | 用途 |
|------|------|
| 16×16 | 任务栏小图标 |
| 32×32 | 桌面图标小 |
| 48×48 | 桌面图标中 |
| 64×64 | 桌面图标大 |
| 128×128 | 应用商店/关于页 |
| 256×256 | 高清桌面图标 |

Windows 还需要 `.ico` 格式（包含 16/32/48/256 多尺寸）。
macOS 需要 `.icns` 格式。

### 5.2 窗口标题栏图标（单色版）

| 尺寸 | 用途 |
|------|------|
| 16×16 | 标题栏标准尺寸 |
| 20×20 | 标题栏高清（Retina） |
| 32×32 | 高分屏备用 |

---

## 6. 主题切换实现

### 6.1 方案说明

```
桌面图标：始终使用彩色版 → 深色/浅色主题下外观一致，无需任何切换逻辑
标题栏图标：使用单色版 + CSS currentColor → 通过修改父元素 color 属性自动适配
```

### 6.2 CSS 实现（标题栏单色图标）

```css
/* 标题栏容器 */
.titlebar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 14px;
}

/* 深色主题 */
.app-theme-dark .titlebar {
  color: #FFFFFF;
}

/* 浅色主题 */
.app-theme-light .titlebar {
  color: #1A1A2E;
}

/* 图标 SVG 继承父元素 color */
.titlebar-icon svg {
  width: 18px;
  height: 18px;
  /* 无需单独设置颜色，currentColor 自动继承 .titlebar 的 color */
}
```

### 6.3 HTML 结构（标题栏）

```html
<div class="titlebar">
  <!-- 单色版 SVG 内联，currentColor 会继承父元素 color -->
  <span class="titlebar-icon">
    <svg viewBox="0 0 256 256" xmlns="http://www.w3.org/2000/svg">
      <path d="M56 84 L100 84 L116 100 L200 100 L200 200 L56 200 Z"
            fill="none" stroke="currentColor" stroke-width="8" stroke-linejoin="round"/>
      <circle cx="128" cy="178" r="9" fill="currentColor"/>
      <path d="M124 178 L128 194 L132 178 Z" fill="currentColor"/>
    </svg>
  </span>
  <span class="titlebar-title">PlotVault PDM</span>
</div>
```

### 6.4 JavaScript 主题切换

```javascript
// 切换主题时，只需修改根元素的 class
function setTheme(theme) {
  const root = document.documentElement;
  root.classList.remove('app-theme-dark', 'app-theme-light');
  root.classList.add(`app-theme-${theme}`);
  // 标题栏图标会自动通过 currentColor 适配，无需额外处理
}

// 初始化时读取系统主题
const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
setTheme(prefersDark ? 'dark' : 'light');

// 监听系统主题变化
window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
  setTheme(e.matches ? 'dark' : 'light');
});
```

### 6.5 Electron 窗口图标（桌面图标）

```javascript
// Electron BrowserWindow 配置
const path = require('path');

mainWindow = new BrowserWindow({
  width: 1200,
  height: 800,
  icon: path.join(__dirname, 'assets', 'icons', 'icon-color-256.png'),
  // icon 字段设置窗口左上角图标（部分平台生效）
  // 桌面任务栏图标也由此字段决定
  titleBarStyle: 'hidden', // 或 'default'
});

// 设置应用 dock 图标 (macOS)
if (process.platform === 'darwin') {
  app.dock.setIcon(path.join(__dirname, 'assets', 'icons', 'icon-color-256.png'));
}
```

---

## 7. 文件目录结构建议

```
plotvault-pdm/
├── assets/
│   └── icons/
│       ├── icon-color.svg          # 彩色版源文件（256×256 矢量）
│       ├── icon-mono.svg           # 单色版源文件（256×256 矢量）
│       ├── png/
│       │   ├── icon-color-16.png
│       │   ├── icon-color-32.png
│       │   ├── icon-color-48.png
│       │   ├── icon-color-64.png
│       │   ├── icon-color-128.png
│       │   ├── icon-color-256.png
│       │   ├── icon-mono-16.png
│       │   ├── icon-mono-20.png
│       │   └── icon-mono-32.png
│       ├── icon.ico                # Windows 多尺寸图标
│       └── icon.icns               # macOS 图标
├── src/
│   └── components/
│       └── TitleBar/
│           ├── TitleBar.tsx        # 标题栏组件（内联单色 SVG）
│           └── TitleBar.css        # 标题栏样式（含主题切换）
```

---

## 8. SVG 转 PNG/ICO 操作指南

### 8.1 使用 sharp（Node.js）将 SVG 转 PNG

```javascript
const sharp = require('sharp');
const fs = require('fs');

const svgBuffer = fs.readFileSync('assets/icons/icon-color.svg');

const sizes = [16, 32, 48, 64, 128, 256];

for (const size of sizes) {
  sharp(svgBuffer)
    .resize(size, size)
    .png()
    .toFile(`assets/icons/png/icon-color-${size}.png`)
    .then(() => console.log(`Generated ${size}px`))
    .catch(err => console.error(err));
}
```

### 8.2 生成 Windows .ico 文件

使用 `png-to-ico` 包：

```bash
npm install --save-dev png-to-ico
```

```javascript
const pngToIco = require('png-to-ico');
const fs = require('fs');

const pngFiles = [16, 32, 48, 256].map(size =>
  fs.readFileSync(`assets/icons/png/icon-color-${size}.png`)
);

pngToIco(pngFiles).then(buf => {
  fs.writeFileSync('assets/icons/icon.ico', buf);
});
```

---

## 9. 验收清单

实现完成后，检查以下项目：

- [ ] 桌面图标在深色桌面背景下清晰可见（彩色版自带背景，应无问题）
- [ ] 桌面图标在浅色桌面背景下清晰可见
- [ ] 窗口标题栏图标在深色主题下为白色（文件夹轮廓 + 钥匙孔）
- [ ] 窗口标题栏图标在浅色主题下为深色
- [ ] 切换主题时，标题栏图标颜色自动变化，无需手动刷新
- [ ] 图标在 16×16 像素下仍可辨识（文件夹轮廓 + 钥匙孔）
- [ ] 图标在 256×256 像素下细节完整（含图纸线条）
- [ ] 桌面图标使用彩色版，标题栏使用单色版，两者视觉风格统一

---

## 10. 附录：原始 SVG 文件路径

| 文件 | 路径 |
|------|------|
| 彩色版源文件 | `plotvault-logos/icon-5-drawing-folder.svg` |
| 单色版源文件 | `plotvault-logos/mono-5-drawing-folder.svg` |

将这两个 SVG 文件复制到项目的 `assets/icons/` 目录，分别重命名为 `icon-color.svg` 和 `icon-mono.svg`，然后按第 7 节的目录结构组织其余导出文件。
