mod cli;
mod config;
mod installer;
mod registry;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, RegistryAction};
use colored::*;
use config::Config;
use installer::ComponentInstaller;
use registry::RegistryManager;

#[tokio::main]
async fn main() -> Result<()> {
  let cli = Cli::parse();

  // Setup error handling and logging
  if std::env::var("RUST_LOG").is_err() {
    std::env::set_var("RUST_LOG", if cli.is_verbose() { "debug" } else { "info" });
  }

  match cli.command {
    Commands::Init {
      force,
      ref base_color,
      ref css,
      ref components,
      ref utils,
    } => {
      handle_init(&cli, force, base_color, css, components, utils).await?;
    }

    Commands::Add {
      ref component,
      ref registry,
      skip_deps,
      force,
    } => {
      handle_add(
        &cli,
        component.as_deref(),
        registry.as_deref(),
        skip_deps,
        force,
      )
      .await?;
    }

    Commands::Remove { ref component } => {
      handle_remove(&cli, component).await?;
    }

    Commands::List {
      ref registry,
      category: _,
    } => {
      handle_list(&cli, registry.as_deref()).await?;
    }

    Commands::Search {
      ref query,
      ref registry,
    } => {
      handle_search(&cli, query, registry.as_deref()).await?;
    }

    Commands::Registry { ref action } => {
      handle_registry(&cli, action).await?;
    }

    Commands::Update {
      component: _,
      registry: _,
    } => {
      println!("{} Update command not implemented yet", "!".yellow());
    }

    Commands::Info {
      ref component,
      ref registry,
    } => {
      handle_info(&cli, component, registry.as_deref()).await?;
    }

    Commands::Outdated { ref registry } => {
      handle_outdated(&cli, registry.as_deref()).await?;
    }
  }

  Ok(())
}

async fn handle_init(
  cli: &Cli,
  force: bool,
  base_color: &str,
  css: &str,
  components: &str,
  utils: &str,
) -> Result<()> {
  let config_path = cli.config_path();

  if config_path.exists() && !force {
    return Err(anyhow::anyhow!(
      "Configuration file '{}' already exists. Use --force to overwrite",
      config_path.display()
    ));
  }

  println!("{} Initializing uiget configuration...", "→".blue());

  let mut config = Config::default();
  config.tailwind.base_color = base_color.to_string();
  config.tailwind.css = css.to_string();
  config.aliases.components = components.to_string();
  config.aliases.utils = utils.to_string();

  config.save_to_file(&config_path)?;

  println!(
    "{} Configuration saved to {}",
    "✓".green(),
    config_path.display().to_string().cyan()
  );
  println!(
    "  You can now add components with: {} {}",
    "uiget add".cyan(),
    "<component-name>".yellow()
  );

  Ok(())
}

async fn handle_add(
  cli: &Cli,
  component: Option<&str>,
  registry: Option<&str>,
  skip_deps: bool,
  force: bool,
) -> Result<()> {
  let config = load_config(cli)?;
  let installer = ComponentInstaller::new(config)?;

  installer
    .install_components(component, registry, force, skip_deps)
    .await?;

  Ok(())
}

async fn handle_remove(cli: &Cli, component: &str) -> Result<()> {
  let config = load_config(cli)?;
  let installer = ComponentInstaller::new(config)?;

  installer.remove_component(component)?;

  Ok(())
}

async fn handle_list(cli: &Cli, registry: Option<&str>) -> Result<()> {
  let config = load_config(cli)?;
  let installer = ComponentInstaller::new(config)?;

  installer.list_components(registry).await?;

  Ok(())
}

async fn handle_search(cli: &Cli, query: &str, registry: Option<&str>) -> Result<()> {
  let config = load_config(cli)?;
  let installer = ComponentInstaller::new(config)?;

  println!("{} Searching for '{}'...", "→".blue(), query.cyan());
  installer.search_components(query, registry).await?;

  Ok(())
}

