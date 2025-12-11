use actix_files::NamedFile;
use actix_web::{get, web, App, HttpResponse, HttpServer, middleware, Result};
use image::imageops::FilterType;
use image::GenericImageView;
use serde::Serialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const THUMB_SIZE: u32 = 200;

#[derive(Clone)]
struct AppConfig {
    pic_dir: Arc<String>,
    thumb_dir: Arc<String>,
}

impl AppConfig {
    fn new(pic_dir: String) -> Self {
        let thumb_dir = format!("{}/.thumbnails", pic_dir);
        Self {
            pic_dir: Arc::new(pic_dir),
            thumb_dir: Arc::new(thumb_dir),
        }
    }
}

#[derive(Serialize)]
struct ImageInfo {
    path: String,
    name: String,
}

#[derive(Serialize)]
struct ImageListResponse {
    count: usize,
    images: Vec<ImageInfo>,
}

fn is_image_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "ico")
    } else {
        false
    }
}

fn generate_thumbnail(src_path: &Path, thumb_path: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let img = image::open(src_path)?;
    let (width, height) = img.dimensions();

    let ratio = THUMB_SIZE as f32 / width.max(height) as f32;
    let new_width = (width as f32 * ratio) as u32;
    let new_height = (height as f32 * ratio) as u32;

    let thumbnail = img.resize(new_width, new_height, FilterType::Lanczos3);

    if let Some(parent) = thumb_path.parent() {
        fs::create_dir_all(parent)?;
    }

    thumbnail.save(thumb_path)?;
    Ok(())
}

fn get_thumbnail_path(thumb_dir: &str, relative_path: &str) -> PathBuf {
    Path::new(thumb_dir).join(relative_path)
}

fn ensure_thumbnail(thumb_dir: &str, src_path: &Path, relative_path: &str) -> Option<PathBuf> {
    let thumb_path = get_thumbnail_path(thumb_dir, relative_path);

    if thumb_path.exists() {
        if let (Ok(src_meta), Ok(thumb_meta)) = (fs::metadata(src_path), fs::metadata(&thumb_path)) {
            if let (Ok(src_time), Ok(thumb_time)) = (src_meta.modified(), thumb_meta.modified()) {
                if thumb_time >= src_time {
                    return Some(thumb_path);
                }
            }
        }
    }

    match generate_thumbnail(src_path, &thumb_path) {
        Ok(_) => Some(thumb_path),
        Err(e) => {
            eprintln!("Failed to generate thumbnail for {:?}: {}", src_path, e);
            None
        }
    }
}

#[get("/thumb/{path:.*}")]
async fn serve_thumbnail(
    path: web::Path<String>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let relative_path = path.into_inner();
    let src_path = Path::new(config.pic_dir.as_str()).join(&relative_path);

    if !src_path.exists() || !is_image_file(&src_path) {
        return Ok(HttpResponse::NotFound().body("Image not found"));
    }

    if let Some(thumb_path) = ensure_thumbnail(&config.thumb_dir, &src_path, &relative_path) {
        let data = fs::read(&thumb_path)?;
        let mime = mime_guess::from_path(&thumb_path).first_or_octet_stream();
        Ok(HttpResponse::Ok()
            .content_type(mime.to_string())
            .body(data))
    } else {
        Ok(HttpResponse::InternalServerError().body("Failed to generate thumbnail"))
    }
}

#[get("/pic/{path:.*}")]
async fn serve_image(
    path: web::Path<String>,
    config: web::Data<AppConfig>,
) -> Result<NamedFile> {
    let relative_path = path.into_inner();
    let file_path = Path::new(config.pic_dir.as_str()).join(&relative_path);
    Ok(NamedFile::open(file_path)?)
}

fn collect_images(dir: &Path, base: &Path, images: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().map(|n| n != ".thumbnails").unwrap_or(false) {
                    collect_images(&path, base, images);
                }
            } else if is_image_file(&path) {
                if let Ok(relative) = path.strip_prefix(base) {
                    images.push(relative.to_string_lossy().to_string());
                }
            }
        }
    }
}

