use std::{
    io,
    path::{Component, Path, PathBuf},
};

#[derive(Debug)]
pub enum DataPathError {
    Outside(PathBuf),
    Io(io::Error),
}

impl From<io::Error> for DataPathError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn resolve_project_data_path(
    project_dir: &Path,
    relative_path: &Path,
) -> Result<PathBuf, DataPathError> {
    let project_root = project_dir.canonicalize()?;
    let data_dir = project_root.join("data");

    if relative_path.is_absolute() {
        return Err(DataPathError::Outside(relative_path.to_path_buf()));
    }

    let mut normalized = PathBuf::new();

    for component in relative_path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(DataPathError::Outside(relative_path.to_path_buf()));
            }
        }
    }

    let resolved = data_dir.join(normalized);
    let canonical_data_dir = if data_dir.exists() {
        let canonical = data_dir.canonicalize()?;

        if !canonical.starts_with(&project_root) {
            return Err(DataPathError::Outside(canonical));
        }

        Some(canonical)
    } else {
        None
    };

    // 项目 data 目录边界：已存在的 data、目标文件或父目录都要用真实路径校验，避免符号链接或目录联接指向项目外。
    if resolved.exists() {
        let canonical_target = resolved.canonicalize()?;
        ensure_inside_data_dir(
            &canonical_target,
            &project_root,
            canonical_data_dir.as_ref(),
        )?;
        return Ok(resolved);
    }

    let nearest_existing = nearest_existing_ancestor(resolved.parent(), &project_root);
    let canonical_parent = nearest_existing.canonicalize()?;
    ensure_inside_data_dir(
        &canonical_parent,
        &project_root,
        canonical_data_dir.as_ref(),
    )?;

    Ok(resolved)
}

fn nearest_existing_ancestor(start: Option<&Path>, fallback: &Path) -> PathBuf {
    let mut current = start.map(Path::to_path_buf);

    while let Some(path) = current {
        if path.exists() {
            return path;
        }

        current = path.parent().map(Path::to_path_buf);
    }

    fallback.to_path_buf()
}

fn ensure_inside_data_dir(
    canonical_path: &Path,
    project_root: &Path,
    canonical_data_dir: Option<&PathBuf>,
) -> Result<(), DataPathError> {
    match canonical_data_dir {
        Some(data_dir) if canonical_path.starts_with(data_dir) => Ok(()),
        Some(_) => Err(DataPathError::Outside(canonical_path.to_path_buf())),
        None if canonical_path.starts_with(project_root) => Ok(()),
        None => Err(DataPathError::Outside(canonical_path.to_path_buf())),
    }
}