async fn handle_registry(cli: &Cli, action: &RegistryAction) -> Result<()> {
  let config_path = cli.config_path();
  let mut config = load_config(cli)?;

  match action {
    RegistryAction::Add { namespace, url } => {
      // Validate URL by creating a registry client
      let mut manager = RegistryManager::new();
      manager.add_registry(namespace.clone(), url.clone())?;

      // Add to config
      config.set_registry(namespace.clone(), url.clone());
      config.save_to_file(&config_path)?;

      println!(
        "{} Added registry '{}' -> {}",
        "✓".green(),
        namespace.cyan(),
        url.blue()
      );
    }

    RegistryAction::Remove { namespace } => {
      if config.registries.remove(namespace).is_some() {
        config.save_to_file(&config_path)?;
        println!("{} Removed registry '{}'", "✓".green(), namespace.cyan());
      } else {
        println!("{} Registry '{}' not found", "!".yellow(), namespace.cyan());
      }
    }

    RegistryAction::List => {
      if config.registries.is_empty() {
        println!("{} No registries configured", "!".yellow());
      } else {
        println!("{} Configured registries:", "📦".blue());
        for (namespace, url) in &config.registries {
          println!("  {} {} -> {}", "→".blue(), namespace.cyan(), url.blue());
        }
      }
    }

    RegistryAction::Test { namespace } => {
      if let Some(url) = config.get_registry_url(&namespace) {
        println!("{} Testing registry '{}'...", "→".blue(), namespace.cyan());

        let mut manager = RegistryManager::new();
        manager.add_registry(namespace.clone(), url.clone())?;

        if let Some(registry) = manager.get_registry(&namespace) {
          match registry.fetch_index().await {
            Ok(index) => {
              println!(
                "{} Registry '{}' is working ({} components available)",
                "✓".green(),
                namespace.cyan(),
                index.len().to_string().yellow()
              );
            }
            Err(e) => {
              println!(
                "{} Registry '{}' failed: {}",
                "✗".red(),
                namespace.cyan(),
                e
              );
            }
          }
        } else {
          println!("{} Failed to create registry client", "✗".red());
        }
      } else {
        println!("{} Registry '{}' not found", "!".yellow(), namespace.cyan());
      }
    }
  }

  Ok(())
}

async fn handle_info(cli: &Cli, component: &str, registry: Option<&str>) -> Result<()> {
  let config = load_config(cli)?;
  let installer = ComponentInstaller::new(config)?;

  installer.show_component_info(component, registry).await?;

  Ok(())
}

async fn handle_outdated(cli: &Cli, registry: Option<&str>) -> Result<()> {
  let config = load_config(cli)?;
  let installer = ComponentInstaller::new(config)?;

  println!("{} Checking for outdated components...", "→".blue());

  let installed_components = installer.get_installed_components()?;

  if installed_components.is_empty() {
    println!("{} No components installed", "!".yellow());
    return Ok(());
  }

  let outdated_results = installer
    .check_outdated_components(&installed_components, registry)
    .await?;

  let outdated_components: Vec<&String> = outdated_results
    .iter()
    .filter_map(|(name, is_outdated)| if *is_outdated { Some(name) } else { None })
    .collect();

  if outdated_components.is_empty() {
    println!("{} All components are up to date!", "✓".green());
  } else {
    println!(
      "\n{} Found {} outdated component(s):",
      "⚠".yellow(),
      outdated_components.len().to_string().yellow()
    );

    for component in outdated_components {
      println!("  {} {} {}", "→".dimmed(), "⚠".yellow(), component.yellow());
    }

    println!(
      "\n{} Run {} to update components",
      "💡".blue(),
      "uiget add <component> --force".cyan()
    );
  }

  Ok(())
}

fn load_config(cli: &Cli) -> Result<Config> {
  let config_path = cli.config_path();

  if !config_path.exists() {
    // Check if we're looking for a specific config file or using defaults
    if cli.config.is_some() {
      return Err(anyhow::anyhow!(
        "Configuration file '{}' not found.",
        config_path.display()
      ));
    } else {
      // No uiget.json or components.json found
      return Err(anyhow::anyhow!(
        "No configuration file found. Looked for 'uiget.json' and 'components.json'. Run 'uiget init' to create one."
      ));
    }
  }

  let config = Config::load_from_file(&config_path)?;
  
  // Show which config file is being used for transparency
  if cli.is_verbose() {
    println!("Using configuration from: {}", config_path.display());
  }
  
  Ok(config)
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;

  use super::*;

  fn create_test_config() -> (TempDir, Config) {
    let temp_dir = TempDir::new().unwrap();
    let mut config = Config::default();
    config
      .registries
      .insert("test".to_string(), "https://example.com".to_string());
    (temp_dir, config)
  }

  #[test]
  fn test_config_loading() {
    let (temp_dir, config) = create_test_config();
    let config_path = temp_dir.path().join("uiget.json");

    config.save_to_file(&config_path).unwrap();

    let loaded_config = Config::load_from_file(&config_path).unwrap();
    assert_eq!(
      config.tailwind.base_color,
      loaded_config.tailwind.base_color
    );
    assert_eq!(config.registries.len(), loaded_config.registries.len());
  }
}