#[get("/api/images")]
async fn api_images(config: web::Data<AppConfig>) -> HttpResponse {
    let pic_path = Path::new(config.pic_dir.as_str());
    let mut image_paths: Vec<String> = Vec::new();
    collect_images(pic_path, pic_path, &mut image_paths);
    image_paths.sort();

    let images: Vec<ImageInfo> = image_paths
        .iter()
        .map(|img| ImageInfo {
            path: img.clone(),
            name: Path::new(img)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        })
        .collect();

    let response = ImageListResponse {
        count: images.len(),
        images,
    };

    HttpResponse::Ok()
        .content_type("application/json")
        .json(response)
}

#[get("/")]
async fn index(config: web::Data<AppConfig>) -> HttpResponse {
    let pic_path = Path::new(config.pic_dir.as_str());
    let mut images: Vec<String> = Vec::new();
    collect_images(pic_path, pic_path, &mut images);
    images.sort();

    let image_items: String = images
        .iter()
        .map(|img| {
            let name = Path::new(img).file_name().unwrap_or_default().to_string_lossy();
            format!(
                r#"<div class="image-item" data-path="{}" onclick="openModal('/pic/{}', '{}')">
                    <img src="/thumb/{}" alt="{}" loading="lazy">
                    <div class="overlay"><div class="image-name">{}</div></div>
                </div>"#,
                img, img, img, img, img, name
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let empty_msg = format!(
        r#"<div class="empty-state" id="emptyState">
            <h2>No images</h2>
            <p>Add images to {}</p>
        </div>"#,
        config.pic_dir
    );

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Gallery</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0a0a0f;
            min-height: 100vh;
        }}

        .toolbar {{
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            height: 50px;
            background: rgba(15, 15, 20, 0.95);
            backdrop-filter: blur(10px);
            border-bottom: 1px solid rgba(255, 255, 255, 0.06);
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 0 24px;
            z-index: 100;
        }}

        .toolbar-left {{
            display: flex;
            align-items: center;
            gap: 12px;
        }}

        .status-indicator {{
            display: flex;
            align-items: center;
            gap: 8px;
            color: #64748b;
            font-size: 0.85rem;
        }}

        .status-dot {{
            width: 6px;
            height: 6px;
            background: #22c55e;
            border-radius: 50%;
            animation: pulse 2s infinite;
        }}

        @keyframes pulse {{
            0%, 100% {{ opacity: 1; }}
            50% {{ opacity: 0.4; }}
        }}

        .image-count {{
            color: #e2e8f0;
            font-weight: 500;
        }}

        .toolbar-right {{
            display: flex;
            align-items: center;
            gap: 16px;
            color: #64748b;
            font-size: 0.8rem;
        }}

        .size-toggle {{
            display: flex;
            gap: 4px;
            background: rgba(255, 255, 255, 0.05);
            padding: 4px;
            border-radius: 6px;
        }}

        .size-btn {{
            padding: 6px 12px;
            border: none;
            background: transparent;
            color: #64748b;
            font-size: 0.75rem;
            cursor: pointer;
            border-radius: 4px;
            transition: all 0.2s;
        }}

        .size-btn:hover {{
            color: #e2e8f0;
        }}

        .size-btn.active {{
            background: rgba(255, 255, 255, 0.1);
            color: #e2e8f0;
        }}

        .play-btn {{
            padding: 6px 14px;
            border: none;
            background: rgba(255, 255, 255, 0.05);
            color: #64748b;
            font-size: 0.75rem;
            cursor: pointer;
            border-radius: 6px;
            transition: all 0.2s;
            display: flex;
            align-items: center;
            gap: 6px;
        }}

        .play-btn:hover {{
            background: rgba(255, 255, 255, 0.1);
            color: #e2e8f0;
        }}

        .play-btn.playing {{
            background: rgba(34, 197, 94, 0.2);
            color: #22c55e;
        }}

        .play-icon {{
            font-size: 0.9rem;
        }}

        .gallery {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
            gap: 12px;
            padding: 70px 20px 20px 20px;
            max-width: 1800px;
            margin: 0 auto;
            transition: gap 0.3s;
        }}

        .gallery.size-large {{
            grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
            gap: 16px;
        }}

        .gallery.size-medium {{
            grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
            gap: 12px;
        }}

        .gallery.size-small {{
            grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
            gap: 8px;
        }}

        .gallery.size-small .overlay {{
            display: none;
        }}

        .image-item {{
            position: relative;
            aspect-ratio: 1;
            border-radius: 8px;
            overflow: hidden;
            cursor: pointer;
            background: #16161d;
            transition: transform 0.2s, box-shadow 0.2s;
        }}

        .image-item:hover {{
            transform: scale(1.02);
            box-shadow: 0 8px 30px rgba(0, 0, 0, 0.4);
        }}

        .image-item img {{
            width: 100%;
            height: 100%;
            object-fit: cover;
            display: block;
        }}

        .image-item .overlay {{
            position: absolute;
            bottom: 0;
            left: 0;
            right: 0;
            padding: 30px 10px 10px;
            background: linear-gradient(transparent, rgba(0,0,0,0.8));
            opacity: 0;
            transition: opacity 0.2s;
        }}

        .image-item:hover .overlay {{
            opacity: 1;
        }}

        .image-item .image-name {{
            color: #fff;
            font-size: 0.75rem;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }}

        .modal {{
            display: none;
            position: fixed;
            inset: 0;
            background: rgba(0, 0, 0, 0.98);
            z-index: 1000;
            justify-content: center;
            align-items: center;
            flex-direction: column;
        }}

        .modal.active {{
            display: flex;
        }}

        .modal-content {{
            max-width: 95vw;
            max-height: 90vh;
            position: relative;
        }}

        .modal-content img {{
            max-width: 100%;
            max-height: 90vh;
            object-fit: contain;
        }}

        .modal-close {{
            position: absolute;
            top: 20px;
            right: 24px;
            font-size: 32px;
            color: #94a3b8;
            cursor: pointer;
            z-index: 1001;
            transition: color 0.2s;
            font-weight: 300;
        }}

        .modal-close:hover {{
            color: #fff;
        }}

        .modal-nav {{
            position: absolute;
            top: 50%;
            transform: translateY(-50%);
            font-size: 48px;
            color: rgba(255, 255, 255, 0.5);
            cursor: pointer;
            padding: 20px;
            transition: color 0.2s;
            user-select: none;
            z-index: 1001;
        }}

        .modal-nav:hover {{
            color: #fff;
        }}

        .modal-nav.prev {{
            left: 10px;
        }}

        .modal-nav.next {{
            right: 10px;
        }}

        .modal-counter {{
            position: absolute;
            top: 20px;
            left: 24px;
            color: #94a3b8;
            font-size: 0.85rem;
            z-index: 1001;
        }}

        .slideshow-progress {{
            position: absolute;
            top: 0;
            left: 0;
            height: 3px;
            background: #22c55e;
            transition: width 0.1s linear;
            z-index: 1002;
        }}

        .modal-info {{
            position: absolute;
            bottom: 20px;
            left: 50%;
            transform: translateX(-50%);
            display: flex;
            align-items: center;
            gap: 20px;
            background: rgba(0, 0, 0, 0.6);
            backdrop-filter: blur(10px);
            padding: 12px 20px;
            border-radius: 8px;
        }}

        .modal-info span {{
            color: #e2e8f0;
            font-size: 0.85rem;
        }}

        .modal-info a {{
            color: #60a5fa;
            text-decoration: none;
            font-size: 0.85rem;
            transition: color 0.2s;
        }}

        .modal-info a:hover {{
            color: #93c5fd;
        }}

        .empty-state {{
            grid-column: 1 / -1;
            text-align: center;
            padding: 80px 20px;
            color: #64748b;
        }}

        .empty-state h2 {{
            font-size: 1.2rem;
            margin-bottom: 8px;
            color: #94a3b8;
            font-weight: 500;
        }}

        .toast {{
            position: fixed;
            bottom: 24px;
            left: 50%;
            transform: translateX(-50%);
            background: #1e293b;
            color: #e2e8f0;
            padding: 10px 20px;
            border-radius: 6px;
            font-size: 0.85rem;
            z-index: 2000;
            opacity: 0;
            transition: opacity 0.3s;
            border: 1px solid rgba(255, 255, 255, 0.1);
        }}

        .toast.show {{
            opacity: 1;
        }}

        @media (max-width: 768px) {{
            .gallery {{
                padding: 60px 10px 10px 10px;
            }}

            .gallery.size-large {{
                grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
            }}

            .gallery.size-medium {{
                grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
            }}

            .gallery.size-small {{
                grid-template-columns: repeat(auto-fill, minmax(90px, 1fr));
            }}

            .toolbar {{
                padding: 0 12px;
            }}

            .size-btn {{
                padding: 6px 10px;
            }}
        }}
    </style>
</head>
<body>
    <div class="toolbar">
        <div class="toolbar-left">
            <div class="status-indicator">
                <span class="status-dot"></span>
                <span class="image-count"><span id="imageCount">{}</span> images</span>
            </div>
        </div>
        <div class="toolbar-right">
            <button class="play-btn" id="playBtn" onclick="toggleSlideshow()">
                <span class="play-icon" id="playIcon">▶</span>
                <span id="playText">Play</span>
            </button>
            <div class="size-toggle">
                <button class="size-btn" data-size="large" onclick="setSize('large')">L</button>
                <button class="size-btn active" data-size="medium" onclick="setSize('medium')">M</button>
                <button class="size-btn" data-size="small" onclick="setSize('small')">S</button>
            </div>
        </div>
    </div>

    <div class="gallery size-medium" id="gallery">
        {}
    </div>

    {}

    <div class="modal" id="imageModal">
        <div class="slideshow-progress" id="slideshowProgress"></div>
        <span class="modal-counter" id="modalCounter"></span>
        <span class="modal-close" onclick="closeModal()">&times;</span>
        <span class="modal-nav prev" onclick="prevImage()">&#8249;</span>
        <span class="modal-nav next" onclick="nextImage()">&#8250;</span>
        <div class="modal-content">
            <img id="modalImage" src="" alt="">
        </div>
        <div class="modal-info">
            <span id="modalFileName"></span>
            <a id="modalDownload" href="" download>Download</a>
            <a id="modalOpen" href="" target="_blank">Open</a>
        </div>
    </div>

    <div class="toast" id="toast"></div>

    <script>
        let currentImages = new Set({});
        let imageList = [];
        let currentIndex = 0;
        let slideshowInterval = null;
        let progressInterval = null;
        let isPlaying = false;

        function updateImageList() {{
            imageList = Array.from(document.querySelectorAll('.image-item')).map(el => ({{
                path: el.dataset.path,
                name: el.querySelector('.image-name')?.textContent || el.dataset.path
            }}));
        }}

        function openModal(src, filename) {{
            updateImageList();
            currentIndex = imageList.findIndex(img => src.includes(img.path));
            if (currentIndex === -1) currentIndex = 0;
            showImage(currentIndex);
            document.getElementById('imageModal').classList.add('active');
            document.body.style.overflow = 'hidden';
        }}

        function showImage(index) {{
            if (imageList.length === 0) return;
            if (index < 0) index = imageList.length - 1;
            if (index >= imageList.length) index = 0;
            currentIndex = index;

            const img = imageList[currentIndex];
            const src = '/pic/' + img.path;

            document.getElementById('modalImage').src = src;
            document.getElementById('modalFileName').textContent = img.name;
            document.getElementById('modalDownload').href = src;
            document.getElementById('modalOpen').href = src;
            document.getElementById('modalCounter').textContent = `${{currentIndex + 1}} / ${{imageList.length}}`;
        }}

        function nextImage() {{
            showImage(currentIndex + 1);
            if (isPlaying) resetProgress();
        }}

        function prevImage() {{
            showImage(currentIndex - 1);
            if (isPlaying) resetProgress();
        }}

        function closeModal() {{
            document.getElementById('imageModal').classList.remove('active');
            document.body.style.overflow = 'auto';
            stopSlideshow();
        }}

        function toggleSlideshow() {{
            if (isPlaying) {{
                stopSlideshow();
            }} else {{
                startSlideshow();
            }}
        }}

        function startSlideshow() {{
            updateImageList();
            if (imageList.length === 0) {{
                showToast('No images');
                return;
            }}

            isPlaying = true;
            document.getElementById('playBtn').classList.add('playing');
            document.getElementById('playIcon').textContent = '⏸';
            document.getElementById('playText').textContent = 'Stop';

            if (!document.getElementById('imageModal').classList.contains('active')) {{
                currentIndex = 0;
                showImage(0);
                document.getElementById('imageModal').classList.add('active');
                document.body.style.overflow = 'hidden';
            }}

            resetProgress();
            slideshowInterval = setInterval(() => {{
                nextImage();
            }}, 3000);
        }}

        function stopSlideshow() {{
            isPlaying = false;
            document.getElementById('playBtn').classList.remove('playing');
            document.getElementById('playIcon').textContent = '▶';
            document.getElementById('playText').textContent = 'Play';
            document.getElementById('slideshowProgress').style.width = '0%';

            if (slideshowInterval) {{
                clearInterval(slideshowInterval);
                slideshowInterval = null;
            }}
            if (progressInterval) {{
                clearInterval(progressInterval);
                progressInterval = null;
            }}
        }}

        function resetProgress() {{
            if (progressInterval) clearInterval(progressInterval);
            let progress = 0;
            document.getElementById('slideshowProgress').style.width = '0%';
            progressInterval = setInterval(() => {{
                progress += 5;
                document.getElementById('slideshowProgress').style.width = progress + '%';
                if (progress >= 100) {{
                    clearInterval(progressInterval);
                }}
            }}, 100);
        }}

        document.getElementById('imageModal').addEventListener('click', function(e) {{
            if (e.target === this) {{
                closeModal();
            }}
        }});

        document.addEventListener('keydown', function(e) {{
            const modal = document.getElementById('imageModal');
            if (!modal.classList.contains('active')) return;

            if (e.key === 'Escape') {{
                closeModal();
            }} else if (e.key === 'ArrowRight' || e.key === ' ') {{
                e.preventDefault();
                nextImage();
            }} else if (e.key === 'ArrowLeft') {{
                prevImage();
            }}
        }});

        function showToast(message) {{
            const toast = document.getElementById('toast');
            toast.textContent = message;
            toast.classList.add('show');
            setTimeout(() => toast.classList.remove('show'), 3000);
        }}

        function setSize(size) {{
            const gallery = document.getElementById('gallery');
            gallery.classList.remove('size-large', 'size-medium', 'size-small');
            gallery.classList.add('size-' + size);

            document.querySelectorAll('.size-btn').forEach(btn => {{
                btn.classList.toggle('active', btn.dataset.size === size);
            }});

            localStorage.setItem('gallery-size', size);
        }}

        // 恢复保存的尺寸设置
        (function() {{
            const savedSize = localStorage.getItem('gallery-size');
            if (savedSize) {{
                setSize(savedSize);
            }}
        }})();

        function createImageElement(img) {{
            const div = document.createElement('div');
            div.className = 'image-item';
            div.setAttribute('data-path', img.path);
            div.onclick = () => openModal('/pic/' + img.path, img.path);
            div.innerHTML = `
                <img src="/thumb/${{img.path}}" alt="${{img.path}}" loading="lazy">
                <div class="overlay"><div class="image-name">${{img.name}}</div></div>
            `;
            return div;
        }}

        async function checkForUpdates() {{
            try {{
                const response = await fetch('/api/images');
                const data = await response.json();
                const newImages = new Set(data.images.map(img => img.path));

                // 检查新增的图片
                const added = data.images.filter(img => !currentImages.has(img.path));

                // 检查删除的图片
                const removed = [...currentImages].filter(path => !newImages.has(path));

                if (added.length > 0 || removed.length > 0) {{
                    const gallery = document.getElementById('gallery');
                    const emptyState = document.getElementById('emptyState');

                    // 添加新图片
                    added.forEach(img => {{
                        const element = createImageElement(img);
                        gallery.appendChild(element);
                    }});

                    // 删除已移除的图片
                    removed.forEach(path => {{
                        const element = gallery.querySelector(`[data-path="${{path}}"]`);
                        if (element) {{
                            element.remove();
                        }}
                    }});

                    // 更新计数
                    document.getElementById('imageCount').textContent = data.count;
                    currentImages = newImages;

                    // 处理空状态
                    if (data.count === 0 && !emptyState) {{
                        gallery.innerHTML = `<div class="empty-state" id="emptyState">
                            <h2>No images</h2>
                            <p>Add images to the directory</p>
                        </div>`;
                    }} else if (data.count > 0 && emptyState) {{
                        emptyState.remove();
                    }}

                    // 显示提示
                    if (added.length > 0) {{
                        showToast(`+${{added.length}} image${{added.length > 1 ? 's' : ''}}`);
                    }}
                    if (removed.length > 0) {{
                        showToast(`-${{removed.length}} image${{removed.length > 1 ? 's' : ''}}`);
                    }}
                }}
            }} catch (error) {{
                console.error('检查更新失败:', error);
            }}
        }}

        // 每 3 秒检查一次更新
        setInterval(checkForUpdates, 3000);
    </script>
</body>
</html>"#,
        images.len(),
        image_items,
        if images.is_empty() { empty_msg.as_str() } else { "" },
        serde_json::to_string(&images).unwrap_or_else(|_| "[]".to_string())
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

fn print_usage() {
    println!("用法: pic_url [选项]");
    println!();
    println!("选项:");
    println!("  -p, --port <端口>      设置服务端口 (默认: 2020)");
    println!("  -d, --dir <目录>       设置图片目录 (默认: ./pic)");
    println!("  -h, --help             显示帮助信息");
    println!();
    println!("环境变量:");
    println!("  PIC_PORT               设置服务端口");
    println!("  PIC_DIR                设置图片目录");
    println!();
    println!("示例:");
    println!("  pic_url                        使用默认配置");
    println!("  pic_url -p 8080                使用端口 8080");
    println!("  pic_url -d /home/user/images   指定图片目录");
    println!("  pic_url -p 8080 -d ./photos    同时指定端口和目录");
    println!("  PIC_PORT=9000 PIC_DIR=/data pic_url  通过环境变量配置");
}

struct Config {
    port: u16,
    pic_dir: String,
}

fn parse_args() -> Config {
    let args: Vec<String> = env::args().collect();
    let default_port: u16 = 2020;
    let default_dir = String::from("./pic");

    // 检查帮助参数
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        std::process::exit(0);
    }

    let mut port: Option<u16> = None;
    let mut pic_dir: Option<String> = None;

    // 从命令行参数解析
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--port" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u16>() {
                        Ok(p) if p > 0 => port = Some(p),
                        Ok(_) => {
                            eprintln!("错误: 端口必须大于 0");
                            std::process::exit(1);
                        }
                        Err(_) => {
                            eprintln!("错误: 无效的端口号 '{}'", args[i + 1]);
                            std::process::exit(1);
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("错误: -p/--port 需要指定端口号");
                    std::process::exit(1);
                }
            }
            "-d" | "--dir" => {
                if i + 1 < args.len() {
                    pic_dir = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("错误: -d/--dir 需要指定目录路径");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("错误: 未知参数 '{}'", args[i]);
                eprintln!("使用 --help 查看帮助信息");
                std::process::exit(1);
            }
        }
    }

    // 从环境变量解析（命令行参数优先）
    if port.is_none() {
        if let Ok(port_str) = env::var("PIC_PORT") {
            match port_str.parse::<u16>() {
                Ok(p) if p > 0 => port = Some(p),
                Ok(_) => {
                    eprintln!("错误: 环境变量 PIC_PORT 必须大于 0");
                    std::process::exit(1);
                }
                Err(_) => {
                    eprintln!("错误: 环境变量 PIC_PORT 无效: '{}'", port_str);
                    std::process::exit(1);
                }
            }
        }
    }

    if pic_dir.is_none() {
        if let Ok(dir) = env::var("PIC_DIR") {
            pic_dir = Some(dir);
        }
    }

    Config {
        port: port.unwrap_or(default_port),
        pic_dir: pic_dir.unwrap_or(default_dir),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let host = "0.0.0.0";
    let args = parse_args();
    let app_config = AppConfig::new(args.pic_dir.clone());

    // 确保图片目录存在
    if !Path::new(&args.pic_dir).exists() {
        fs::create_dir_all(&args.pic_dir)?;
        println!("已创建图片目录: {}", args.pic_dir);
    }

    // 确保缩略图目录存在
    if !Path::new(app_config.thumb_dir.as_str()).exists() {
        fs::create_dir_all(app_config.thumb_dir.as_str())?;
        println!("已创建缩略图目录: {}", app_config.thumb_dir);
    }

    println!("本地图床已启动");
    println!("图片目录: {}", args.pic_dir);
    println!("缩略图目录: {}", app_config.thumb_dir);
    println!("访问地址: http://{}:{}/", host, args.port);
    println!("自动刷新: 已启用 (每 3 秒检查)");

    let config_data = web::Data::new(app_config);

    HttpServer::new(move || {
        App::new()
            .app_data(config_data.clone())
            .wrap(middleware::Logger::default())
            .service(index)
            .service(api_images)
            .service(serve_thumbnail)
            .service(serve_image)
    })
    .bind((host, args.port))?
    .run()
    .await
}
