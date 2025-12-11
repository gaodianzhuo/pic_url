# PicURL - 本地图床服务器

一个轻量级的本地图床服务器，使用 Rust + Actix-web 构建，支持缩略图预览和大图查看。

![ui](./ui.png)

## 功能特性

### 核心功能

- **缩略图预览** - 所有图片以缩略图网格形式展示，加载快速
- **点击查看大图** - 点击缩略图弹出模态框显示原图
- **幻灯片播放** - 自动循环播放所有图片，每 3 秒切换一张
- **三种尺寸切换** - 支持大 (L)、中 (M)、小 (S) 三种展示尺寸，设置自动保存
- **自动缩略图生成** - 首次访问时自动生成并缓存缩略图
- **智能缓存** - 缩略图带时间戳验证，源文件更新后自动重新生成
- **子目录支持** - 递归扫描 pic 目录下所有子文件夹中的图片
- **懒加载** - 图片使用浏览器原生懒加载，提升页面性能
- **自动刷新** - 每 3 秒检测目录变化，新增/删除图片自动更新页面（无需刷新）

### 支持的图片格式

| 格式 | 扩展名 |
|------|--------|
| JPEG | `.jpg`, `.jpeg` |
| PNG | `.png` |
| GIF | `.gif` |
| WebP | `.webp` |
| BMP | `.bmp` |
| ICO | `.ico` |

### 界面功能

- 极简工具栏设计，固定在页面顶部
- 响应式布局，适配桌面和移动设备
- 深色主题 UI，护眼舒适
- 三种图片尺寸：
  - **L (Large)** - 大尺寸，每张最小 300px
  - **M (Medium)** - 中等尺寸，每张最小 200px（默认）
  - **S (Small)** - 小尺寸，每张最小 120px
- 尺寸设置自动保存到浏览器，刷新后保持
- 悬停显示文件名（小尺寸模式下隐藏）
- 模态框支持多种关闭方式：
  - 点击右上角 × 按钮
  - 点击背景区域
  - 按 ESC 键
- 提供原图下载链接
- 支持新窗口打开原图

## 安装

### 前置要求

- Rust 1.70+ (推荐使用 rustup 安装)
- Cargo 包管理器

### 编译

```bash
# 克隆或进入项目目录
cd pic_server

# 编译 release 版本（推荐）
cargo build --release

# 编译后的可执行文件位于
./target/release/pic_url
```

## 使用方法

### 基本使用

```bash
# 使用默认端口 2020 启动
./target/release/pic_url

# 或者使用 cargo run
cargo run --release
```

启动后访问 http://localhost:2020 即可查看图片画廊。

### 自定义配置

支持通过命令行参数或环境变量配置端口和图片目录。

#### 1. 命令行参数

```bash
# 指定端口
./pic_url -p 8080
./pic_url --port 3000

# 指定图片目录
./pic_url -d /home/user/images
./pic_url --dir ./photos

# 同时指定端口和目录
./pic_url -p 8080 -d /data/pictures
```

#### 2. 环境变量

```bash
# 设置端口
PIC_PORT=9000 ./pic_url

# 设置图片目录
PIC_DIR=/home/user/images ./pic_url

# 同时设置
PIC_PORT=9000 PIC_DIR=/data ./pic_url

# 或者先导出环境变量
export PIC_PORT=9000
export PIC_DIR=/data/pictures
./pic_url
```

#### 3. 查看帮助

```bash
./pic_url --help
./pic_url -h
```

输出：

```
用法: pic_url [选项]

选项:
  -p, --port <端口>      设置服务端口 (默认: 2020)
  -d, --dir <目录>       设置图片目录 (默认: ./pic)
  -h, --help             显示帮助信息

环境变量:
  PIC_PORT               设置服务端口
  PIC_DIR                设置图片目录

示例:
  pic_url                        使用默认配置
  pic_url -p 8080                使用端口 8080
  pic_url -d /home/user/images   指定图片目录
  pic_url -p 8080 -d ./photos    同时指定端口和目录
  PIC_PORT=9000 PIC_DIR=/data pic_url  通过环境变量配置
```

**优先级**：命令行参数 > 环境变量 > 默认值

### 添加图片

将图片文件放入图片目录即可（默认 `./pic`，可通过 `-d` 参数自定义），支持创建子目录组织图片：

```
<图片目录>/
├── photo1.jpg
├── photo2.png
├── .thumbnails/     # 自动生成的缩略图缓存（勿删除）
├── 旅行/
│   ├── 北京.jpg
│   └── 上海.png
└── 截图/
    └── screen1.png
```

**注意**：`.thumbnails` 目录由程序自动创建和管理，用于缓存缩略图。

## API 路由

| 路径 | 方法 | 说明 |
|------|------|------|
| `/` | GET | 图片画廊首页，显示所有图片的缩略图 |
| `/api/images` | GET | 获取图片列表 JSON（用于自动刷新） |
| `/thumb/{path}` | GET | 获取指定图片的缩略图 |
| `/pic/{path}` | GET | 获取原始图片文件 |

### 示例

