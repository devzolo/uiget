use regex::Regex;
use serde::Deserialize;
use std::{env, fs, path::{Path, PathBuf}, time::SystemTime, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    YarnClassic, // yarn 1.x
    YarnBerry,   // yarn 2+
    Pnpm,
    Bun,
    Unknown,
}

#[derive(Debug, Clone)]
pub enum DetectionSource {
    PackageJsonField,     // package.json "packageManager"
    Lockfile(PathBuf),    // yarn.lock, pnpm-lock.yaml, etc.
    YarnArtifacts(PathBuf), // .pnp.cjs, .yarnrc.yml com yarnPath/nodeLinker
    PnpmArtifacts(PathBuf), // pnpm-workspace.yaml
    UserAgent(String),    // npm_config_user_agent
    Heuristic,            // fallback
}

#[derive(Debug, Clone)]
pub struct Detection {
    pub manager: PackageManager,
    pub version_hint: Option<String>,
    pub source: DetectionSource,
    pub project_root: PathBuf,
}

#[derive(Debug)]
pub enum DetectError {
    NoProject(String),
    Io(std::io::Error),
    BadJson(String, String),
}

impl fmt::Display for DetectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DetectError::NoProject(path) => {
                write!(f, "nenhum projeto Node encontrado (package.json) a partir de {}", path)
            }
            DetectError::Io(err) => write!(f, "erro de IO: {}", err),
            DetectError::BadJson(path, msg) => write!(f, "json inválido em {}: {}", path, msg),
        }
    }
}

impl std::error::Error for DetectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DetectError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for DetectError {
    fn from(err: std::io::Error) -> Self {
        DetectError::Io(err)
    }
}

#[derive(Deserialize)]
struct PackageJson {
    #[serde(default)]
    packageManager: Option<String>,
}

pub fn detect_package_manager(start_dir: impl AsRef<Path>) -> Result<Detection, DetectError> {
    let start = start_dir.as_ref().canonicalize()?;
    let project_root = find_project_root(&start)
        .ok_or_else(|| DetectError::NoProject(start.display().to_string()))?;

    // 0) user agent (se existir) – útil quando a CLI é invocada via npm/yarn/pnpm/bun
    if let Some(ua) = env::var("npm_config_user_agent").ok() {
        if let Some((pm, ver)) = parse_user_agent(&ua) {
            return Ok(Detection {
                manager: pm,
                version_hint: ver,
                source: DetectionSource::UserAgent(ua),
                project_root,
            });
        }
    }

    // 1) package.json → "packageManager"
    if let Ok((pm, ver)) = read_package_manager_field(&project_root) {
        return Ok(Detection {
            manager: pm,
            version_hint: ver,
            source: DetectionSource::PackageJsonField,
            project_root,
        });
    }

    // 2) artefatos específicos (yarn berry, pnpm)
    if let Some(path) = find_yarn_artifacts(&project_root) {
        return Ok(Detection {
            manager: PackageManager::YarnBerry,
            version_hint: None,
            source: DetectionSource::YarnArtifacts(path),
            project_root,
        });
    }
    if let Some(path) = find_pnpm_artifacts(&project_root) {
        return Ok(Detection {
            manager: PackageManager::Pnpm,
            version_hint: None,
            source: DetectionSource::PnpmArtifacts(path),
            project_root,
        });
    }

    // 3) lockfiles (com desempate por mtime)
    if let Some(det) = pick_by_lockfiles(&project_root)? {
        return Ok(det);
    }

    // 4) fallback explícito
    Ok(Detection {
        manager: PackageManager::Npm,
        version_hint: None,
        source: DetectionSource::Heuristic,
        project_root,
    })
}

