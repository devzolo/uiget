use clap::{Parser, Subcommand};

/// A CLI tool for downloading shadcn components from multiple registries
#[derive(Parser)]
#[command(name = "uiget")]
#[command(about = "Download shadcn components from multiple registries")]
#[command(long_about = None)]
#[command(version)]
pub struct Cli {
  #[command(subcommand)]
  pub command: Commands,

  /// Path to configuration file
  #[arg(short, long, global = true)]
  pub config: Option<String>,

  /// Enable verbose output
  #[arg(short, long, global = true)]
  pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
  /// Initialize a new configuration file
  Init {
    /// Force overwrite existing configuration
    #[arg(short, long)]
    force: bool,

    /// Base color for the theme
    #[arg(long, default_value = "slate")]
    base_color: String,

    /// CSS file path
    #[arg(long, default_value = "src/app.css")]
    css: String,

    /// Components alias
    #[arg(long, default_value = "$lib/components")]
    components: String,

    /// Utils alias
    #[arg(long, default_value = "$lib/utils")]
    utils: String,
  },

  /// Add a component from a registry
  Add {
    /// Component name to add (optional - if not provided, shows interactive
    /// menu)
    component: Option<String>,

    /// Registry namespace to use (defaults to auto-detect)
    #[arg(short, long)]
    registry: Option<String>,

    /// Skip dependency installation
    #[arg(long)]
    skip_deps: bool,

    /// Overwrite existing files
    #[arg(short, long)]
    force: bool,
  },

  /// Remove a component
  Remove {
    /// Component name to remove
    component: String,
  },

  /// List available components
  List {
    /// Registry namespace to list from
    #[arg(short, long)]
    registry: Option<String>,

    /// Category to filter by
    #[arg(long)]
    category: Option<String>,
  },

  /// Search for components
  Search {
    /// Search query
    query: String,

    /// Registry namespace to search in
    #[arg(short, long)]
    registry: Option<String>,
  },

  /// Manage registries
  Registry {
    #[command(subcommand)]
    action: RegistryAction,
  },

  /// Update components to latest versions
  Update {
    /// Specific component to update
    component: Option<String>,

    /// Registry namespace
    #[arg(short, long)]
    registry: Option<String>,
  },

  /// Show information about a component
  Info {
    /// Component name
    component: String,

    /// Registry namespace
    #[arg(short, long)]
    registry: Option<String>,
  },

  /// List outdated components
  Outdated {
    /// Registry namespace to check
    #[arg(short, long)]
    registry: Option<String>,
  },
}

#[derive(Subcommand)]
pub enum RegistryAction {
  /// Add a new registry
  Add {
    /// Registry namespace
    namespace: String,

    /// Registry URL
    url: String,
  },

  /// Remove a registry
  Remove {
    /// Registry namespace
    namespace: String,
  },

  /// List all registries
  List,

  /// Test registry connection
  Test {
    /// Registry namespace to test
    namespace: String,
  },
}

impl Cli {
  /// Get the configuration file path
  pub fn config_path(&self) -> std::path::PathBuf {
    if let Some(config_path) = &self.config {
      std::path::PathBuf::from(config_path)
    } else {
      // Default to current directory
      let current_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
      
      // Try uiget.json first
      let uiget_path = current_dir.join("uiget.json");
      if uiget_path.exists() {
        return uiget_path;
      }
      
      // Fallback to components.json (shadcn default)
      let components_path = current_dir.join("components.json");
      if components_path.exists() {
        return components_path;
      }
      
      // Return uiget.json as default for new configurations
      uiget_path
    }
  }

  /// Check if verbose mode is enabled
  pub fn is_verbose(&self) -> bool {
    self.verbose
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
  }
}
