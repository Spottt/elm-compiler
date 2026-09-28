//! Elm 0.19.1's Develop server and original Elm frontend (see LICENSE-ELM).
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};
use tiny_http::{Header, Response, Server};

pub fn run(args: Vec<String>) -> Result<(), String> {
    use std::io::IsTerminal;
    let ansi = std::io::stderr().is_terminal();
    let Some(port) = crate::reactor_cli::parse(args, ansi)? else {
        eprint!("{}", crate::reactor_cli::help(ansi));
        return Ok(());
    };
    println!("Go to http://localhost:{port} to see your project dashboard.");
    let port = u16::try_from(port).map_err(|_| format!("invalid reactor service port: {port}"))?;
    let root = env::current_dir()
        .and_then(|p| p.canonicalize())
        .map_err(|e| e.to_string())?;
    let home = env::var_os("ELM_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".elm")))
        .ok_or("ELM_HOME or HOME is required")?;
    let context = Arc::new(Context {
        root,
        home,
        compiler: env::current_exe().map_err(|e| e.to_string())?,
        compilation: Mutex::new(()),
    });
    let server = Arc::new(Server::http(("0.0.0.0", port)).map_err(|e| e.to_string())?);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let server = Arc::clone(&server);
            let context = Arc::clone(&context);
            scope.spawn(move || {
                for request in server.incoming_requests() {
                    let page = context.serve(request.url()).unwrap_or_else(|error| Page {
                        status: 500,
                        mime: "text/html;charset=utf-8",
                        location: None,
                        body: page_html("Errors", Some(&crate::diagnostic::report(&error)))
                            .into_bytes(),
                    });
                    let mut response = Response::from_data(page.body).with_status_code(page.status);
                    if !page.mime.is_empty() {
                        response.add_header(
                            Header::from_bytes("Content-Type", page.mime)
                                .expect("static content type"),
                        );
                    }
                    if let Some(location) = page.location
                        && let Ok(header) = Header::from_bytes("Location", location)
                    {
                        response.add_header(header);
                    }
                    let _ = request.respond(response);
                }
            });
        }
    });
    Ok(())
}

struct Context {
    root: PathBuf,
    home: PathBuf,
    compiler: PathBuf,
    compilation: Mutex<()>,
}
struct Page {
    status: u16,
    mime: &'static str,
    body: Vec<u8>,
    location: Option<String>,
}
impl Page {
    fn elm_html(body: String) -> Self {
        Self {
            mime: "text/html",
            ..Self::html(body)
        }
    }
    fn html(body: String) -> Self {
        Self {
            status: 200,
            mime: "text/html;charset=utf-8",
            body: body.into_bytes(),
            location: None,
        }
    }
}

impl Context {
    fn manifest(&self) -> Option<PathBuf> {
        self.root
            .ancestors()
            .map(|path| path.join("elm.json"))
            .find(|path| path.is_file())
    }
    fn serve(&self, url: &str) -> Result<Page, String> {
        let not_found = || Page {
            status: 404,
            ..Page::html(page_html("NotFound", None))
        };
        let Some(relative) = safe_path(url) else {
            return Ok(not_found());
        };
        let path = self.root.join(&relative);
        if let Ok(canonical) = path.canonicalize() {
            if !canonical.starts_with(&self.root) {
                return Ok(not_found());
            }
            if canonical.is_dir() {
                if !url.split('?').next().unwrap_or(url).ends_with('/') {
                    let (path, query) = url
                        .split_once('?')
                        .map_or((url, None), |(a, b)| (a, Some(b)));
                    let location = format!(
                        "{path}/{}",
                        query.map(|q| format!("?{q}")).unwrap_or_default()
                    );
                    return Ok(Page {
                        status: 302,
                        mime: "",
                        location: Some(location),
                        ..Page::html(String::new())
                    });
                }
                return Ok(Page::html(page_html(
                    "Index",
                    Some(&self.index(&relative)?),
                )));
            }
            if canonical.is_file() {
                if path.extension().is_some_and(|ext| ext == "elm") {
                    return self.compile(&path);
                }
                let bytes = fs::read(&path).map_err(|e| e.to_string())?;
                if let Some(mime) = mime_type(&relative) {
                    return Ok(Page {
                        status: 200,
                        mime,
                        body: bytes,
                        location: None,
                    });
                }
                return Ok(Page::elm_html(code_html(
                    &format!("~/{relative}"),
                    &String::from_utf8_lossy(&bytes),
                )));
            }
        }
        if let Some((body, mime)) = asset(&relative) {
            return Ok(Page {
                status: 200,
                mime,
                body: body.to_vec(),
                location: None,
            });
        }
        Ok(not_found())
    }

