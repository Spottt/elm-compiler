//! Unix REPL evaluations run in disposable process groups so SIGINT can cancel
//! compilation as well as JavaScript without committing the candidate session.
use nix::{
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use std::os::unix::process::CommandExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub struct Evaluation {
    interrupted: Arc<AtomicBool>,
    registration: signal_hook::SigId,
    request: serde_json::Value,
    request_path: PathBuf,
    output: PathBuf,
    node: PathBuf,
}
impl Evaluation {
    pub fn new(
        manifest: &Path,
        home: &Path,
        entry: &Path,
        node: &Path,
        ansi: bool,
    ) -> Result<Self, String> {
        let interrupted = Arc::new(AtomicBool::new(false));
        let registration =
            signal_hook::flag::register(signal_hook::consts::SIGINT, interrupted.clone())
                .map_err(|e| e.to_string())?;
        let output = entry.with_extension("js");
        Ok(Self {
            interrupted,
            registration,
            request: serde_json::json!({"manifest":manifest,"home":home,"entry":entry,"output":output,"ansi":ansi}),
            request_path: entry.with_extension("request.json"),
            output,
            node: node.into(),
        })
    }
    pub fn run(&self, source: &str, binding: Option<&str>) -> Result<(), String> {
        self.interrupted.store(false, Ordering::SeqCst);
        let mut request = self.request.clone();
        request["source"] = source.into();
        request["binding"] = binding.into();
        fs::write(
            &self.request_path,
            serde_json::to_vec(&request).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let mut compiler = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
        compiler
            .arg("--internal-repl-compile")
            .arg(&self.request_path)
            .stdin(Stdio::null());
        self.wait(compiler)?;
        if binding.is_some() {
            let mut node = Command::new(&self.node);
            node.stdin(fs::File::open(&self.output).map_err(|e| e.to_string())?);
            self.wait(node)?;
        }
        Ok(())
    }
    fn wait(&self, mut command: Command) -> Result<(), String> {
        if self.interrupted.load(Ordering::SeqCst) {
            println!("<cancelled>");
            return Err(String::new());
        }
        command
            .process_group(0)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        let mut child = EvaluationChild(command.spawn().map_err(|e| e.to_string())?);
        loop {
            if self.interrupted.load(Ordering::SeqCst) {
                println!("<cancelled>");
                return Err(String::new());
            }
            if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(String::new())
                };
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Evaluation {
    fn drop(&mut self) {
        signal_hook::low_level::unregister(self.registration);
    }
}
struct EvaluationChild(Child);
impl Drop for EvaluationChild {
    fn drop(&mut self) {
        // Kill descendants too, including a custom interpreter's children.
        let _ = killpg(Pid::from_raw(self.0.id() as i32), Signal::SIGKILL);
        let _ = self.0.wait();
    }
}

pub fn compile_request(path: &Path) -> Result<(), String> {
    let request: serde_json::Value =
        serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let string = |key| {
        request[key]
            .as_str()
            .ok_or_else(|| format!("missing REPL request {key}"))
    };
    let script = planexpo_elm::repl_compile::compile(
        Path::new(string("manifest")?),
        Path::new(string("home")?),
        Path::new(string("entry")?),
        string("source")?,
        request["binding"].as_str(),
        request["ansi"].as_bool().ok_or("missing REPL ansi flag")?,
    )
    .map_err(|error| {
        crate::diagnostic::repl_error(Path::new(string("entry").expect("validated entry")), error)
    })?;
    if let Some(script) = script {
        fs::write(string("output")?, script).map_err(|e| e.to_string())?;
    }
    Ok(())
}
