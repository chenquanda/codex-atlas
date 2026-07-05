use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

use crate::{
    domain::Ability,
    skilldoc::{self, SkillDocError, SkillFileContent, SkillFileLanguage},
    util::{self, DataPathError},
};

pub const TRANSLATION_CACHE_FILE: &str = "translation-cache.json";
const CODEX_TRANSLATION_TIMEOUT_MILLIS: u64 = 120_000;
const WAIT_POLL_MILLIS: u64 = 100;
const COMMAND_TEXT_LIMIT: usize = 4096;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranslationState {
    pub skill_id: String,
    pub relative_path: String,
    pub content_hash: String,
    pub show_translate_action: bool,
    pub cached_translation: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranslationResult {
    pub skill_id: String,
    pub relative_path: String,
    pub content_hash: String,
    pub translation: String,
    pub cached: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranslationCacheEntry {
    pub skill_id: String,
    pub relative_path: String,
    pub content_hash: String,
    pub translation: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationCommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub timeout_millis: u64,
    pub prompt_path: PathBuf,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub working_dir: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranslationRunnerError {
    Io(String),
    Timeout {
        timeout_millis: u64,
    },
    Failed {
        status_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    EmptyOutput {
        stdout: String,
        stderr: String,
    },
}

impl Display for TranslationRunnerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslationRunnerError::Io(message) => formatter.write_str(message),
            TranslationRunnerError::Timeout { timeout_millis } => {
                write!(formatter, "Codex 翻译命令超时: {timeout_millis} ms")
            }
            TranslationRunnerError::Failed {
                status_code,
                stderr,
                ..
            } => write!(
                formatter,
                "Codex 翻译命令失败，退出码 {:?}: {}",
                status_code, stderr
            ),
            TranslationRunnerError::EmptyOutput { stderr, .. } => {
                write!(formatter, "Codex 翻译命令没有写入最后输出: {stderr}")
            }
        }
    }
}

#[derive(Debug)]
pub enum TranslationError {
    SkillDoc(SkillDocError),
    CachePath(PathBuf),
    Io(io::Error),
    Json(serde_json::Error),
    AlreadyChinese {
        skill_id: String,
        relative_path: String,
    },
    Command {
        spec: TranslationCommandSpec,
        error: TranslationRunnerError,
    },
}

impl Display for TranslationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslationError::SkillDoc(error) => write!(formatter, "{error}"),
            TranslationError::CachePath(path) => {
                write!(
                    formatter,
                    "翻译缓存路径越过项目 data 目录: {}",
                    path.display()
                )
            }
            TranslationError::Io(error) => write!(formatter, "翻译缓存或临时文件读写失败: {error}"),
            TranslationError::Json(error) => write!(formatter, "翻译缓存 JSON 处理失败: {error}"),
            TranslationError::AlreadyChinese {
                skill_id,
                relative_path,
            } => write!(
                formatter,
                "Skill 文件已经是中文内容，无需翻译: {skill_id} / {relative_path}"
            ),
            TranslationError::Command { spec, error } => write!(
                formatter,
                "外部翻译命令失败: {} {:?}: {error}",
                spec.program, spec.args
            ),
        }
    }
}

impl std::error::Error for TranslationError {}

impl From<SkillDocError> for TranslationError {
    fn from(error: SkillDocError) -> Self {
        Self::SkillDoc(error)
    }
}

impl From<io::Error> for TranslationError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for TranslationError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub trait TranslationRunner {
    fn run(
        &self,
        spec: &TranslationCommandSpec,
        prompt: &str,
    ) -> Result<String, TranslationRunnerError>;
}

pub struct RealCodexRunner;

impl TranslationRunner for RealCodexRunner {
    fn run(
        &self,
        spec: &TranslationCommandSpec,
        prompt: &str,
    ) -> Result<String, TranslationRunnerError> {
        run_codex_exec(spec, prompt)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
struct TranslationCacheFile {
    entries: BTreeMap<String, TranslationCacheEntry>,
}

static TRANSLATION_CACHE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn content_hash(content: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;

    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    format!("{hash:016x}")
}

pub fn cache_key(skill_id: &str, relative_path: &str, content_hash: &str) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        skill_id.len(),
        skill_id,
        relative_path.len(),
        relative_path,
        content_hash
    )
}

