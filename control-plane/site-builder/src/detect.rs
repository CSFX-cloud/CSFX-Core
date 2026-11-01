use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Static,
    Container,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Detection {
    pub framework: &'static str,
    pub mode: Mode,
    pub install_command: Option<String>,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
    pub start_command: Option<String>,
    pub port: Option<u16>,
}

pub struct RepoSnapshot {
    pub files: HashSet<String>,
    pub package_json: Option<Value>,
}

#[derive(Clone, Copy)]
enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    fn detect(files: &HashSet<String>) -> Self {
        let has = |name: &str| files.contains(name);
        if has("pnpm-lock.yaml") {
            Self::Pnpm
        } else if has("yarn.lock") {
            Self::Yarn
        } else if has("bun.lock") || has("bun.lockb") {
            Self::Bun
        } else {
            Self::Npm
        }
    }

    fn install(self) -> &'static str {
        match self {
            Self::Npm => "npm ci",
            Self::Pnpm => "pnpm install --frozen-lockfile",
            Self::Yarn => "yarn install --frozen-lockfile",
            Self::Bun => "bun install --frozen-lockfile",
        }
    }

    fn run(self, script: &str) -> String {
        match self {
            Self::Npm => format!("npm run {script}"),
            Self::Pnpm => format!("pnpm run {script}"),
            Self::Yarn => format!("yarn {script}"),
            Self::Bun => format!("bun run {script}"),
        }
    }
}

struct Packages<'a> {
    json: &'a Value,
}

impl Packages<'_> {
    fn depends_on(&self, name: &str) -> bool {
        ["dependencies", "devDependencies"]
            .iter()
            .any(|section| self.json[section].get(name).is_some())
    }

    fn has_script(&self, name: &str) -> bool {
        self.json["scripts"].get(name).is_some()
    }
}

fn node_static(framework: &'static str, manager: PackageManager, output_dir: &str) -> Detection {
    Detection {
        framework,
        mode: Mode::Static,
        install_command: Some(manager.install().to_string()),
        build_command: Some(manager.run("build")),
        output_dir: Some(output_dir.to_string()),
        start_command: None,
        port: None,
    }
}

fn node_server(
    framework: &'static str,
    manager: PackageManager,
    start_command: String,
    port: u16,
) -> Detection {
    Detection {
        framework,
        mode: Mode::Container,
        install_command: Some(manager.install().to_string()),
        build_command: Some(manager.run("build")),
        output_dir: None,
        start_command: Some(start_command),
        port: Some(port),
    }
}

fn detect_node(packages: &Packages, manager: PackageManager) -> Option<Detection> {
    let serve = |script: &str| manager.run(script);
    if packages.depends_on("@sveltejs/kit") {
        return Some(sveltekit(packages, manager));
    }
    if packages.depends_on("next") {
        return Some(node_server("next", manager, serve("start"), 3000));
    }
    if packages.depends_on("nuxt") {
        let start = "node .output/server/index.mjs".to_string();
        return Some(node_server("nuxt", manager, start, 3000));
    }
    if packages.depends_on("astro") {
        return Some(astro(packages, manager));
    }
    if packages.depends_on("@docusaurus/core") {
        return Some(node_static("docusaurus", manager, "build"));
    }
    if packages.depends_on("vite") {
        return Some(node_static("vite", manager, "dist"));
    }
    if packages.depends_on("react-scripts") {
        return Some(node_static("create-react-app", manager, "build"));
    }
    generic_node(packages, manager)
}

fn sveltekit(packages: &Packages, manager: PackageManager) -> Detection {
    if packages.depends_on("@sveltejs/adapter-node") {
        return node_server("sveltekit", manager, "node build".to_string(), 3000);
    }
    node_static("sveltekit-static", manager, "build")
}

fn astro(packages: &Packages, manager: PackageManager) -> Detection {
    if packages.depends_on("@astrojs/node") {
        let start = "node ./dist/server/entry.mjs".to_string();
        return node_server("astro-server", manager, start, 4321);
    }
    node_static("astro", manager, "dist")
}

fn generic_node(packages: &Packages, manager: PackageManager) -> Option<Detection> {
    if packages.has_script("build") {
        return Some(node_static("node", manager, "dist"));
    }
    if packages.has_script("start") {
        return Some(Detection {
            framework: "node",
            mode: Mode::Container,
            install_command: Some(manager.install().to_string()),
            build_command: None,
            output_dir: None,
            start_command: Some(manager.run("start")),
            port: Some(3000),
        });
    }
    None
}

fn detect_hugo(files: &HashSet<String>) -> Option<Detection> {
    let configured = ["hugo.toml", "hugo.yaml", "hugo.json", "config.toml"]
        .iter()
        .any(|name| files.contains(*name));
    configured.then(|| Detection {
        framework: "hugo",
        mode: Mode::Static,
        install_command: None,
        build_command: Some("hugo --minify".to_string()),
        output_dir: Some("public".to_string()),
        start_command: None,
        port: None,
    })
}

fn detect_jekyll(files: &HashSet<String>) -> Option<Detection> {
    (files.contains("_config.yml") && files.contains("Gemfile")).then(|| Detection {
        framework: "jekyll",
        mode: Mode::Static,
        install_command: Some("bundle install".to_string()),
        build_command: Some("bundle exec jekyll build".to_string()),
        output_dir: Some("_site".to_string()),
        start_command: None,
        port: None,
    })
}

