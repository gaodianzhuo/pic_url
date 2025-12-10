use actix_files::NamedFile;
use actix_web::{get, web, App, HttpResponse, HttpServer, middleware, Result};
use image::imageops::FilterType;
use image::GenericImageView;
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

#[get("/")]
async fn index(config: web::Data<AppConfig>) -> HttpResponse {
    let pic_path = Path::new(config.pic_dir.as_str());
    let mut images: Vec<String> = Vec::new();
    collect_images(pic_path, pic_path, &mut images);
    images.sort();

    let image_items: String = images
        .iter()
        .map(|img| {
            format!(
                r#"<div class="image-item" onclick="openModal('/pic/{}', '{}')">
                    <img src="/thumb/{}" alt="{}" loading="lazy">
                    <div class="image-name">{}</div>
                </div>"#,
                img, img, img, img,
                Path::new(img).file_name().unwrap_or_default().to_string_lossy()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let empty_msg = format!(
        r#"<div class="empty-state">
            <h2>暂无图片</h2>
            <p>请将图片放入 {} 目录</p>
        </div>"#,
        config.pic_dir
    );

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>图床 - 图片浏览</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
            min-height: 100vh;
            padding: 20px;
        }}

        .header {{
            text-align: center;
            padding: 30px 0;
            color: #fff;
        }}

        .header h1 {{
            font-size: 2.5rem;
            margin-bottom: 10px;
            background: linear-gradient(90deg, #00d4ff, #7b2cbf);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }}

        .header p {{
            color: #8892b0;
            font-size: 1rem;
        }}

        .gallery {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
            gap: 20px;
            max-width: 1400px;
            margin: 0 auto;
            padding: 20px;
        }}

        .image-item {{
            background: rgba(255, 255, 255, 0.05);
            border-radius: 12px;
            overflow: hidden;
            cursor: pointer;
            transition: all 0.3s ease;
            border: 1px solid rgba(255, 255, 255, 0.1);
        }}

        .image-item:hover {{
            transform: translateY(-5px);
            box-shadow: 0 20px 40px rgba(0, 0, 0, 0.3);
            border-color: rgba(0, 212, 255, 0.3);
        }}

        .image-item img {{
            width: 100%;
            height: 180px;
            object-fit: cover;
            display: block;
            background: rgba(0, 0, 0, 0.2);
        }}

        .image-name {{
            padding: 12px;
            color: #ccd6f6;
            font-size: 0.85rem;
            text-align: center;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }}

        .modal {{
            display: none;
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: rgba(0, 0, 0, 0.95);
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
            max-height: 85vh;
            position: relative;
        }}

        .modal-content img {{
            max-width: 100%;
            max-height: 85vh;
            object-fit: contain;
            border-radius: 8px;
            box-shadow: 0 0 50px rgba(0, 0, 0, 0.5);
        }}

        .modal-close {{
            position: absolute;
            top: 20px;
            right: 30px;
            font-size: 40px;
            color: #fff;
            cursor: pointer;
            z-index: 1001;
            transition: color 0.3s;
        }}

        .modal-close:hover {{
            color: #00d4ff;
        }}

        .modal-info {{
            margin-top: 15px;
            color: #8892b0;
            text-align: center;
            font-size: 0.9rem;
        }}

        .modal-info a {{
            color: #00d4ff;
            text-decoration: none;
            margin-left: 10px;
        }}

        .modal-info a:hover {{
            text-decoration: underline;
        }}

        .empty-state {{
            text-align: center;
            padding: 60px 20px;
            color: #8892b0;
        }}

        .empty-state h2 {{
            font-size: 1.5rem;
            margin-bottom: 10px;
            color: #ccd6f6;
        }}

        .loading {{
            display: flex;
            justify-content: center;
            align-items: center;
            height: 180px;
            background: rgba(0, 0, 0, 0.2);
        }}

        .spinner {{
            width: 40px;
            height: 40px;
            border: 3px solid rgba(255, 255, 255, 0.1);
            border-top-color: #00d4ff;
            border-radius: 50%;
            animation: spin 1s linear infinite;
        }}

        @keyframes spin {{
            to {{ transform: rotate(360deg); }}
        }}

        @media (max-width: 768px) {{
            .gallery {{
                grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
                gap: 12px;
                padding: 10px;
            }}

            .image-item img {{
                height: 120px;
            }}

            .header h1 {{
                font-size: 1.8rem;
            }}
        }}
    </style>
</head>
<body>
    <div class="header">
        <h1>本地图床</h1>
        <p>共 {} 张图片 | 点击缩略图查看大图</p>
    </div>

    <div class="gallery">
        {}
    </div>

    {}

    <div class="modal" id="imageModal">
        <span class="modal-close" onclick="closeModal()">&times;</span>
        <div class="modal-content">
            <img id="modalImage" src="" alt="">
        </div>
        <div class="modal-info">
            <span id="modalFileName"></span>
            <a id="modalDownload" href="" download>下载原图</a>
            <a id="modalOpen" href="" target="_blank">新窗口打开</a>
        </div>
    </div>

    <script>
        function openModal(src, filename) {{
            const modal = document.getElementById('imageModal');
            const modalImg = document.getElementById('modalImage');
            const modalFileName = document.getElementById('modalFileName');
            const modalDownload = document.getElementById('modalDownload');
            const modalOpen = document.getElementById('modalOpen');

            modal.classList.add('active');
            modalImg.src = src;
            modalFileName.textContent = filename;
            modalDownload.href = src;
            modalOpen.href = src;

            document.body.style.overflow = 'hidden';
        }}

        function closeModal() {{
            const modal = document.getElementById('imageModal');
            modal.classList.remove('active');
            document.body.style.overflow = 'auto';
        }}

        document.getElementById('imageModal').addEventListener('click', function(e) {{
            if (e.target === this) {{
                closeModal();
            }}
        }});

        document.addEventListener('keydown', function(e) {{
            if (e.key === 'Escape') {{
                closeModal();
            }}
        }});
    </script>
</body>
</html>"#,
        images.len(),
        image_items,
        if images.is_empty() { empty_msg.as_str() } else { "" }
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

    let config_data = web::Data::new(app_config);

    HttpServer::new(move || {
        App::new()
            .app_data(config_data.clone())
            .wrap(middleware::Logger::default())
            .service(index)
            .service(serve_thumbnail)
            .service(serve_image)
    })
    .bind((host, args.port))?
    .run()
    .await
}