pub fn get_translation_state(
    project_dir: impl AsRef<Path>,
    ability: &Ability,
    relative_path: &str,
) -> Result<TranslationState, TranslationError> {
    let project_dir = project_dir.as_ref();
    let file = skilldoc::read_skill_file(ability, relative_path)?;
    let hash = content_hash(&file.content);
    let cached_translation = if file.language == SkillFileLanguage::Chinese {
        None
    } else {
        cached_translation(project_dir, &file, &hash)?
    };

    Ok(TranslationState {
        skill_id: file.skill_id,
        relative_path: file.relative_path,
        content_hash: hash,
        // 中文文件和已有缓存都不需要显示翻译动作；列表扫描也不会触发这里，只有当前打开文件才查缓存。
        show_translate_action: file.language != SkillFileLanguage::Chinese
            && cached_translation.is_none(),
        cached_translation,
    })
}

pub fn translate_skill_file(
    project_dir: impl AsRef<Path>,
    ability: &Ability,
    relative_path: &str,
) -> Result<TranslationResult, TranslationError> {
    translate_skill_file_with_runner(project_dir, ability, relative_path, &RealCodexRunner)
}

pub fn translate_skill_file_with_runner<R: TranslationRunner>(
    project_dir: impl AsRef<Path>,
    ability: &Ability,
    relative_path: &str,
    runner: &R,
) -> Result<TranslationResult, TranslationError> {
    let project_dir = project_dir.as_ref();
    let file = skilldoc::read_skill_file(ability, relative_path)?;
    let hash = content_hash(&file.content);

    if file.language == SkillFileLanguage::Chinese {
        return Err(TranslationError::AlreadyChinese {
            skill_id: file.skill_id,
            relative_path: file.relative_path,
        });
    }

    if let Some(translation) = cached_translation(project_dir, &file, &hash)? {
        return Ok(TranslationResult {
            skill_id: file.skill_id,
            relative_path: file.relative_path,
            content_hash: hash,
            translation,
            cached: true,
        });
    }

    let spec = build_codex_exec_spec(project_dir, &file.skill_id, &file.relative_path, &hash)?;
    let prompt = build_translation_prompt(&file);
    let request_files =
        TempFileCleanup::new(vec![spec.input_path.clone(), spec.prompt_path.clone()]);
    prepare_translation_temp_files(&spec, &file.content, &prompt)?;
    let run_result = runner.run(&spec, &prompt);
    drop(request_files);
    let translation = run_result
        .map_err(|error| TranslationError::Command {
            spec: spec.clone(),
            error,
        })?
        .trim_start_matches('\u{feff}')
        .trim()
        .to_string();

    if translation.is_empty() {
        return Err(TranslationError::Command {
            spec,
            error: TranslationRunnerError::EmptyOutput {
                stdout: String::new(),
                stderr: "Codex 返回了空译文".to_string(),
            },
        });
    }

    save_cached_translation(
        project_dir,
        TranslationCacheEntry {
            skill_id: file.skill_id.clone(),
            relative_path: file.relative_path.clone(),
            content_hash: hash.clone(),
            translation: translation.clone(),
        },
    )?;

    Ok(TranslationResult {
        skill_id: file.skill_id,
        relative_path: file.relative_path,
        content_hash: hash,
        translation,
        cached: false,
    })
}

pub fn build_codex_exec_spec(
    project_dir: impl AsRef<Path>,
    skill_id: &str,
    relative_path: &str,
    content_hash: &str,
) -> Result<TranslationCommandSpec, TranslationError> {
    let project_dir = project_dir.as_ref();
    let tmp_dir = resolve_project_tmp_dir(project_dir)?;
    let base_name = format!(
        "translation-{}-{}-{}-{}-{}",
        std::process::id(),
        nonce(),
        sanitize_file_part(skill_id),
        sanitize_file_part(relative_path),
        content_hash
    );
    let prompt_path = tmp_dir.join(format!("{base_name}.prompt.txt"));
    let input_path = tmp_dir.join(format!("{base_name}.input.txt"));
    let output_path = tmp_dir.join(format!("{base_name}.output.txt"));

    let mut env = BTreeMap::new();
    let tmp_value = tmp_dir.to_string_lossy().into_owned();
    env.insert("TEMP".to_string(), tmp_value.clone());
    env.insert("TMP".to_string(), tmp_value);

    Ok(TranslationCommandSpec {
        program: "codex".to_string(),
        args: vec![
            "exec".to_string(),
            "--ephemeral".to_string(),
            "-C".to_string(),
            project_dir.to_string_lossy().into_owned(),
            "--output-last-message".to_string(),
            output_path.to_string_lossy().into_owned(),
            "-".to_string(),
        ],
        env,
        timeout_millis: CODEX_TRANSLATION_TIMEOUT_MILLIS,
        prompt_path,
        input_path,
        output_path,
        working_dir: project_dir.to_path_buf(),
    })
}

