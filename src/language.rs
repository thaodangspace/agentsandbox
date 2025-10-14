use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

use crate::startup_log;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ProjectLanguage {
    Rust,
    NodeJs,
    Python,
    Go,
    Php,
    Ruby,
}

impl ProjectLanguage {
    pub fn name(&self) -> &'static str {
        match self {
            ProjectLanguage::Rust => "Rust",
            ProjectLanguage::NodeJs => "Node.js",
            ProjectLanguage::Python => "Python",
            ProjectLanguage::Go => "Go",
            ProjectLanguage::Php => "PHP",
            ProjectLanguage::Ruby => "Ruby",
        }
    }

    pub fn global_config_paths(&self) -> Vec<&'static str> {
        match self {
            ProjectLanguage::Rust => vec![".cargo"],
            ProjectLanguage::NodeJs => vec![".npm", ".npmrc", ".yarn"],
            ProjectLanguage::Python => vec![".pip", ".cache/pip", ".pypirc"],
            ProjectLanguage::Go => vec!["go", ".config/go"],
            ProjectLanguage::Php => vec![".composer"],
            ProjectLanguage::Ruby => vec![".gem", ".bundle"],
        }
    }

    pub fn tool(&self) -> &'static str {
        match self {
            ProjectLanguage::Rust => "cargo",
            ProjectLanguage::NodeJs => "npm",
            ProjectLanguage::Python => "pip",
            ProjectLanguage::Go => "go",
            ProjectLanguage::Php => "composer",
            ProjectLanguage::Ruby => "bundle",
        }
    }

    pub fn install_cmd(&self) -> &'static str {
        match self {
            ProjectLanguage::Rust => "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && ~/.cargo/bin/rustup component add rustfmt clippy",
            ProjectLanguage::NodeJs => "curl -fsSL https://deb.nodesource.com/setup_22.x | sudo bash - && sudo apt-get install -y nodejs",
            ProjectLanguage::Python => "sudo apt-get update && sudo apt-get install -y python3 python3-pip",
            ProjectLanguage::Go => "wget https://go.dev/dl/go1.24.5.linux-amd64.tar.gz && sudo tar -C /usr/local -xzf go1.24.5.linux-amd64.tar.gz && rm go1.24.5.linux-amd64.tar.gz",
            ProjectLanguage::Php => "sudo apt-get update && sudo apt-get install -y php-cli unzip && curl -sS https://getcomposer.org/installer | php -- --install-dir=/usr/local/bin --filename=composer",
            ProjectLanguage::Ruby => "sudo apt-get update && sudo apt-get install -y ruby-full && sudo gem install bundler",
        }
    }
}

pub fn detect_project_languages(dir: &Path) -> Vec<ProjectLanguage> {
    let mut langs = Vec::new();
    if dir.join("Cargo.toml").exists() {
        langs.push(ProjectLanguage::Rust);
    }
    if dir.join("package.json").exists() {
        langs.push(ProjectLanguage::NodeJs);
    }
    if dir.join("requirements.txt").exists() || dir.join("pyproject.toml").exists() {
        langs.push(ProjectLanguage::Python);
    }
    if dir.join("go.mod").exists() {
        langs.push(ProjectLanguage::Go);
    }
    if dir.join("composer.json").exists() {
        langs.push(ProjectLanguage::Php);
    }
    if dir.join("Gemfile").exists() {
        langs.push(ProjectLanguage::Ruby);
    }
    langs
}

pub fn ensure_language_tools(container_name: &str, languages: &[ProjectLanguage]) -> Result<()> {
    let mut installed_languages = Vec::new();
    for lang in languages {
        let tool = lang.tool();
        let check_status = Command::new("docker")
            .args([
                "exec",
                container_name,
                "bash",
                "-lc",
                &format!("command -v {tool}"),
            ])
            .status()
            .with_context(|| format!("Failed to check for {}", tool))?;
        if check_status.success() {
            continue;
        }
        println!("Installing toolchain for {}...", lang.name());
        let install_status = Command::new("docker")
            .args(["exec", container_name, "bash", "-lc", lang.install_cmd()])
            .status()
            .with_context(|| format!("Failed to install {}", tool))?;
        if !install_status.success() {
            anyhow::bail!("Installation for {} failed", tool);
        }
        installed_languages.push(lang.name());
    }
    if !installed_languages.is_empty() {
        startup_log::event(format!(
            "🛠️  Installing developer tooling for {}",
            installed_languages.join(", ")
        ));
    }
    Ok(())
}

// For languages that require per-project dependencies, prepare them inside the container.
// Currently supports Node.js: runs `npm ci` if lockfile exists, else `npm install`.
// For Node.js projects, copy host node_modules into the container's isolated
// volume path to keep installs off the host while avoiding a full reinstall.
pub fn sync_node_modules_from_host(
    container_name: &str,
    project_dir: &Path,
    languages: &[ProjectLanguage],
) -> Result<()> {
    if !languages.iter().any(|l| *l == ProjectLanguage::NodeJs) {
        return Ok(());
    }

    let host_nm = project_dir.join("node_modules");
    if !host_nm.is_dir() {
        // Nothing to copy; host has no node_modules
        return Ok(());
    }

    // Ensure target path exists in container and is owned by user, then copy
    let container_nm = host_nm.display().to_string();
    let mkdir_cmd = format!(
        "sudo mkdir -p '{}' && sudo chown -R $(id -u):$(id -g) '{}'",
        container_nm, container_nm
    );
    let mkdir_status = Command::new("docker")
        .args(["exec", container_name, "bash", "-lc", &mkdir_cmd])
        .status()
        .context("Failed to ensure node_modules path inside container")?;
    if !mkdir_status.success() {
        anyhow::bail!("Failed to create node_modules path in container");
    }

    // Copy contents of host node_modules into container path
    let src = format!("{}/.", host_nm.display());
    let dest = format!("{}:{}", container_name, container_nm);
    let cp_status = Command::new("docker")
        .args(["cp", &src, &dest])
        .status()
        .context("Failed to copy node_modules to container")?;
    if !cp_status.success() {
        anyhow::bail!("Copying node_modules to container failed");
    }

    startup_log::event("📦 Syncing host node_modules into container volume".to_string());

    Ok(())
}