fn plain_static(files: &HashSet<String>) -> Option<Detection> {
    files.contains("index.html").then(|| Detection {
        framework: "static",
        mode: Mode::Static,
        install_command: None,
        build_command: None,
        output_dir: Some(".".to_string()),
        start_command: None,
        port: None,
    })
}

fn dockerfile() -> Detection {
    Detection {
        framework: "dockerfile",
        mode: Mode::Container,
        install_command: None,
        build_command: None,
        output_dir: None,
        start_command: None,
        port: None,
    }
}

pub fn detect(snapshot: &RepoSnapshot) -> Option<Detection> {
    if snapshot.files.contains("Dockerfile") {
        return Some(dockerfile());
    }
    if let Some(json) = &snapshot.package_json {
        let manager = PackageManager::detect(&snapshot.files);
        return detect_node(&Packages { json }, manager);
    }
    detect_hugo(&snapshot.files)
        .or_else(|| detect_jekyll(&snapshot.files))
        .or_else(|| plain_static(&snapshot.files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn snapshot(files: &[&str], package_json: Option<Value>) -> RepoSnapshot {
        RepoSnapshot {
            files: files.iter().map(|name| name.to_string()).collect(),
            package_json,
        }
    }

    fn node(dependencies: Value, files: &[&str]) -> Option<Detection> {
        detect(&snapshot(
            files,
            Some(json!({ "dependencies": dependencies, "scripts": { "build": "x" } })),
        ))
    }

    #[test]
    fn dockerfile_wins_over_everything() {
        let found = detect(&snapshot(&["Dockerfile", "index.html"], None)).unwrap();
        assert_eq!(found.framework, "dockerfile");
        assert_eq!(found.mode, Mode::Container);
    }

    #[test]
    fn vite_is_static_with_dist_output() {
        let found = node(json!({ "vite": "5" }), &["pnpm-lock.yaml"]).unwrap();
        assert_eq!(found.mode, Mode::Static);
        assert_eq!(found.output_dir.as_deref(), Some("dist"));
        assert_eq!(found.build_command.as_deref(), Some("pnpm run build"));
        assert_eq!(
            found.install_command.as_deref(),
            Some("pnpm install --frozen-lockfile")
        );
    }

    #[test]
    fn sveltekit_adapter_decides_the_mode() {
        let server = node(
            json!({ "@sveltejs/kit": "2", "@sveltejs/adapter-node": "5" }),
            &[],
        )
        .unwrap();
        assert_eq!(server.mode, Mode::Container);
        assert_eq!(server.start_command.as_deref(), Some("node build"));
        assert_eq!(server.port, Some(3000));

        let fixed = node(
            json!({ "@sveltejs/kit": "2", "@sveltejs/adapter-static": "3" }),
            &[],
        )
        .unwrap();
        assert_eq!(fixed.mode, Mode::Static);
        assert_eq!(fixed.output_dir.as_deref(), Some("build"));
    }

    #[test]
    fn astro_with_node_adapter_runs_as_container() {
        let found = node(json!({ "astro": "4", "@astrojs/node": "8" }), &[]).unwrap();
        assert_eq!(found.mode, Mode::Container);
        assert_eq!(found.port, Some(4321));
        let fixed = node(json!({ "astro": "4" }), &[]).unwrap();
        assert_eq!(fixed.mode, Mode::Static);
    }

    #[test]
    fn next_and_nuxt_run_as_containers() {
        let next = node(json!({ "next": "15" }), &["yarn.lock"]).unwrap();
        assert_eq!(next.start_command.as_deref(), Some("yarn start"));
        let nuxt = node(json!({ "nuxt": "3" }), &[]).unwrap();
        assert_eq!(nuxt.mode, Mode::Container);
    }

    #[test]
    fn docusaurus_and_create_react_app_are_static() {
        let docs = node(json!({ "@docusaurus/core": "3" }), &[]).unwrap();
        assert_eq!(docs.framework, "docusaurus");
        let cra = node(json!({ "react-scripts": "5" }), &[]).unwrap();
        assert_eq!(cra.output_dir.as_deref(), Some("build"));
    }

    #[test]
    fn plain_node_falls_back_to_scripts() {
        let build_only = detect(&snapshot(&[], Some(json!({ "scripts": { "build": "x" } }))));
        assert_eq!(build_only.unwrap().mode, Mode::Static);
        let start_only = detect(&snapshot(&[], Some(json!({ "scripts": { "start": "x" } }))));
        assert_eq!(start_only.unwrap().mode, Mode::Container);
        assert!(detect(&snapshot(&[], Some(json!({})))).is_none());
    }

    #[test]
    fn hugo_jekyll_and_plain_html_are_static() {
        assert_eq!(
            detect(&snapshot(&["hugo.toml"], None)).unwrap().framework,
            "hugo"
        );
        let jekyll = detect(&snapshot(&["_config.yml", "Gemfile"], None)).unwrap();
        assert_eq!(jekyll.output_dir.as_deref(), Some("_site"));
        let plain = detect(&snapshot(&["index.html"], None)).unwrap();
        assert_eq!(plain.output_dir.as_deref(), Some("."));
        assert!(detect(&snapshot(&["README.md"], None)).is_none());
    }
}