fn cached_translation(
    project_dir: &Path,
    file: &SkillFileContent,
    hash: &str,
) -> Result<Option<String>, TranslationError> {
    let _guard = translation_cache_lock()
        .lock()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "翻译缓存锁已损坏"))?;
    let cache = load_cache(project_dir)?;
    let key = cache_key(&file.skill_id, &file.relative_path, hash);

    Ok(cache
        .entries
        .get(&key)
        .map(|entry| entry.translation.clone()))
}

fn save_cached_translation(
    project_dir: &Path,
    entry: TranslationCacheEntry,
) -> Result<(), TranslationError> {
    let _guard = translation_cache_lock()
        .lock()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "翻译缓存锁已损坏"))?;
    let mut cache = load_cache(project_dir)?;
    let key = cache_key(&entry.skill_id, &entry.relative_path, &entry.content_hash);
    cache.entries.insert(key, entry);
    save_cache(project_dir, &cache)
}

fn translation_cache_lock() -> &'static Mutex<()> {
    TRANSLATION_CACHE_LOCK.get_or_init(|| Mutex::new(()))
}

fn prepare_translation_temp_files(
    spec: &TranslationCommandSpec,
    input: &str,
    prompt: &str,
) -> Result<(), TranslationError> {
    // 外部命令边界：当前文件原文、提示词和 Codex 输出都放在项目 .tmp，便于限制落盘范围，也避免系统 TEMP 混入 C 盘缓存。
    let result = (|| {
        fs::write(&spec.input_path, input)?;
        fs::write(&spec.prompt_path, prompt)?;
        Ok::<(), io::Error>(())
    })();

    if let Err(error) = result {
        let _ = fs::remove_file(&spec.input_path);
        let _ = fs::remove_file(&spec.prompt_path);
        return Err(TranslationError::Io(error));
    }

    Ok(())
}

fn load_cache(project_dir: &Path) -> Result<TranslationCacheFile, TranslationError> {
    let path = translation_cache_path(project_dir)?;
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(TranslationCacheFile::default());
        }
        Err(error) => return Err(TranslationError::Io(error)),
    };

    Ok(serde_json::from_str(&content)?)
}

fn save_cache(project_dir: &Path, cache: &TranslationCacheFile) -> Result<(), TranslationError> {
    let path = translation_cache_path(project_dir)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(cache)?;
    let tmp_dir = resolve_project_tmp_dir(project_dir)?;
    let tmp_path = tmp_dir.join(format!(
        ".translation-cache-{}-{}.json",
        std::process::id(),
        nonce()
    ));

    let write_result = (|| {
        let mut file = File::create(&tmp_path)?;
        // 缓存失效依赖 key 中的 contentHash；写入半截 JSON 会让后续误判为无缓存，所以先写项目 .tmp 再替换目标。
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        Ok::<(), io::Error>(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(TranslationError::Io(error));
    }

    replace_file_atomically(&tmp_path, &path).map_err(TranslationError::Io)
}

fn translation_cache_path(project_dir: &Path) -> Result<PathBuf, TranslationError> {
    util::resolve_project_data_path(project_dir, Path::new(TRANSLATION_CACHE_FILE)).map_err(
        |error| match error {
            DataPathError::Outside(path) => TranslationError::CachePath(path),
            DataPathError::Io(error) => TranslationError::Io(error),
        },
    )
}

fn resolve_project_tmp_dir(project_dir: &Path) -> Result<PathBuf, TranslationError> {
    let project_root = project_dir.canonicalize()?;
    let tmp_dir = project_dir.join(".tmp");
    fs::create_dir_all(&tmp_dir)?;
    let canonical_tmp = tmp_dir.canonicalize()?;

    // 临时文件边界：外部命令输出、日志和缓存临时文件都只能落在当前项目 .tmp，不能复用系统 TEMP。
    if !canonical_tmp.starts_with(&project_root) {
        return Err(TranslationError::CachePath(canonical_tmp));
    }

    Ok(tmp_dir)
}

fn build_translation_prompt(file: &SkillFileContent) -> String {
    format!(
        "\
你是 Codex Atlas 的 Skill 文档翻译器。
只翻译当前打开的单个文件，不要遍历目录，不要批量翻译任何其他 Skill，不要修改文件。
请把下面内容翻译成自然、准确、面向中国用户的中文。保留 Markdown 结构、代码块、命令、API 名称和配置键；只输出译文，不要解释。

Skill ID: {}
Relative path: {}

<content>
{}
</content>
",
        file.skill_id, file.relative_path, file.content
    )
}

fn run_codex_exec(
    spec: &TranslationCommandSpec,
    prompt: &str,
) -> Result<String, TranslationRunnerError> {
    let stdout_path = sibling_command_path(&spec.output_path, "stdout.log");
    let stderr_path = sibling_command_path(&spec.output_path, "stderr.log");
    let _cleanup = TempFileCleanup::new(vec![
        spec.output_path.clone(),
        stdout_path.clone(),
        stderr_path.clone(),
    ]);
    let stdout_file = File::create(&stdout_path).map_err(io_error)?;
    let stderr_file = File::create(&stderr_path).map_err(io_error)?;

    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    for (key, value) in &spec.env {
        command.env(key, value);
    }

    // 真实翻译命令固定使用 `codex exec --ephemeral`，并通过 stdin 传提示词，避免长 Skill 内容进入命令行参数。
    let mut child = command.spawn().map_err(io_error)?;
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(error) = stdin.write_all(prompt.as_bytes()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io_error(error));
        }
    }

    let started_at = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(io_error)? {
            break status;
        }
        if started_at.elapsed() >= Duration::from_millis(spec.timeout_millis) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(TranslationRunnerError::Timeout {
                timeout_millis: spec.timeout_millis,
            });
        }
        thread::sleep(Duration::from_millis(WAIT_POLL_MILLIS));
    };

    let stdout = read_limited_text(&stdout_path);
    let stderr = read_limited_text(&stderr_path);

    if !status.success() {
        return Err(TranslationRunnerError::Failed {
            status_code: status.code(),
            stdout,
            stderr,
        });
    }

    let translation = fs::read_to_string(&spec.output_path).map_err(io_error)?;
    if translation.trim().is_empty() {
        return Err(TranslationRunnerError::EmptyOutput { stdout, stderr });
    }

    Ok(translation)
}