    fn index(&self, relative: &str) -> Result<Value, String> {
        let directory = self.root.join(relative);
        let mut dirs = vec![".".to_string(), "..".to_string()];
        let mut files = Vec::new();
        for entry in fs::read_dir(&directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                dirs.push(name);
            } else if path.is_file() {
                let runnable = path.extension().is_some_and(|ext| ext == "elm")
                    && fs::read_to_string(&path).is_ok_and(|source| source.contains("\nmain "));
                files.push(json!({"name":name,"runnable":runnable}));
            }
        }
        let manifest = self.manifest();
        let outline = manifest.as_ref().and_then(|path| {
            let source = fs::read_to_string(path).ok()?;
            let config = planexpo_elm::outline::decode(&source).ok()?;
            planexpo_elm::outline::validate(&config, path.parent()?).ok()?;
            Some(config)
        });
        let exact = if outline
            .as_ref()
            .is_some_and(|value| value["type"] == "package")
        {
            planexpo_elm::package_resolution::resolve(manifest.as_ref().unwrap(), &self.home)
                .unwrap_or_default()
        } else {
            Default::default()
        };
        Ok(
            json!({"root":self.root,"pwd":relative.split('/').filter(|s| !s.is_empty()).collect::<Vec<_>>(),
            "dirs":dirs,"files":files,"readme":fs::read_to_string(directory.join("README.md")).ok(),
            "outline":outline,"exactDeps":exact}),
        )
    }

    fn compile(&self, path: &Path) -> Result<Page, String> {
        if self.manifest().is_none() {
            return Ok(Page::elm_html(page_html(
                "Errors",
                Some(
                    &json!({"type":"error","path":null,"title":"NEW PROJECT?","message":[
                        "Are you trying to start a new project? Try this command in the terminal:\n\n    ",
                        {"bold":false,"underline":false,"color":"GREEN","string":"elm init"},
                        "\n\nIt will help you get started!"
                    ]}),
                ),
            )));
        }
        let _lock = self
            .compilation
            .lock()
            .map_err(|_| "reactor compilation lock poisoned")?;
        let output = tempfile::Builder::new()
            .suffix(".html")
            .tempfile()
            .map_err(|e| e.to_string())?;
        let result = Command::new(&self.compiler)
            .current_dir(&self.root)
            .args(["make", "--report=json", "--output"])
            .arg(output.path())
            .arg(path)
            .output()
            .map_err(|e| e.to_string())?;
        if result.status.success() {
            Ok(Page {
                body: fs::read(output.path()).map_err(|e| e.to_string())?,
                ..Page::elm_html(String::new())
            })
        } else {
            let diagnostic = serde_json::from_slice(&result.stderr).unwrap_or_else(|_| {
                crate::diagnostic::report(&String::from_utf8_lossy(&result.stderr))
            });
            Ok(Page::elm_html(page_html("Errors", Some(&diagnostic))))
        }
    }
}

