use std::path::PathBuf;

pub struct ScannerFixture {
    root: PathBuf,
}

impl ScannerFixture {
    pub fn new(name: &str) -> Self {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri should live inside project root")
            .to_path_buf();
        let root = project_root
            .join(".tmp")
            .join("test-fixtures")
            .join(format!("scanner-{name}-{}", std::process::id()));

        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scanner fixture root");

        Self { root }
    }

    pub fn path(&self, relative: impl AsRef<std::path::Path>) -> PathBuf {
        self.root.join(relative)
    }

    pub fn write_text(&self, relative: impl AsRef<std::path::Path>, contents: &str) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture parent directory");
        }
        std::fs::write(path, contents).expect("write fixture text file");
    }

    pub fn write_bytes(&self, relative: impl AsRef<std::path::Path>, contents: &[u8]) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture parent directory");
        }
        std::fs::write(path, contents).expect("write fixture byte file");
    }
}

impl Drop for ScannerFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