fn find_project_root(from: &Path) -> Option<PathBuf> {
    let mut cur = Some(from.to_path_buf());
    while let Some(dir) = cur {
        if dir.join("package.json").exists() {
            return Some(dir);
        }
        cur = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

fn read_package_manager_field(root: &Path) -> Result<(PackageManager, Option<String>), DetectError> {
    let pj_path = root.join("package.json");
    let data = fs::read_to_string(&pj_path)?;
    let pj: PackageJson = serde_json::from_str(&data)
        .map_err(|e| DetectError::BadJson(pj_path.display().to_string(), e.to_string()))?;

    if let Some(pm_str) = pj.packageManager {
        // formato: "<name>@<version>", ex: "pnpm@8.15.4", "yarn@3.5.1", "npm@9.9.0", "bun@1.1.8"
        let re = Regex::new(r"^(?P<name>[a-zA-Z]+)@(?P<ver>[\w\.\-]+)$").unwrap();
        if let Some(caps) = re.captures(&pm_str) {
            let name = &caps["name"].to_lowercase();
            let ver = caps["ver"].to_string();
            let pm = match name.as_str() {
                "pnpm" => PackageManager::Pnpm,
                "npm" => PackageManager::Npm,
                "bun" => PackageManager::Bun,
                "yarn" => {
                    // yarn 1.x = classic, 2+ = berry
                    if is_semver_gte(&ver, 2, 0, 0) { PackageManager::YarnBerry } else { PackageManager::YarnClassic }
                }
                _ => PackageManager::Unknown,
            };
            return Ok((pm, Some(ver)));
        }
    }
    Err(DetectError::BadJson(
        pj_path.display().to_string(),
        "campo packageManager ausente ou fora do formato <name>@<version>".into(),
    ))
}

fn find_yarn_artifacts(root: &Path) -> Option<PathBuf> {
    // Yarn Berry geralmente tem .yarn/ e/ou .pnp.cjs/.pnp.data.json
    let candidates = [
        root.join(".pnp.cjs"),
        root.join(".pnp.loader.mjs"),
        root.join(".pnp.data.json"),
        root.join(".yarnrc.yml"),
        root.join(".yarn"),
    ];
    for p in candidates {
        if p.exists() {
            // heurística extra: se existir .yarnrc.yml com "nodeLinker: pnp" ou "yarnPath"
            if p.file_name()?.to_string_lossy() == ".yarnrc.yml" {
                if let Ok(s) = fs::read_to_string(&p) {
                    if s.contains("nodeLinker: pnp") || s.contains("yarnPath:") {
                        return Some(p);
                    }
                }
                // mesmo sem essas chaves, manter como pista
                return Some(p);
            }
            return Some(p);
        }
    }
    None
}

fn find_pnpm_artifacts(root: &Path) -> Option<PathBuf> {
    let p = root.join("pnpm-workspace.yaml");
    if p.exists() { Some(p) } else { None }
}

fn pick_by_lockfiles(root: &Path) -> Result<Option<Detection>, std::io::Error> {
    let mut candidates: Vec<(PackageManager, PathBuf, SystemTime)> = Vec::new();

    let map = [
        (PackageManager::YarnClassic, root.join("yarn.lock")),
        (PackageManager::Pnpm, root.join("pnpm-lock.yaml")),
        (PackageManager::Npm, root.join("package-lock.json")),
        (PackageManager::Bun, root.join("bun.lockb")),
    ];

    for (pm, path) in map {
        if path.exists() {
            let meta = fs::metadata(&path)?;
            let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            candidates.push((pm, path, mtime));
        }
    }

    if candidates.is_empty() {
        return Ok(None);
    }

    // desempate: lockfile mais recente
    candidates.sort_by_key(|(_, _, m)| *m);
    let (pm, path, _) = candidates.last().unwrap().clone();

    Ok(Some(Detection {
        manager: pm,
        version_hint: None,
        source: DetectionSource::Lockfile(path),
        project_root: root.to_path_buf(),
    }))
}

/// npm_config_user_agent exemplos:
/// "pnpm/8.15.3 npm/? node/v20.14.0 darwin arm64"
/// "yarn/1.22.19 npm/? node/v18.16.0 win32 x64"
/// "npm/9.6.7 node/v18.16.0 linux x64"
/// "bun/1.1.8 darwin x64"
fn parse_user_agent(ua: &str) -> Option<(PackageManager, Option<String>)> {
    let parts: Vec<&str> = ua.split_whitespace().collect();
    if parts.is_empty() { return None; }
    let first = parts[0]; // "<name>/<version>" ou algo similar

    // Check if first part contains '/' and has the right format
    if !first.contains('/') { return None; }

    let mut it = first.split('/');
    let name = it.next()?.to_lowercase();
    let ver = it.next().map(|s| s.to_string());

    // If no version part, it's invalid format
    if ver.is_none() { return None; }

    let pm = match name.as_str() {
        "pnpm" => PackageManager::Pnpm,
        "yarn" => {
            // não temos a major aqui; se quiser diferenciar 1.x de 2+ via UA,
            // parse ver e decide:
            if let Some(v) = &ver {
                if is_semver_gte(v, 2, 0, 0) { PackageManager::YarnBerry } else { PackageManager::YarnClassic }
            } else {
                PackageManager::YarnClassic
            }
        }
        "npm" => PackageManager::Npm,
        "bun" => PackageManager::Bun,
        _ => return None, // Invalid/unknown package manager
    };
    Some((pm, ver))
}

fn is_semver_gte(ver: &str, maj: u64, min: u64, pat: u64) -> bool {
    // parse parcial: "3.6.1", "3.6", "3"
    let mut nums = ver.split('.').map(|s| s.parse::<u64>().unwrap_or(0));
    let vmaj = nums.next().unwrap_or(0);
    let vmin = nums.next().unwrap_or(0);
    let vpat = nums.next().unwrap_or(0);
    (vmaj, vmin, vpat) >= (maj, min, pat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_package_manager_install_commands() {
        assert_eq!(PackageManager::Npm.install_command(), vec!["npm", "install"]);
        assert_eq!(PackageManager::YarnClassic.install_command(), vec!["yarn", "add"]);
        assert_eq!(PackageManager::YarnBerry.install_command(), vec!["yarn", "add"]);
        assert_eq!(PackageManager::Pnpm.install_command(), vec!["pnpm", "add"]);
        assert_eq!(PackageManager::Bun.install_command(), vec!["bun", "add"]);
        assert_eq!(PackageManager::Unknown.install_command(), vec!["npm", "install"]);
    }

    #[test]
    fn test_package_manager_install_dev_commands() {
        assert_eq!(PackageManager::Npm.install_dev_command(), vec!["npm", "install", "--save-dev"]);
        assert_eq!(PackageManager::YarnClassic.install_dev_command(), vec!["yarn", "add", "--dev"]);
        assert_eq!(PackageManager::YarnBerry.install_dev_command(), vec!["yarn", "add", "--dev"]);
        assert_eq!(PackageManager::Pnpm.install_dev_command(), vec!["pnpm", "add", "--save-dev"]);
        assert_eq!(PackageManager::Bun.install_dev_command(), vec!["bun", "add", "--dev"]);
        assert_eq!(PackageManager::Unknown.install_dev_command(), vec!["npm", "install", "--save-dev"]);
    }

    #[test]
    fn test_package_manager_names() {
        assert_eq!(PackageManager::Npm.name(), "npm");
        assert_eq!(PackageManager::YarnClassic.name(), "yarn (classic)");
        assert_eq!(PackageManager::YarnBerry.name(), "yarn (berry)");
        assert_eq!(PackageManager::Pnpm.name(), "pnpm");
        assert_eq!(PackageManager::Bun.name(), "bun");
        assert_eq!(PackageManager::Unknown.name(), "unknown");
    }

    #[test]
    fn test_parse_user_agent() {
        // Test npm user agent
        let ua = "npm/9.6.7 node/v18.16.0 linux x64";
        let (pm, ver) = parse_user_agent(ua).unwrap();
        assert_eq!(pm, PackageManager::Npm);
        assert_eq!(ver, Some("9.6.7".to_string()));

        // Test yarn classic user agent
        let ua = "yarn/1.22.19 npm/? node/v18.16.0 win32 x64";
        let (pm, ver) = parse_user_agent(ua).unwrap();
        assert_eq!(pm, PackageManager::YarnClassic);
        assert_eq!(ver, Some("1.22.19".to_string()));

        // Test yarn berry user agent
        let ua = "yarn/3.5.1 npm/? node/v18.16.0 win32 x64";
        let (pm, ver) = parse_user_agent(ua).unwrap();
        assert_eq!(pm, PackageManager::YarnBerry);
        assert_eq!(ver, Some("3.5.1".to_string()));

        // Test pnpm user agent
        let ua = "pnpm/8.15.3 npm/? node/v20.14.0 darwin arm64";
        let (pm, ver) = parse_user_agent(ua).unwrap();
        assert_eq!(pm, PackageManager::Pnpm);
        assert_eq!(ver, Some("8.15.3".to_string()));

        // Test bun user agent
        let ua = "bun/1.1.8 darwin x64";
        let (pm, ver) = parse_user_agent(ua).unwrap();
        assert_eq!(pm, PackageManager::Bun);
        assert_eq!(ver, Some("1.1.8".to_string()));

        // Test invalid user agent
        assert!(parse_user_agent("").is_none());
        assert!(parse_user_agent("invalid").is_none());
    }

    #[test]
    fn test_is_semver_gte() {
        assert!(is_semver_gte("3.6.1", 3, 6, 0));
        assert!(is_semver_gte("3.6.0", 3, 6, 0));
        assert!(is_semver_gte("3.7.0", 3, 6, 0));
        assert!(is_semver_gte("4.0.0", 3, 6, 0));

        assert!(!is_semver_gte("3.5.9", 3, 6, 0));
        assert!(!is_semver_gte("2.9.9", 3, 6, 0));
        assert!(!is_semver_gte("3.6.0", 3, 6, 1));
    }

    #[test]
    fn test_find_project_root() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("my-project");
        fs::create_dir(&project_dir).unwrap();

        // No package.json yet
        assert!(find_project_root(&project_dir).is_none());

        // Create package.json
        fs::write(project_dir.join("package.json"), r#"{"name": "test"}"#).unwrap();
        assert_eq!(find_project_root(&project_dir), Some(project_dir.clone()));

        // Test from subdirectory
        let sub_dir = project_dir.join("src");
        fs::create_dir(&sub_dir).unwrap();
        assert_eq!(find_project_root(&sub_dir), Some(project_dir));
    }

    #[test]
    fn test_detect_error_display() {
        let err = DetectError::NoProject("/path/to/project".to_string());
        assert!(err.to_string().contains("nenhum projeto Node"));

        let err = DetectError::BadJson("file.json".to_string(), "invalid json".to_string());
        assert!(err.to_string().contains("json inválido"));
    }
}

impl PackageManager {
    /// Retorna o comando para instalar dependências normais
    pub fn install_command(&self) -> Vec<String> {
        match self {
            PackageManager::Npm => vec!["npm".to_string(), "install".to_string()],
            PackageManager::YarnClassic => vec!["yarn".to_string(), "add".to_string()],
            PackageManager::YarnBerry => vec!["yarn".to_string(), "add".to_string()],
            PackageManager::Pnpm => vec!["pnpm".to_string(), "add".to_string()],
            PackageManager::Bun => vec!["bun".to_string(), "add".to_string()],
            PackageManager::Unknown => vec!["npm".to_string(), "install".to_string()],
        }
    }

    /// Retorna o comando para instalar dev dependencies
    pub fn install_dev_command(&self) -> Vec<String> {
        match self {
            PackageManager::Npm => vec!["npm".to_string(), "install".to_string(), "--save-dev".to_string()],
            PackageManager::YarnClassic => vec!["yarn".to_string(), "add".to_string(), "--dev".to_string()],
            PackageManager::YarnBerry => vec!["yarn".to_string(), "add".to_string(), "--dev".to_string()],
            PackageManager::Pnpm => vec!["pnpm".to_string(), "add".to_string(), "--save-dev".to_string()],
            PackageManager::Bun => vec!["bun".to_string(), "add".to_string(), "--dev".to_string()],
            PackageManager::Unknown => vec!["npm".to_string(), "install".to_string(), "--save-dev".to_string()],
        }
    }

    /// Retorna o nome do package manager para exibição
    pub fn name(&self) -> &'static str {
        match self {
            PackageManager::Npm => "npm",
            PackageManager::YarnClassic => "yarn (classic)",
            PackageManager::YarnBerry => "yarn (berry)",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Bun => "bun",
            PackageManager::Unknown => "unknown",
        }
    }
}

impl Detection {
    /// Retorna informações sobre a detecção para logging
    pub fn info(&self) -> String {
        let source_desc = match &self.source {
            DetectionSource::PackageJsonField => "package.json field".to_string(),
            DetectionSource::Lockfile(path) => format!("lockfile: {}", path.display()),
            DetectionSource::YarnArtifacts(path) => format!("yarn artifacts: {}", path.display()),
            DetectionSource::PnpmArtifacts(path) => format!("pnpm artifacts: {}", path.display()),
            DetectionSource::UserAgent(ua) => format!("user agent: {}", ua),
            DetectionSource::Heuristic => "heuristic".to_string(),
        };

        format!(
            "Detected {} via {} at {}",
            self.manager.name(),
            source_desc,
            self.project_root.display()
        )
    }
}