fn safe_path(url: &str) -> Option<String> {
    let raw = url.split('?').next()?.strip_prefix('/')?;
    let mut bytes = Vec::new();
    let mut chars = raw.bytes();
    while let Some(byte) = chars.next() {
        bytes.push(if byte == b'%' {
            let a = (chars.next()? as char).to_digit(16)?;
            let b = (chars.next()? as char).to_digit(16)?;
            (a * 16 + b) as u8
        } else {
            byte
        });
    }
    let path = String::from_utf8(bytes).ok()?;
    if path.contains(['\0', '\\'])
        || Path::new(&path)
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    Some(
        Path::new(&path)
            .components()
            .filter_map(|part| match part {
                Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn page_html(module: &str, flags: Option<&Value>) -> String {
    let flags = flags
        .map_or_else(|| "undefined".into(), Value::to_string)
        .replace("</", "<\\/");
    format!(
        "<!DOCTYPE HTML>\n<html>\n<head>\n  <meta charset=\"UTF-8\">\n  <link type=\"text/css\" rel=\"stylesheet\" href=\"/_elm/styles.css\">\n  <script src=\"/_elm/elm.js\"></script>\n</head>\n<body>\n<script>\nElm.{module}.init({{ flags: {flags} }});\n</script>\n</body>\n</html>\n"
    )
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn code_html(title: &str, source: &str) -> String {
    format!(
        r#"<!DOCTYPE HTML>
<html>
<head>
  <meta charset="UTF-8">
  <title>{}</title>
  <style type="text/css">
    @import url(/_elm/source-code-pro.ttf);
    html, head, body, pre {{ margin: 0; height: 100%; }}
    body {{ font-family: "Source Code Pro", monospace; }}
  </style>
  <link type="text/css" rel="stylesheet" href="//cdnjs.cloudflare.com/ajax/libs/highlight.js/9.3.0/styles/default.min.css">
  <script src="//cdnjs.cloudflare.com/ajax/libs/highlight.js/9.3.0/highlight.min.js"></script>
  <script>if (hljs) {{ hljs.initHighlightingOnLoad(); }}</script>
</head>
<body style="background-color: #F0F0F0;">
<pre><code>{}</code></pre>
</body>
</html>
"#,
        escape(title),
        escape(source)
    )
}

fn asset(path: &str) -> Option<(&'static [u8], &'static str)> {
    Some(match path {
        "_elm/elm.js" => (
            include_bytes!("../reactor/assets/elm.js"),
            "application/javascript;charset=utf-8",
        ),
        "_elm/styles.css" => (
            include_bytes!("../reactor/assets/styles.css"),
            "text/css;charset=utf-8",
        ),
        "_elm/source-code-pro.ttf" => (
            include_bytes!("../reactor/assets/source-code-pro.ttf"),
            "font/ttf;charset=utf-8",
        ),
        "_elm/source-sans-pro.ttf" => (
            include_bytes!("../reactor/assets/source-sans-pro.ttf"),
            "font/ttf;charset=utf-8",
        ),
        "favicon.ico" => (
            include_bytes!("../reactor/assets/favicon.ico"),
            "image/x-icon;charset=utf-8",
        ),
        _ => return None,
    })
}
fn mime_type(path: &str) -> Option<&'static str> {
    MIME_TYPES
        .iter()
        .find_map(|(extension, mime)| path.ends_with(extension).then_some(*mime))
}
const MIME_TYPES: &[(&str, &str)] = &[
    (".tar.bz2", "application/x-bzip-compressed-tar"),
    (".tar.gz", "application/x-tgz"),
    (".woff2", "font/woff2"),
    (".html", "text/html"),
    (".jpeg", "image/jpeg"),
    (".json", "application/json"),
    (".mpeg", "video/mpeg"),
    (".sfnt", "font/sfnt"),
    (".text", "text/plain"),
    (".webm", "video/webm"),
    (".webp", "image/webp"),
    (".woff", "font/woff"),
    (".asc", "text/plain"),
    (".asf", "video/x-ms-asf"),
    (".asx", "video/x-ms-asf"),
    (".avi", "video/x-msvideo"),
    (".bz2", "application/x-bzip"),
    (".css", "text/css"),
    (".dtd", "text/xml"),
    (".dvi", "application/x-dvi"),
    (".gif", "image/gif"),
    (".htm", "text/html"),
    (".ico", "image/x-icon"),
    (".jpg", "image/jpeg"),
    (".m3u", "audio/x-mpegurl"),
    (".mov", "video/quicktime"),
    (".mp3", "audio/mpeg"),
    (".mp4", "video/mp4"),
    (".mpg", "video/mpeg"),
    (".ogg", "application/ogg"),
    (".otf", "font/otf"),
    (".pac", "application/x-ns-proxy-autoconfig"),
    (".pdf", "application/pdf"),
    (".png", "image/png"),
    (".sig", "application/pgp-signature"),
    (".spl", "application/futuresplash"),
    (".svg", "image/svg+xml"),
    (".swf", "application/x-shockwave-flash"),
    (".tar", "application/x-tar"),
    (".tbz", "application/x-bzip-compressed-tar"),
    (".tgz", "application/x-tgz"),
    (".ttf", "font/ttf"),
    (".txt", "text/plain"),
    (".wav", "audio/x-wav"),
    (".wax", "audio/x-ms-wax"),
    (".wma", "audio/x-ms-wma"),
    (".wmv", "video/x-ms-wmv"),
    (".xbm", "image/x-xbitmap"),
    (".xml", "text/xml"),
    (".xpm", "image/x-xpixmap"),
    (".xwd", "image/x-xwindowdump"),
    (".zip", "application/zip"),
    (".gz", "application/x-gzip"),
    (".js", "text/javascript"),
    (".qt", "video/quicktime"),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_decode_without_escaping_the_served_directory() {
        assert_eq!(
            safe_path("/a%20b/Main.elm?debug=true"),
            Some("a b/Main.elm".into())
        );
        assert_eq!(safe_path("/a/./b/"), Some("a/b".into()));
        for path in [
            "/../secret",
            "/%2e%2e/secret",
            "/%00",
            "/%",
            "/%ff",
            "/%2fetc/passwd",
            "/a%5c..%5cb",
        ] {
            assert!(safe_path(path).is_none(), "{path}");
        }
    }
    #[test]
    fn embedded_data_and_source_cannot_close_their_html_container() {
        assert!(
            !page_html(
                "Index",
                Some(&json!({"readme":"</script><script>evil()</script>"}))
            )
            .contains("</script><script>evil")
        );
        assert!(code_html("file", "<script>evil()</script>").contains("&lt;script&gt;"));
    }
}