fn read_limited_text(path: &Path) -> String {
    fs::read_to_string(path)
        .map(|content| truncate_text(&content))
        .unwrap_or_default()
}

fn truncate_text(content: &str) -> String {
    if content.len() <= COMMAND_TEXT_LIMIT {
        return content.to_string();
    }

    let mut output = String::new();
    for character in content.chars() {
        if output.len() + character.len_utf8() > COMMAND_TEXT_LIMIT {
            break;
        }
        output.push(character);
    }
    output.push_str("\n...[已截断]");
    output
}

fn sibling_command_path(path: &Path, suffix: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "translation".into());
    path.with_file_name(format!("{file_name}.{suffix}"))
}

fn sanitize_file_part(value: &str) -> String {
    let mut output = String::new();
    let mut last_dash = false;

    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            output.push(character);
            last_dash = false;
        } else if !last_dash {
            output.push('-');
            last_dash = true;
        }
    }

    let trimmed = output.trim_matches('-');
    if trimmed.is_empty() {
        "skill".to_string()
    } else {
        trimmed.to_string()
    }
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn io_error(error: io::Error) -> TranslationRunnerError {
    TranslationRunnerError::Io(error.to_string())
}

fn replace_file_atomically(temp_path: &Path, target_path: &Path) -> io::Result<()> {
    if let Ok(metadata) = fs::metadata(target_path) {
        if !metadata.is_file() {
            let _ = fs::remove_file(temp_path);
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("翻译缓存目标不是文件: {}", target_path.display()),
            ));
        }
    }

    replace_file_atomically_inner(temp_path, target_path).map_err(|error| {
        let _ = fs::remove_file(temp_path);
        error
    })
}

#[cfg(windows)]
fn replace_file_atomically_inner(temp_path: &Path, target_path: &Path) -> io::Result<()> {
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let existing = path_to_wide(temp_path);
    let target = path_to_wide(target_path);

    // Windows 的 std::fs::rename 不能覆盖已有文件；用系统 replace 语义避免先删除缓存目标造成缺失窗口。
    let replaced = unsafe {
        MoveFileExW(
            existing.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if replaced == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file_atomically_inner(temp_path: &Path, target_path: &Path) -> io::Result<()> {
    fs::rename(temp_path, target_path)
}

#[cfg(windows)]
fn path_to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

struct TempFileCleanup {
    paths: Vec<PathBuf>,
}

impl TempFileCleanup {
    fn new(paths: Vec<PathBuf>) -> Self {
        Self { paths }
    }
}

impl Drop for TempFileCleanup {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = fs::remove_file(path);
        }
    }
}
