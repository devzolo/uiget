mod builder;
mod cli;
mod config;
mod installer;
mod package_manager;
mod registry;

use anyhow::Result;
use builder::RegistryBuilder;
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

    Commands::Build { ref registry, ref output } => {
      handle_build(&cli, registry, output)?;
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

  // Parse component name to extract namespace if in @namespace/component format
  let (parsed_component, parsed_registry) = if let Some(comp_name) = component {
    parse_component_with_namespace(comp_name, registry)
  } else {
    (component.map(|s| s.to_string()), registry.map(|s| s.to_string()))
  };

  installer
    .install_components(
      parsed_component.as_deref(), 
      parsed_registry.as_deref(), 
      force, 
      skip_deps
    )
    .await?;

  Ok(())
}

/// Parse component name to extract namespace if in @namespace/component format
/// Returns (component_name, registry_namespace)
fn parse_component_with_namespace(component_name: &str, existing_registry: Option<&str>) -> (Option<String>, Option<String>) {
  // If registry is already explicitly provided, use it as-is
  if let Some(registry) = existing_registry {
    return (Some(component_name.to_string()), Some(registry.to_string()));
  }

  // Check if component name contains @namespace/ pattern
  if component_name.starts_with('@') && component_name.contains('/') {
    if let Some(slash_pos) = component_name.find('/') {
      let namespace = &component_name[..slash_pos]; // includes the @
      let component = &component_name[slash_pos + 1..];
      
      // Only return if both parts are non-empty
      if !namespace.is_empty() && !component.is_empty() && namespace.len() > 1 {
        return (Some(component.to_string()), Some(namespace.to_string()));
      }
    }
  }

  // Default case: return component as-is
  (Some(component_name.to_string()), existing_registry.map(|s| s.to_string()))
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
      manager.add_registry_with_style(namespace.clone(), url.clone(), config.style.clone())?;

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
        for (namespace, registry_config) in &config.registries {
          println!("  {} {} -> {}", "→".blue(), namespace.cyan(), registry_config.url().blue());
        }
      }
    }

    RegistryAction::Test { namespace } => {
      if let Some(registry_config) = config.get_registry(&namespace) {
        println!("{} Testing registry '{}'...", "→".blue(), namespace.cyan());

        let mut manager = RegistryManager::new();
        manager.add_registry_config_with_style(namespace.clone(), registry_config.clone(), config.style.clone())?;

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

fn handle_build(_cli: &Cli, registry_path: &str, output_path: &str) -> Result<()> {
  use std::path::Path;

  let registry_path = Path::new(registry_path);
  let output_path = Path::new(output_path);

  if !registry_path.exists() {
    return Err(anyhow::anyhow!(
      "Registry file '{}' not found",
      registry_path.display()
    ));
  }

  println!(
    "{} Building components from {}...", 
    "→".blue(), 
    registry_path.display().to_string().cyan()
  );

  let builder = RegistryBuilder::new(registry_path, output_path)?;
  
  println!(
    "{} Building components to {}...",
    "→".blue(),
    output_path.display().to_string().cyan()
  );

  builder.build()?;

  println!();
  println!(
    "{} Registry built successfully!",
    "✓".green()
  );
  println!(
    "  {} Generated files in {}",
    "→".blue(),
    output_path.display().to_string().cyan()
  );

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
  use crate::config::RegistryConfig;

  fn create_test_config() -> (TempDir, Config) {
    let temp_dir = TempDir::new().unwrap();
    let mut config = Config::default();
    config
      .registries
      .insert("test".to_string(), RegistryConfig::String("https://example.com/registry/{name}.json".to_string()));
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
