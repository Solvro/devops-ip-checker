use std::{
    env::{self},
    fs::File,
    io::{Read, Write},
    path::Path,
};

use chrono::Local;
use cloneable_errors::{ErrorContext, ResContext, bail};

struct GitMetadata {
    r#ref: Option<String>,
    hash: String,
}

fn main() -> Result<(), ErrorContext> {
    let metadata_file =
        Path::new(&env::var("OUT_DIR").context("OUT_DIR not set")?).join("metadata.rs");
    let mut metadata_file = File::create(&metadata_file)
        .with_context(|| format!("Failed to create file {}", metadata_file.display()))?;

    let git_meta = get_git_metadata()
        .map_err(|e| {
            eprintln!("Failed to extract current git commit hash from build context:\n{e:?}");
        })
        .ok();

    writeln!(
        &mut metadata_file,
        "/// Hash of the git commit this server was built from\npub const GIT_HASH: Option<&str> = {:?};",
        git_meta.as_ref().map(|m| &m.hash),
    ).context("Failed to write GIT_HASH constant to metadata file")?;
    writeln!(
        &mut metadata_file,
        "/// Git ref this server was built from\npub const GIT_REF: Option<&str> = {:?};",
        git_meta.as_ref().and_then(|m| m.r#ref.as_ref()),
    )
    .context("Failed to write GIT_REF constant to metadata file")?;
    writeln!(
        &mut metadata_file,
        "/// Timestamp in RFC2822 when this server was built\npub const BUILD_TIME: &str = {:?};",
        get_build_timestamp(),
    )
    .context("Failed to write BUILD_TIME constant to metadata file")?;

    Ok(())
}

fn read_file<P: AsRef<Path>>(path: P) -> Result<String, ErrorContext> {
    let path: &Path = path.as_ref();
    let mut file =
        File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut buf = String::with_capacity(64);
    file.read_to_string(&mut buf)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    Ok(buf)
}

fn get_git_metadata() -> Result<GitMetadata, ErrorContext> {
    get_direct_git_metadata().or_else(|e| get_git_meta_from_env().ok_or(e))
}

fn get_git_meta_from_env() -> Option<GitMetadata> {
    let mut commit = env::var("SOURCE_COMMIT").ok()?;
    commit.make_ascii_lowercase();

    if !is_valid_git_hash(&commit) {
        return None;
    }

    Some(GitMetadata {
        r#ref: env::var("COOLIFY_BRANCH").ok(),
        hash: commit,
    })
}

fn get_direct_git_metadata() -> Result<GitMetadata, ErrorContext> {
    let git_dir = Path::new(&env::var_os("GIT_DIR").unwrap_or_else(|| ".git".into()))
        .canonicalize()
        .context("Failed to canonicalize the path to the git dir")?;
    let mut head_file = read_file(git_dir.join("HEAD"))?;

    if let Some(r#ref) = head_file.trim().strip_prefix("ref: ") {
        let ref_path = git_dir
            .join(r#ref)
            .canonicalize()
            .context("Failed to canonicalize the path to the git ref file pointed by HEAD")?;

        if !ref_path.starts_with(git_dir) {
            bail!(".git/HEAD contained an invalid ref");
        }

        let mut ref_file = read_file(&ref_path)?;
        ref_file.make_ascii_lowercase();
        let ref_file = ref_file.trim();

        if !is_valid_git_hash(ref_file) {
            bail!(
                "{} (pointed to by .git/HEAD) did not contain a git commit hash",
                ref_path.display()
            );
        }

        Ok(GitMetadata {
            r#ref: Some(r#ref.to_string()),
            hash: ref_file.to_string(),
        })
    } else {
        head_file.make_ascii_lowercase();
        let head_file = head_file.trim();

        if !is_valid_git_hash(head_file) {
            bail!(".git/HEAD did not contain a ref or git commit hash");
        }

        Ok(GitMetadata {
            r#ref: None,
            hash: head_file.to_string(),
        })
    }
}

fn is_valid_git_hash(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn get_build_timestamp() -> String {
    Local::now().to_rfc2822()
}