```bash
# 访问首页
curl http://localhost:2020/

# 获取图片列表 JSON
curl http://localhost:2020/api/images

# 获取缩略图
curl http://localhost:2020/thumb/photo1.jpg

# 获取原图
curl http://localhost:2020/pic/photo1.jpg

# 访问子目录中的图片
curl http://localhost:2020/pic/旅行/北京.jpg
```

`/api/images` 返回格式：

```json
{
  "count": 3,
  "images": [
    {"path": "photo1.jpg", "name": "photo1.jpg"},
    {"path": "photo2.png", "name": "photo2.png"},
    {"path": "旅行/北京.jpg", "name": "北京.jpg"}
  ]
}
```

## 目录结构

```
pic_server/
├── Cargo.toml          # 项目配置和依赖
├── Cargo.lock          # 依赖版本锁定
├── README.md           # 本文档
├── src/
│   └── main.rs         # 主程序源码
├── pic/                # 图片存储目录（自动创建）
│   └── .thumbnails/    # 缩略图缓存目录（自动创建）
└── target/             # 编译输出目录
    └── release/
        └── pic_url     # 可执行文件
```

## 技术栈

| 组件 | 技术 | 版本 |
|------|------|------|
| 语言 | Rust | 2021 Edition |
| Web 框架 | Actix-web | 4.x |
| 文件服务 | Actix-files | 0.6 |
| 异步运行时 | Tokio | 1.x |
| 图片处理 | image | 0.25 |
| MIME 类型 | mime_guess | 2.0 |

## 配置参数

| 参数 | 默认值 | 可配置 | 说明 |
|------|--------|--------|------|
| 监听地址 | `0.0.0.0` | 否 | 监听所有网络接口 |
| 端口 | `2020` | 是 | HTTP 服务端口 (`-p` / `PIC_PORT`) |
| 图片目录 | `./pic` | 是 | 图片存储路径 (`-d` / `PIC_DIR`) |
| 缩略图目录 | `<图片目录>/.thumbnails` | 自动 | 缩略图缓存路径 |
| 缩略图尺寸 | `200px` | 否 | 缩略图最大边长 |

## 性能优化

1. **缩略图缓存** - 生成的缩略图保存到磁盘，避免重复计算
2. **时间戳验证** - 只有源文件更新时才重新生成缩略图
3. **懒加载** - 使用浏览器原生 `loading="lazy"` 属性
4. **高质量缩放** - 使用 Lanczos3 算法生成高质量缩略图
5. **异步 I/O** - 基于 Tokio 异步运行时，支持高并发

## 常见问题

### Q: 如何修改缩略图大小？

修改 `src/main.rs` 中的 `THUMB_SIZE` 常量：

```rust
const THUMB_SIZE: u32 = 200;  // 改为你想要的尺寸
```

然后重新编译并删除 `.thumbnails` 目录以重新生成缩略图。

### Q: 如何清除缩略图缓存？

```bash
# 默认目录
rm -rf ./pic/.thumbnails

# 自定义目录
rm -rf /your/pic/dir/.thumbnails
```

下次访问时会自动重新生成。

### Q: 支持上传图片吗？

当前版本不支持 Web 上传，需要手动将图片放入图片目录（默认 `./pic`）。

### Q: 如何后台运行？

```bash
# 使用 nohup（指定端口和目录）
nohup ./pic_url -p 8080 -d /data/images > pic_url.log 2>&1 &

# 或使用 systemd 服务（推荐生产环境）
```

### Q: 如何限制访问？

当前版本监听 `0.0.0.0`，局域网内所有设备都可访问。如需限制，可以：
- 使用防火墙规则
- 配合 Nginx 反向代理添加认证

## 许可证

MIT License

## 更新日志

### v0.5.0

- 新增幻灯片播放功能，每 3 秒自动切换一张图片
- 播放控制：点击 Play 按钮开始播放，Stop 停止
- 进度条显示当前播放进度
- 支持左右箭头导航切换上一张/下一张
- 显示当前图片序号（如 "3 / 10"）
- 键盘快捷键支持：
  - `←` / `→` 切换图片
  - `Space` 暂停/继续播放
  - `Escape` 关闭模态框
- 循环播放：最后一张后自动回到第一张

### v0.4.0

- 新增图片尺寸切换功能（L/M/S 三种尺寸）
- 尺寸设置自动保存到 localStorage
- 界面优化：极简工具栏，去除大标题
- 图片卡片改为正方形，悬停显示文件名
- 小尺寸模式下隐藏文件名遮罩
- 移动端适配优化

### v0.3.0

- 新增自动刷新功能，每 3 秒检测目录变化
- 新增/删除图片时页面自动更新，无需手动刷新
- 新增 `/api/images` 接口返回图片列表 JSON
- 新增 Toast 提示显示图片变化
- 页面显示实时状态指示器

### v0.2.0

- 新增自定义图片目录功能 (`-d` / `--dir` / `PIC_DIR`)
- 缩略图目录自动跟随图片目录
- 完善命令行参数解析和错误提示

### v0.1.0

- 初始版本
- 支持缩略图预览和大图查看
- 支持自定义端口 (`-p` / `--port` / `PIC_PORT`)
- 响应式深色主题 UI
- 自动生成和缓存缩略图
- 支持多种图片格式（JPG、PNG、GIF、WebP、BMP、ICO）
