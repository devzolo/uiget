use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::{anyhow, Result};
use colored::*;
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect, Select};
use sha2::{Digest, Sha256};

use crate::{
  config::{Config, ResolvedPaths},
  package_manager::{detect_package_manager, Detection},
  registry::{Component, ComponentFile, RegistryManager},
};

/// Component installer handles downloading and installing components
pub struct ComponentInstaller {
  config: Config,
  registry_manager: RegistryManager,
  typescript_paths: Option<ResolvedPaths>,
  package_manager: Option<Detection>,
}

/// Component installation context with type information
#[derive(Debug, Clone)]
pub struct ComponentContext {
  pub name: String,
  pub component_type: Option<String>,
  pub registry: Option<String>,
}

/// Dependencies to be installed
#[derive(Debug, Clone)]
pub struct ComponentDependencies {
  pub dependencies: Vec<String>,
  pub dev_dependencies: Vec<String>,
}

impl ComponentInstaller {
  /// Create a new component installer
  pub fn new(config: Config) -> Result<Self> {
    let mut registry_manager = RegistryManager::new();

    // Add all registries from config
    for (namespace, registry_config) in &config.registries {
      registry_manager.add_registry_config_with_style(
        namespace.clone(),
        registry_config.clone(),
        config.style.clone(),
      )?;
    }

    // Resolve TypeScript paths if TypeScript is enabled
    let typescript_paths = config.resolve_typescript_paths().unwrap_or(None);

    // Detect package manager
    let package_manager = match detect_package_manager(std::env::current_dir()?) {
      Ok(detection) => {
        println!("{} {}", "📦".blue(), detection.info());
        Some(detection)
      }
      Err(e) => {
        eprintln!("{} Failed to detect package manager: {:?}", "!".yellow(), e);
        None
      }
    };

    Ok(Self {
      config,
      registry_manager,
      typescript_paths,
      package_manager,
    })
  }

  /// Get the appropriate alias path based on component type
  fn get_alias_for_component_type(&self, component_type: Option<&str>) -> &str {
    match component_type {
      Some("registry:hook") => self
        .config
        .aliases
        .hooks
        .as_deref()
        .unwrap_or(&self.config.aliases.components),
      Some("registry:ui") => self
        .config
        .aliases
        .ui
        .as_deref()
        .unwrap_or(&self.config.aliases.components),
      Some("registry:util") => &self.config.aliases.utils,
      Some("registry:lib") => self
        .config
        .aliases
        .lib
        .as_deref()
        .unwrap_or(&self.config.aliases.components),
      _ => &self.config.aliases.components, // Default fallback
    }
  }

  /// Create component context from component information
  fn create_component_context(&self, component: &Component) -> ComponentContext {
    ComponentContext {
      name: component.name.clone(),
      component_type: component.component_type.clone(),
      registry: component.registry.clone(),
    }
  }

  /// Install components with optional interactive selection
  pub async fn install_components(
    &self,
    component_name: Option<&str>,
    registry_namespace: Option<&str>,
    force: bool,
    skip_deps: bool,
  ) -> Result<()> {
    if let Some(name) = component_name {
      // Install specific component
      self
        .install_component(name, registry_namespace, force, skip_deps)
        .await
    } else {
      // Show interactive menu
      self
        .interactive_component_selection(registry_namespace, force, skip_deps)
        .await
    }
  }

  /// Install a component
  pub async fn install_component(
    &self,
    component_name: &str,
    registry_namespace: Option<&str>,
    force: bool,
    skip_deps: bool,
  ) -> Result<()> {
    Box::pin(self.install_component_inner(component_name, registry_namespace, force, skip_deps))
      .await
  }

  /// Internal recursive installation function
  async fn install_component_inner(
    &self,
    component_name: &str,
    registry_namespace: Option<&str>,
    force: bool,
    skip_deps: bool,
  ) -> Result<()> {
    println!(
      "{} Installing component '{}'...",
      "→".blue(),
      component_name.cyan()
    );

    // Fetch component
    let component = if let Some(namespace) = registry_namespace {
      self
        .registry_manager
        .fetch_component(namespace, component_name)
        .await?
    } else {
      self
        .registry_manager
        .fetch_component_auto(component_name)
        .await?
    };

    // Install dependencies first (if not skipped)
    if !skip_deps {
      if let Some(dependencies) = &component.registry_dependencies {
        for dep in dependencies {
          println!("{} Installing dependency '{}'...", "→".yellow(), dep.cyan());
          Box::pin(self.install_component_inner(dep, registry_namespace, force, true)).await?;
        }
      }
    }

    // Create component context for proper alias resolution
    let component_context = self.create_component_context(&component);

    // Install component files with context
    self.install_component_files(&component, &component_context, force)?;

    // Install dependencies if component has any dependencies and package manager
    // was detected
    let deps = ComponentDependencies {
      dependencies: component.dependencies.clone().unwrap_or_default(),
      dev_dependencies: component.dev_dependencies.clone().unwrap_or_default(),
    };

    if !deps.dependencies.is_empty() || !deps.dev_dependencies.is_empty() {
      self.install_dependencies(&deps)?;
    }

    println!(
      "{} Successfully installed '{}'",
      "✓".green(),
      component_name.cyan()
    );
    Ok(())
  }

  /// Interactive component selection menu
  async fn interactive_component_selection(
    &self,
    registry_namespace: Option<&str>,
    force: bool,
    skip_deps: bool,
  ) -> Result<()> {
    // Determine which registry to use
    let namespace = if let Some(ns) = registry_namespace {
      ns.to_string()
    } else {
      // Let user select registry if multiple are available
      let registries: Vec<String> = self
        .registry_manager
        .namespaces()
        .into_iter()
        .cloned()
        .collect();

      if registries.is_empty() {
        return Err(anyhow!(
          "No registries configured. Run 'uiget registry add' first."
        ));
      }

      if registries.len() == 1 {
        registries[0].clone()
      } else {
        let selection = Select::with_theme(&ColorfulTheme::default())
          .with_prompt("Select a registry:")
          .items(&registries)
          .default(0)
          .interact()?;

        registries[selection].clone()
      }
    };

    // Fetch components from selected registry
    let registry = self
      .registry_manager
      .get_registry(&namespace)
      .ok_or_else(|| anyhow!("Registry '{}' not found", namespace))?;

    println!(
      "{} Fetching components from '{}'...",
      "→".blue(),
      namespace.cyan()
    );
    let index = registry.fetch_index().await?;

    if index.is_empty() {
      println!(
        "{} No components available in registry '{}'",
        "!".yellow(),
        namespace.cyan()
      );
      return Ok(());
    }

    // Get list of installed components
    let installed_components = self.get_installed_components().unwrap_or_default();

    // Pre-load outdated status for all installed components
    println!("{} Checking component status...", "→".blue());
    let outdated_results = self
      .check_outdated_components(&installed_components, Some(&namespace))
      .await
      .unwrap_or_default();

    let outdated_components: std::collections::HashSet<String> = outdated_results
      .into_iter()
      .filter_map(|(name, is_outdated)| if is_outdated { Some(name) } else { None })
      .collect();

    // Group components by type
    let mut ui_components = Vec::new();
    let mut blocks = Vec::new();
    let mut hooks = Vec::new();
    let mut libs = Vec::new();
    let mut other = Vec::new();

    for component in index.as_slice() {
      match component.component_type.as_deref() {
        Some("registry:ui") => ui_components.push(component),
        Some("registry:block") => blocks.push(component),
        Some("registry:hook") => hooks.push(component),
        Some("registry:lib") => libs.push(component),
        _ => other.push(component),
      }
    }

    // Create display items with categories and track category indices
    let mut display_items = Vec::new();
    let mut component_map = Vec::new();
    let mut category_ranges = Vec::new(); // (category_index, start_index, end_index)

    if !ui_components.is_empty() {
      let category_index = display_items.len();
      display_items.push(format!("📦 UI Components ({})", ui_components.len()));
      component_map.push(None); // Category header

      let start_index = display_items.len();
      for component in &ui_components {
        let is_installed = installed_components.contains(&component.name);
        let status_icon = if is_installed {
          if outdated_components.contains(&component.name) {
            "⚠"
          } else {
            "✓"
          }
        } else {
          " "
        };
        display_items.push(format!(
          "  {} {} {}",
          "→".dimmed(),
          status_icon,
          component.name
        ));
        component_map.push(Some(*component));
      }
      let end_index = display_items.len() - 1;
      category_ranges.push((category_index, start_index, end_index));
    }

    if !blocks.is_empty() {
      let category_index = display_items.len();
      display_items.push(format!("🧩 Blocks ({})", blocks.len()));
      component_map.push(None); // Category header

      let start_index = display_items.len();
      for component in &blocks {
        let is_installed = installed_components.contains(&component.name);
        let status_icon = if is_installed {
          if outdated_components.contains(&component.name) {
            "⚠"
          } else {
            "✓"
          }
        } else {
          " "
        };
        display_items.push(format!(
          "  {} {} {}",
          "→".dimmed(),
          status_icon,
          component.name
        ));
        component_map.push(Some(*component));
      }
      let end_index = display_items.len() - 1;
      category_ranges.push((category_index, start_index, end_index));
    }

    if !hooks.is_empty() {
      let category_index = display_items.len();
      display_items.push(format!("🪝 Hooks ({})", hooks.len()));
      component_map.push(None); // Category header

      let start_index = display_items.len();
      for component in &hooks {
        let is_installed = installed_components.contains(&component.name);
        let status_icon = if is_installed {
          if outdated_components.contains(&component.name) {
            "⚠"
          } else {
            "✓"
          }
        } else {
          " "
        };
        display_items.push(format!(
          "  {} {} {}",
          "→".dimmed(),
          status_icon,
          component.name
        ));
        component_map.push(Some(*component));
      }
      let end_index = display_items.len() - 1;
      category_ranges.push((category_index, start_index, end_index));
    }

    if !libs.is_empty() {
      let category_index = display_items.len();
      display_items.push(format!("📚 Libraries ({})", libs.len()));
      component_map.push(None); // Category header

      let start_index = display_items.len();
      for component in &libs {
        let is_installed = installed_components.contains(&component.name);
        let status_icon = if is_installed {
          if outdated_components.contains(&component.name) {
            "⚠"
          } else {
            "✓"
          }
        } else {
          " "
        };
        display_items.push(format!(
          "  {} {} {}",
          "→".dimmed(),
          status_icon,
          component.name
        ));
        component_map.push(Some(*component));
      }
      let end_index = display_items.len() - 1;
      category_ranges.push((category_index, start_index, end_index));
    }

    if !other.is_empty() {
      let category_index = display_items.len();
      display_items.push(format!("⚙️ Other ({})", other.len()));
      component_map.push(None); // Category header

      let start_index = display_items.len();
      for component in &other {
        let is_installed = installed_components.contains(&component.name);
        let status_icon = if is_installed {
          if outdated_components.contains(&component.name) {
            "⚠"
          } else {
            "✓"
          }
        } else {
          " "
        };
        display_items.push(format!(
          "  {} {} {}",
          "→".dimmed(),
          status_icon,
          component.name
        ));
        component_map.push(Some(*component));
      }
      let end_index = display_items.len() - 1;
      category_ranges.push((category_index, start_index, end_index));
    }

    // First, show category selection menu
    let mut category_options = vec!["🔍 Browse and select individual components".to_string()];
    let mut category_data = vec![None]; // None for individual browsing

    if !ui_components.is_empty() {
      category_options.push(format!(
        "📦 Select ALL UI Components ({} items)",
        ui_components.len()
      ));
      category_data.push(Some(("ui", &ui_components)));
    }

    if !blocks.is_empty() {
      category_options.push(format!("🧩 Select ALL Blocks ({} items)", blocks.len()));
      category_data.push(Some(("blocks", &blocks)));
    }

    if !hooks.is_empty() {
      category_options.push(format!("🪝 Select ALL Hooks ({} items)", hooks.len()));
      category_data.push(Some(("hooks", &hooks)));
    }

    if !libs.is_empty() {
      category_options.push(format!("📚 Select ALL Libraries ({} items)", libs.len()));
      category_data.push(Some(("libs", &libs)));
    }

    if !other.is_empty() {
      category_options.push(format!("⚙️ Select ALL Other ({} items)", other.len()));
      category_data.push(Some(("other", &other)));
    }

    category_options.push("❌ Cancel".to_string());
    category_data.push(None);

    let choice = Select::with_theme(&ColorfulTheme::default())
      .with_prompt("What would you like to do?")
      .items(&category_options)
      .default(0)
      .interact()?;

    let selected_components: Vec<&crate::registry::ComponentInfo> = match category_data.get(choice)
    {
      Some(Some((category_name, components))) => {
        // Bulk selection confirmed
        println!(
          "\n{} Selected ALL {} ({} components)",
          "✅".green(),
          category_name,
          components.len()
        );

        // Show preview of what will be installed
        println!("Components to be installed:");
        for (i, component) in components.iter().enumerate() {
          println!(
            "  {}. {}",
            (i + 1).to_string().dimmed(),
            component.name.cyan()
          );
          if i >= 9 {
            println!(
              "  ... and {} more",
              (components.len() - 10).to_string().dimmed()
            );
            break;
          }
        }

        if !Confirm::with_theme(&ColorfulTheme::default())
          .with_prompt(&format!("Install all {} components?", components.len()))
          .default(true)
          .interact()?
        {
          println!("{} Installation cancelled", "❌".red());
          return Ok(());
        }

        components.iter().copied().collect()
      }
      Some(None) if choice == 0 => {
        // Individual component selection
        println!("\n{} Component Browser", "🔍".blue());
        println!(
          "{}",
          "Use ↑↓ to navigate, Space to select multiple, Enter to confirm".dimmed()
        );

        let selections = MultiSelect::with_theme(&ColorfulTheme::default())
          .with_prompt("Select components to install:")
          .items(&display_items)
          .interact()?;

        // Filter out category headers and get components
        selections
          .into_iter()
          .filter_map(|i| component_map.get(i).and_then(|opt| *opt))
          .collect()
      }
      _ => {
        // Cancel
        println!("{} Operation cancelled", "👋".yellow());
        return Ok(());
      }
    };

    if selected_components.is_empty() {
      println!("{} No components selected", "!".yellow());
      return Ok(());
    }

    // Install selected components
    println!(
      "\n{} Installing {} component(s)...",
      "→".blue(),
      selected_components.len().to_string().cyan()
    );

    for component in selected_components {
      println!();
      self
        .install_component(&component.name, Some(&namespace), force, skip_deps)
        .await?;
    }

    println!(
      "\n{} All selected components installed successfully!",
      "✓".green()
    );

    Ok(())
  }

  /// Install component files to the filesystem
  fn install_component_files(
    &self,
    component: &Component,
    context: &ComponentContext,
    force: bool,
  ) -> Result<()> {
    for file in &component.files {
      self.install_file(file, context, force)?;
    }
    Ok(())
  }

  /// Install a single file
  fn install_file(
    &self,
    file: &ComponentFile,
    context: &ComponentContext,
    force: bool,
  ) -> Result<()> {
    let target_path = self.resolve_file_path(&file.get_target_path(), context)?;

    // Check if file exists and force is not enabled
    if target_path.exists() && !force {
      return Err(anyhow!(
        "File '{}' already exists. Use --force to overwrite",
        target_path.display()
      ));
    }

    // Create directory if it doesn't exist
    if let Some(parent) = target_path.parent() {
      fs::create_dir_all(parent)?;
    }

    // Process placeholders in file content with component context
    let processed_content = self.process_placeholders(&file.content, Some(context))?;

    // Write processed file content
    fs::write(&target_path, processed_content)?;

    println!(
      "  {} {}",
      "✓".green(),
      target_path.display().to_string().dimmed()
    );

    Ok(())
  }

  /// Resolve file path using aliases and component target paths
  fn resolve_file_path(&self, target: &str, context: &ComponentContext) -> Result<PathBuf> {
    // The target format is like "button/button.svelte" or "button/index.ts"
    // We need to place this in the appropriate directory based on component type

    let alias_path = self.get_alias_for_component_type(context.component_type.as_deref());

    // First try to resolve using TypeScript paths if available
    let resolved_alias_path = if let Some(ref ts_paths) = self.typescript_paths {
      self.resolve_path_with_typescript(alias_path, &ts_paths.paths)
    } else {
      // Fallback to manual resolution
      self.resolve_path_manually(alias_path)
    };

    // Handle path normalization for different component types
    let normalized_target = if context.component_type.as_deref() == Some("registry:ui")
      && target.starts_with("ui/")
      && resolved_alias_path.ends_with("/ui")
    {
      // Remove "ui/" prefix from target to avoid duplication for UI components
      target.strip_prefix("ui/").unwrap_or(target)
    } else {
      target
    };

    let resolved_path = format!("{}/{}", resolved_alias_path, normalized_target);

    // Convert to absolute path
    let current_dir = std::env::current_dir()?;
    let path = current_dir.join(&resolved_path);

    Ok(path)
  }

  /// Resolve path using TypeScript path mappings
  fn resolve_path_with_typescript(
    &self,
    ui_path: &str,
    ts_paths: &HashMap<String, String>,
  ) -> String {
    // Try to find a matching TypeScript path mapping
    for (alias, resolved_path) in ts_paths {
      if ui_path.starts_with(alias) {
        // Replace the alias with the resolved path
        let remaining_path = ui_path.strip_prefix(alias).unwrap_or("");
        let remaining_path = remaining_path.trim_start_matches('/');

        if remaining_path.is_empty() {
          return resolved_path.clone();
        } else {
          return format!("{}/{}", resolved_path, remaining_path);
        }
      }
    }

    // If no TypeScript mapping found, fall back to manual resolution
    self.resolve_path_manually(ui_path)
  }

  /// Resolve path manually (fallback method)
  fn resolve_path_manually(&self, ui_path: &str) -> String {
    // Replace $lib placeholder if present in ui_path
    if ui_path.contains("$lib") {
      if let Some(lib_path) = &self.config.aliases.lib {
        return ui_path.replace("$lib", lib_path);
      } else {
        return ui_path.replace("$lib", "src/lib");
      }
    }

    // When there's no tsconfig.json, use the aliases exactly as configured
    // Don't override or modify the paths - respect the user's configuration
    ui_path.to_string()
  }

  /// Remove a component
  pub fn remove_component(&self, component_name: &str) -> Result<()> {
    println!(
      "{} Removing component '{}'...",
      "→".red(),
      component_name.cyan()
    );

    // This is a simplified implementation
    // In a real implementation, you'd need to track installed components
    // and their files to remove them properly

    println!(
      "{} Component removal not fully implemented yet",
      "!".yellow()
    );
    println!("  You'll need to manually remove the component files");

    Ok(())
  }

  /// Search components across registries
  pub async fn search_components(
    &self,
    query: &str,
    registry_namespace: Option<&str>,
  ) -> Result<()> {
    if let Some(namespace) = registry_namespace {
      // Search in specific registry
      if let Some(registry) = self.registry_manager.get_registry(namespace) {
        let results = registry.search_components(query).await?;
        self.print_search_results_async(namespace, &results).await;
      } else {
        return Err(anyhow!("Registry '{}' not found", namespace));
      }
    } else {
      // Search in all registries
      let results = self.registry_manager.search_all(query).await?;

      if results.is_empty() {
        println!(
          "{} No components found matching '{}'",
          "!".yellow(),
          query.cyan()
        );
        return Ok(());
      }

      for (namespace, components) in results {
        self
          .print_search_results_async(&namespace, &components)
          .await;
      }
    }

    Ok(())
  }

  /// Print search results (async version)
  async fn print_search_results_async(
    &self,
    namespace: &str,
    components: &[crate::registry::ComponentInfo],
  ) {
    if components.is_empty() {
      return;
    }

    // Get list of installed components for this instance
    let installed_components = self.get_installed_components().unwrap_or_default();

    println!("\n{} Registry: {}", "📦".blue(), namespace.cyan());

    for component in components {
      let is_installed = installed_components.contains(&component.name);

      let (status_icon, name_display, status_text) = if is_installed {
        // Check if component is outdated
        let is_outdated = self
          .is_component_outdated(&component.name, Some(namespace))
          .await
          .unwrap_or(false);

        if is_outdated {
          ("⚠".yellow(), component.name.yellow(), "Outdated".yellow())
        } else {
          ("✓".green(), component.name.green(), "Installed".green())
        }
      } else {
        (
          " ".normal(),
          component.name.cyan(),
          "Not Installed".dimmed(),
        )
      };

      println!("  {} {} {}", "→".blue(), status_icon, name_display);

      if let Some(comp_type) = &component.component_type {
        let type_display = match comp_type.as_str() {
          "registry:ui" => "UI Component".green(),
          "registry:block" => "Block".blue(),
          "registry:hook" => "Hook".yellow(),
          "registry:lib" => "Library".purple(),
          _ => comp_type.dimmed(),
        };
        println!("    Type: {}", type_display);
      }

      println!("    Status: {}", status_text);

      if let Some(deps) = &component.registry_dependencies {
        if !deps.is_empty() {
          println!("    Dependencies: {}", deps.join(", ").dimmed());
        }
      }
    }
  }

  /// Print search results (sync fallback version)
  #[allow(dead_code)]
  fn print_search_results(&self, namespace: &str, components: &[crate::registry::ComponentInfo]) {
    if components.is_empty() {
      return;
    }

    // Get list of installed components for this instance
    let installed_components = self.get_installed_components().unwrap_or_default();

    println!("\n{} Registry: {}", "📦".blue(), namespace.cyan());

    for component in components {
      let is_installed = installed_components.contains(&component.name);
      let status_icon = if is_installed {
        "✓".green()
      } else {
        " ".normal()
      };
      let name_display = if is_installed {
        component.name.green()
      } else {
        component.name.cyan()
      };

      println!("  {} {} {}", "→".blue(), status_icon, name_display);

      if let Some(comp_type) = &component.component_type {
        let type_display = match comp_type.as_str() {
          "registry:ui" => "UI Component".green(),
          "registry:block" => "Block".blue(),
          "registry:hook" => "Hook".yellow(),
          "registry:lib" => "Library".purple(),
          _ => comp_type.dimmed(),
        };
        println!("    Type: {}", type_display);
      }

      if is_installed {
        println!("    Status: {}", "Installed".green());
      }

      if let Some(deps) = &component.registry_dependencies {
        if !deps.is_empty() {
          println!("    Dependencies: {}", deps.join(", ").dimmed());
        }
      }
    }
  }

  /// List components from a registry
  pub async fn list_components(&self, registry_namespace: Option<&str>) -> Result<()> {
    if let Some(namespace) = registry_namespace {
      // List from specific registry
      if let Some(registry) = self.registry_manager.get_registry(namespace) {
        let index = registry.fetch_index().await?;
        let components: Vec<_> = index.as_slice().into_iter().cloned().collect();
        self
          .print_component_list_async(namespace, &components)
          .await;
      } else {
        return Err(anyhow!("Registry '{}' not found", namespace));
      }
    } else {
      // List from all registries
      for namespace in self.registry_manager.namespaces() {
        if let Some(registry) = self.registry_manager.get_registry(namespace) {
          match registry.fetch_index().await {
            Ok(index) => {
              let components: Vec<_> = index.as_slice().into_iter().cloned().collect();
              self
                .print_component_list_async(namespace, &components)
                .await;
            }
            Err(e) => {
              eprintln!(
                "Warning: Failed to fetch components from '{}': {}",
                namespace, e
              );
            }
          }
        }
      }
    }

    Ok(())
  }

  /// Print component list (async version)
  async fn print_component_list_async(
    &self,
    namespace: &str,
    components: &[crate::registry::ComponentInfo],
  ) {
    if components.is_empty() {
      return;
    }

    // Get list of installed components for this instance
    let installed_components = self.get_installed_components().unwrap_or_default();

    println!(
      "\n{} Registry: {} ({} components)",
      "📦".blue(),
      namespace.cyan(),
      components.len().to_string().yellow()
    );

    // Group by type
    let mut by_type: std::collections::HashMap<String, Vec<&crate::registry::ComponentInfo>> =
      std::collections::HashMap::new();

    for component in components {
      let comp_type = component
        .component_type
        .as_deref()
        .unwrap_or("other")
        .to_string();
      by_type.entry(comp_type).or_default().push(component);
    }

    // Display by type
    for (comp_type, comps) in by_type {
      let type_display = match comp_type.as_str() {
        "registry:ui" => "UI Components".green(),
        "registry:block" => "Blocks".blue(),
        "registry:hook" => "Hooks".yellow(),
        "registry:lib" => "Libraries".purple(),
        "registry:style" => "Styles".cyan(),
        _ => "Other".dimmed(),
      };

      println!("  {}", type_display);

      for component in comps {
        let is_installed = installed_components.contains(&component.name);

        let (status_icon, name_display) = if is_installed {
          // Check if component is outdated
          let is_outdated = self
            .is_component_outdated(&component.name, Some(namespace))
            .await
            .unwrap_or(false);

          if is_outdated {
            ("⚠".yellow(), component.name.yellow())
          } else {
            ("✓".green(), component.name.green())
          }
        } else {
          (" ".normal(), component.name.normal())
        };

        println!("    {} {} {}", "→".dimmed(), status_icon, name_display);
      }
    }
  }

  /// Print component list (sync fallback version without outdated check)
  #[allow(dead_code)]
  fn print_component_list(&self, namespace: &str, components: &[crate::registry::ComponentInfo]) {
    if components.is_empty() {
      return;
    }

    // Get list of installed components for this instance
    let installed_components = self.get_installed_components().unwrap_or_default();

    println!(
      "\n{} Registry: {} ({} components)",
      "📦".blue(),
      namespace.cyan(),
      components.len().to_string().yellow()
    );

    // Group by type
    let mut by_type: std::collections::HashMap<String, Vec<&crate::registry::ComponentInfo>> =
      std::collections::HashMap::new();

    for component in components {
      let comp_type = component
        .component_type
        .as_deref()
        .unwrap_or("other")
        .to_string();
      by_type.entry(comp_type).or_default().push(component);
    }

    // Display by type
    for (comp_type, comps) in by_type {
      let type_display = match comp_type.as_str() {
        "registry:ui" => "UI Components".green(),
        "registry:block" => "Blocks".blue(),
        "registry:hook" => "Hooks".yellow(),
        "registry:lib" => "Libraries".purple(),
        "registry:style" => "Styles".cyan(),
        _ => "Other".dimmed(),
      };

      println!("  {}", type_display);

      for component in comps {
        let is_installed = installed_components.contains(&component.name);
        let status_icon = if is_installed {
          "✓".green()
        } else {
          " ".normal()
        };
        let name_display = if is_installed {
          component.name.green()
        } else {
          component.name.normal()
        };

        println!("    {} {} {}", "→".dimmed(), status_icon, name_display);
      }
    }
  }

  /// Show component information
  pub async fn show_component_info(
    &self,
    component_name: &str,
    registry_namespace: Option<&str>,
  ) -> Result<()> {
    let component = if let Some(namespace) = registry_namespace {
      self
        .registry_manager
        .fetch_component(namespace, component_name)
        .await?
    } else {
      self
        .registry_manager
        .fetch_component_auto(component_name)
        .await?
    };

    println!("\n{} Component: {}", "📦".blue(), component.name.cyan());

    if let Some(comp_type) = &component.component_type {
      println!("Type: {}", comp_type.yellow());
    }

    if let Some(registry) = &component.registry {
      println!("Registry: {}", registry.yellow());
    }

    if let Some(dependencies) = &component.registry_dependencies {
      if !dependencies.is_empty() {
        println!("Registry Dependencies:");
        for dep in dependencies {
          println!("  - {}", dep.cyan());
        }
      }
    }

    if let Some(dependencies) = &component.dev_dependencies {
      if !dependencies.is_empty() {
        println!("Dev Dependencies:");
        for dep in dependencies {
          println!("  - {}", dep.cyan());
        }
      }
    }

    // Show registry dependencies from component info if available
    // (This would need to be fetched from the index, but for now we'll use
    // component.dependencies)

    println!("Files:");
    for file in &component.files {
      println!("  - {}", file.get_target_path().cyan());
    }

    Ok(())
  }

  /// Check if a component is installed locally
  pub fn is_component_installed(&self, component_name: &str) -> bool {
    // Get the UI directory path where components are installed
    let ui_path = self
      .config
      .aliases
      .ui
      .as_ref()
      .unwrap_or(&self.config.aliases.components);

    // Use the same resolution logic as resolve_file_path
    let resolved_ui_path = if let Some(ref ts_paths) = self.typescript_paths {
      self.resolve_path_with_typescript(ui_path, &ts_paths.paths)
    } else {
      self.resolve_path_manually(ui_path)
    };

    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let components_dir = current_dir.join(&resolved_ui_path);

    // Check if component directory exists (for @svelte registry style)
    let component_dir_path = components_dir.join(component_name);
    if component_dir_path.exists() && component_dir_path.is_dir() {
      return true;
    }

    // Check if component file exists (for @default registry style)
    // Try common file extensions
    let extensions = ["tsx", "ts", "jsx", "js", "svelte", "vue"];
    for ext in &extensions {
      let component_file_path = components_dir.join(format!("{}.{}", component_name, ext));
      if component_file_path.exists() && component_file_path.is_file() {
        return true;
      }
    }

    false
  }

  /// Get list of locally installed components
  pub fn get_installed_components(&self) -> Result<Vec<String>> {
    let ui_path = self
      .config
      .aliases
      .ui
      .as_ref()
      .unwrap_or(&self.config.aliases.components);

    // Use the same resolution logic as resolve_file_path
    let resolved_ui_path = if let Some(ref ts_paths) = self.typescript_paths {
      self.resolve_path_with_typescript(ui_path, &ts_paths.paths)
    } else {
      self.resolve_path_manually(ui_path)
    };

    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let components_dir = current_dir.join(&resolved_ui_path);

    let mut installed = Vec::new();

    if components_dir.exists() {
      for entry in fs::read_dir(&components_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
          // Handle directory-based components (like @svelte registry)
          if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // Skip hidden directories and common non-component directories
            if !name.starts_with('.') && name != "index.ts" && name != "index.js" {
              installed.push(name.to_string());
            }
          }
        } else if path.is_file() {
          // Handle file-based components (like @default registry)
          if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            // Skip hidden files and common non-component files
            if !file_name.starts_with('.')
              && !file_name.ends_with(".d.ts")
              && !file_name.ends_with(".map")
              && file_name != "index.ts"
              && file_name != "index.js"
            {
              // Extract component name from file name (remove extension)
              if let Some(component_name) = file_name.split('.').next() {
                if !component_name.is_empty() {
                  installed.push(component_name.to_string());
                }
              }
            }
          }
        }
      }
    }

    installed.sort();
    installed.dedup(); // Remove duplicates in case both file and directory exist
    Ok(installed)
  }

  /// Check if an installed component is outdated compared to registry version
  pub async fn is_component_outdated(
    &self,
    component_name: &str,
    registry_namespace: Option<&str>,
  ) -> Result<bool> {
    // First check if component is installed
    if !self.is_component_installed(component_name) {
      return Ok(false); // Not installed, so not outdated
    }

    // Fetch the latest version from registry
    let registry_component = if let Some(namespace) = registry_namespace {
      match self
        .registry_manager
        .fetch_component(namespace, component_name)
        .await
      {
        Ok(comp) => comp,
        Err(_) => return Ok(false), // Can't fetch, assume not outdated
      }
    } else {
      match self
        .registry_manager
        .fetch_component_auto(component_name)
        .await
      {
        Ok(comp) => comp,
        Err(_) => return Ok(false), // Can't fetch, assume not outdated
      }
    };

    // Create component context for proper path resolution
    let component_context = self.create_component_context(&registry_component);

    // Compare local files with registry files
    for registry_file in &registry_component.files {
      let local_path =
        self.resolve_file_path(&registry_file.get_target_path(), &component_context)?;

      if !local_path.exists() {
        return Ok(true); // File missing locally, component is outdated
      }

      let local_content = match fs::read_to_string(&local_path) {
        Ok(content) => content,
        Err(_) => return Ok(true), // Can't read local file, assume outdated
      };

      // Normalize whitespace and line endings for comparison
      let local_normalized = self.normalize_content(&local_content);
      let registry_normalized = self.normalize_content(&registry_file.content);

      if local_normalized != registry_normalized {
        return Ok(true); // Content differs, component is outdated
      }
    }

    Ok(false) // All files match, component is up to date
  }

  /// Normalize content for comparison (removes whitespace differences and
  /// processes placeholders)
  fn normalize_content(&self, content: &str) -> String {
    // First process placeholders to ensure both local and registry content are
    // comparable
    let processed_content = self
      .process_placeholders(content, None)
      .unwrap_or_else(|_| content.to_string());

    // Then normalize whitespace
    processed_content
      .lines()
      .map(|line| line.trim())
      .filter(|line| !line.is_empty())
      .collect::<Vec<_>>()
      .join("\n")
  }

  /// Get hash of local component files for comparison
  #[allow(dead_code)]
  fn get_component_hash(&self, component_name: &str) -> Result<String> {
    let ui_path = self
      .config
      .aliases
      .ui
      .as_ref()
      .unwrap_or(&self.config.aliases.components);

    // Use the same resolution logic as resolve_file_path
    let resolved_ui_path = if let Some(ref ts_paths) = self.typescript_paths {
      self.resolve_path_with_typescript(ui_path, &ts_paths.paths)
    } else {
      self.resolve_path_manually(ui_path)
    };

    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let component_dir = current_dir.join(&resolved_ui_path).join(component_name);

    if !component_dir.exists() {
      return Err(anyhow!("Component '{}' not found", component_name));
    }

    let mut hasher = Sha256::new();
    let mut file_contents = Vec::new();

    // Collect all files in component directory
    self.collect_component_files(&component_dir, &mut file_contents)?;

    // Sort files by path for consistent hashing
    file_contents.sort_by(|a, b| a.0.cmp(&b.0));

    // Hash all file contents
    for (path, content) in file_contents {
      hasher.update(path.as_bytes());
      hasher.update(self.normalize_content(&content).as_bytes());
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
  }

  /// Recursively collect all files in a component directory
  #[allow(dead_code)]
  fn collect_component_files(
    &self,
    dir: &PathBuf,
    files: &mut Vec<(String, String)>,
  ) -> Result<()> {
    for entry in fs::read_dir(dir)? {
      let entry = entry?;
      let path = entry.path();

      if path.is_file() {
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
          // Skip hidden files and common non-component files
          if !file_name.starts_with('.')
            && !file_name.ends_with(".d.ts")
            && !file_name.ends_with(".map")
          {
            let content = fs::read_to_string(&path)?;
            let relative_path = path
              .strip_prefix(dir)
              .unwrap_or(&path)
              .to_string_lossy()
              .to_string();

            files.push((relative_path, content));
          }
        }
      } else if path.is_dir() {
        // Recursively process subdirectories
        self.collect_component_files(&path, files)?;
      }
    }

    Ok(())
  }

  /// Check multiple components for outdated status
  pub async fn check_outdated_components(
    &self,
    component_names: &[String],
    registry_namespace: Option<&str>,
  ) -> Result<Vec<(String, bool)>> {
    let mut results = Vec::new();

    for component_name in component_names {
      let is_outdated = self
        .is_component_outdated(component_name, registry_namespace)
        .await?;
      results.push((component_name.clone(), is_outdated));
    }

    Ok(results)
  }

  /// Process placeholders in file content based on configuration
  fn process_placeholders(
    &self,
    content: &str,
    context: Option<&ComponentContext>,
  ) -> Result<String> {
    let mut processed_content = content.to_string();

    // Replace $UTILS$ placeholder
    if let Some(utils_path) = self.get_utils_import_path() {
      processed_content = processed_content.replace("$UTILS$", &utils_path);
    }

    // Replace $COMPONENTS$ placeholder with context-aware resolution
    if let Some(components_path) = self.get_components_import_path_with_context(context) {
      processed_content = processed_content.replace("$COMPONENTS$", &components_path);
    }

    // Replace $HOOKS$ placeholder with context-aware resolution
    if let Some(hooks_path) = self.get_hooks_import_path_with_context(context) {
      processed_content = processed_content.replace("$HOOKS$", &hooks_path);
    }

    // Replace $LIB$ placeholder with context-aware resolution
    if let Some(lib_path) = self.get_lib_import_path_with_context(context) {
      processed_content = processed_content.replace("$LIB$", &lib_path);
    }

    // Post-process imports: remove .js extensions when TypeScript is enabled
    if self.is_typescript_enabled() {
      processed_content = self.remove_js_extensions_from_imports(&processed_content);
    }

    Ok(processed_content)
  }

  /// Check if TypeScript is enabled in the configuration
  fn is_typescript_enabled(&self) -> bool {
    match &self.config.typescript {
      Some(crate::config::TypeScriptConfig::Boolean(true)) => true,
      Some(crate::config::TypeScriptConfig::Object { .. }) => true,
      _ => false,
    }
  }

  /// Remove .js extensions from import statements when TypeScript is enabled
  fn remove_js_extensions_from_imports(&self, content: &str) -> String {
    use regex::Regex;

    // Pattern 1: Standard import statements with .js extensions
    // Matches: import ... from "path.js" or import ... from 'path.js'
    let import_regex = Regex::new(r#"(import\s+[^"']*["'])([^"']+)\.js(["'])"#).unwrap();
    let mut processed = import_regex.replace_all(content, "$1$2$3").to_string();

    // Pattern 2: Export statements with .js extensions
    // Matches: export ... from "path.js" or export ... from 'path.js'
    let export_regex = Regex::new(r#"(export\s+[^"']*["'])([^"']+)\.js(["'])"#).unwrap();
    processed = export_regex.replace_all(&processed, "$1$2$3").to_string();

    // Pattern 3: Dynamic imports with .js extensions
    // Matches: import("path.js") or import('path.js')
    let dynamic_import_regex =
      Regex::new(r#"(import\s*\(\s*["'])([^"']+)\.js(["']\s*\))"#).unwrap();
    processed = dynamic_import_regex
      .replace_all(&processed, "$1$2$3")
      .to_string();

    // Pattern 4: Placeholder-specific case like $UTILS$.js
    // This handles cases where placeholders are followed by .js
    let placeholder_regex = Regex::new(r"\$([A-Z_]+)\$\.js\b").unwrap();
    processed = placeholder_regex
      .replace_all(&processed, "$$1$")
      .to_string();

    processed
  }

  /// Get the utils import path based on configuration
  fn get_utils_import_path(&self) -> Option<String> {
    let utils_path = &self.config.aliases.utils;

    // First try to resolve using TypeScript paths if available
    if let Some(ref ts_paths) = self.typescript_paths {
      let resolved = self.resolve_import_path_with_typescript(utils_path, &ts_paths.paths);
      if !resolved.is_empty() {
        return Some(resolved);
      }
    }

    // Fallback to manual resolution
    self.resolve_import_path_manually(utils_path)
  }

  /// Get the components import path based on configuration
  fn get_components_import_path(&self) -> Option<String> {
    let components_path = &self.config.aliases.components;

    // First try to resolve using TypeScript paths if available
    if let Some(ref ts_paths) = self.typescript_paths {
      let resolved = self.resolve_import_path_with_typescript(components_path, &ts_paths.paths);
      if !resolved.is_empty() {
        return Some(resolved);
      }
    }

    // Fallback to manual resolution
    self.resolve_import_path_manually(components_path)
  }

  /// Get the components import path with context awareness
  fn get_components_import_path_with_context(
    &self,
    context: Option<&ComponentContext>,
  ) -> Option<String> {
    let components_path = if let Some(ctx) = context {
      // Use the alias based on component type
      self.get_alias_for_component_type(ctx.component_type.as_deref())
    } else {
      &self.config.aliases.components
    };

    // First try to resolve using TypeScript paths if available
    if let Some(ref ts_paths) = self.typescript_paths {
      let resolved = self.resolve_import_path_with_typescript(components_path, &ts_paths.paths);
      if !resolved.is_empty() {
        return Some(resolved);
      }
    }

    // Fallback to manual resolution
    self.resolve_import_path_manually(components_path)
  }

  /// Get the hooks import path based on configuration
  fn get_hooks_import_path(&self) -> Option<String> {
    if let Some(hooks_path) = &self.config.aliases.hooks {
      // First try to resolve using TypeScript paths if available
      if let Some(ref ts_paths) = self.typescript_paths {
        let resolved = self.resolve_import_path_with_typescript(hooks_path, &ts_paths.paths);
        if !resolved.is_empty() {
          return Some(resolved);
        }
      }

      // Fallback to manual resolution
      self.resolve_import_path_manually(hooks_path)
    } else {
      None
    }
  }

  /// Get the hooks import path with context awareness
  fn get_hooks_import_path_with_context(
    &self,
    context: Option<&ComponentContext>,
  ) -> Option<String> {
    let hooks_path = if let Some(ctx) = context {
      // For hooks components, use hooks alias, otherwise use the component type alias
      if ctx.component_type.as_deref() == Some("registry:hook") {
        self
          .config
          .aliases
          .hooks
          .as_deref()
          .unwrap_or(&self.config.aliases.components)
      } else {
        self.get_alias_for_component_type(ctx.component_type.as_deref())
      }
    } else {
      self
        .config
        .aliases
        .hooks
        .as_deref()
        .unwrap_or(&self.config.aliases.components)
    };

    // First try to resolve using TypeScript paths if available
    if let Some(ref ts_paths) = self.typescript_paths {
      let resolved = self.resolve_import_path_with_typescript(hooks_path, &ts_paths.paths);
      if !resolved.is_empty() {
        return Some(resolved);
      }
    }

    // Fallback to manual resolution
    self.resolve_import_path_manually(hooks_path)
  }

  /// Get the lib import path based on configuration
  fn get_lib_import_path(&self) -> Option<String> {
    if let Some(lib_path) = &self.config.aliases.lib {
      // First try to resolve using TypeScript paths if available
      if let Some(ref ts_paths) = self.typescript_paths {
        let resolved = self.resolve_import_path_with_typescript(lib_path, &ts_paths.paths);
        if !resolved.is_empty() {
          return Some(resolved);
        }
      }

      // For lib, usually just return the original alias since it's the base
      Some(lib_path.clone())
    } else {
      None
    }
  }

  /// Get the lib import path with context awareness
  fn get_lib_import_path_with_context(&self, context: Option<&ComponentContext>) -> Option<String> {
    let lib_path = if let Some(ctx) = context {
      // For lib components, use lib alias, otherwise use the component type alias
      if ctx.component_type.as_deref() == Some("registry:lib") {
        self
          .config
          .aliases
          .lib
          .as_deref()
          .unwrap_or(&self.config.aliases.components)
      } else {
        self.get_alias_for_component_type(ctx.component_type.as_deref())
      }
    } else {
      self
        .config
        .aliases
        .lib
        .as_deref()
        .unwrap_or(&self.config.aliases.components)
    };

    // First try to resolve using TypeScript paths if available
    if let Some(ref ts_paths) = self.typescript_paths {
      let resolved = self.resolve_import_path_with_typescript(lib_path, &ts_paths.paths);
      if !resolved.is_empty() {
        return Some(resolved);
      }
    }

    // For lib, usually just return the original alias since it's the base
    Some(lib_path.to_string())
  }

  /// Install dependencies using the detected package manager
  fn install_dependencies(&self, deps: &ComponentDependencies) -> Result<()> {
    let Some(detection) = &self.package_manager else {
      println!(
        "{} Skipping dependency installation - no package manager detected",
        "!".yellow()
      );
      return Ok(());
    };

    let total_deps = deps.dependencies.len() + deps.dev_dependencies.len();
    if total_deps == 0 {
      return Ok(());
    }

    println!(
      "{} Installing {} dependencies with {}",
      "📦".blue(),
      total_deps.to_string().cyan(),
      detection.manager.name().cyan()
    );

    // Install regular dependencies first
    if !deps.dependencies.is_empty() {
      self.install_dependency_type(&detection, &deps.dependencies, false)?;
    }

    // Install dev dependencies
    if !deps.dev_dependencies.is_empty() {
      self.install_dependency_type(&detection, &deps.dev_dependencies, true)?;
    }

    Ok(())
  }

  /// Install a specific type of dependencies (regular or dev)
  fn install_dependency_type(
    &self,
    detection: &Detection,
    dependencies: &[String],
    is_dev: bool,
  ) -> Result<()> {
    if dependencies.is_empty() {
      return Ok(());
    }

    let dep_type = if is_dev {
      "dev dependencies"
    } else {
      "dependencies"
    };
    println!(
      "{} Installing {} {} with {}",
      "→".blue(),
      dependencies.len().to_string().cyan(),
      dep_type.cyan(),
      detection.manager.name().cyan()
    );

    // Build the command
    let mut cmd = if is_dev {
      detection.manager.install_dev_command()
    } else {
      detection.manager.install_command()
    };
    cmd.extend(dependencies.iter().cloned());

    println!("{} Running: {}", "→".blue(), cmd.join(" ").cyan());

    // Try to execute the command, with fallbacks for different package managers
    let status = self.execute_package_manager_command(&cmd, &detection.project_root)?;

    if status.success() {
      println!("{} {} installed successfully", "✓".green(), dep_type);
    } else {
      println!("{} Failed to install {}", "✗".red(), dep_type);
      return Err(anyhow!("Package manager command failed for {}", dep_type));
    }

    Ok(())
  }

  /// Detect the best execution strategy for the package manager
  fn detect_execution_strategy(
    &self,
    cmd: &[String],
    project_root: &std::path::Path,
  ) -> Option<String> {
    // Test direct execution first
    if std::process::Command::new(&cmd[0])
      .arg("--version")
      .current_dir(project_root)
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .status()
      .map(|s| s.success())
      .unwrap_or(false)
    {
      return Some("direct".to_string());
    }

    // Test npx for pnpm
    if cmd[0] == "pnpm"
      && std::process::Command::new("npx")
        .args(&[&cmd[0], "--version"])
        .current_dir(project_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
      return Some("npx".to_string());
    }

    // Test npm exec for pnpm/yarn
    if (cmd[0] == "pnpm" || cmd[0] == "yarn")
      && std::process::Command::new("npm")
        .args(&["exec", &cmd[0], "--", "--version"])
        .current_dir(project_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
      return Some("npm_exec".to_string());
    }

    // Test local binary
    let local_cmd_path = project_root.join("node_modules").join(".bin").join(&cmd[0]);
    if local_cmd_path.exists()
      && std::process::Command::new(&local_cmd_path)
        .arg("--version")
        .current_dir(project_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
      return Some("local_bin".to_string());
    }

    // Test corepack
    if std::process::Command::new("corepack")
      .args(&[&cmd[0], "--version"])
      .current_dir(project_root)
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .status()
      .map(|s| s.success())
      .unwrap_or(false)
    {
      return Some("corepack".to_string());
    }

    // Test cmd.exe on Windows
    #[cfg(windows)]
    if std::process::Command::new("cmd")
      .args(&["/C", &cmd[0], "--version"])
      .current_dir(project_root)
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .status()
      .map(|s| s.success())
      .unwrap_or(false)
    {
      return Some("cmd".to_string());
    }

    // Test PowerShell on Windows
    #[cfg(windows)]
    {
      let ps_command = format!("& {} --version", cmd[0]);
      if std::process::Command::new("powershell")
        .args(&["-Command", &ps_command])
        .current_dir(project_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
      {
        return Some("powershell".to_string());
      }
    }

    None
  }

  /// Execute package manager command using the detected strategy
  fn execute_package_manager_command(
    &self,
    cmd: &[String],
    project_root: &std::path::Path,
  ) -> Result<std::process::ExitStatus> {
    // Detect the best strategy first
    let strategy = self.detect_execution_strategy(cmd, project_root);

    match strategy.as_deref() {
      Some("direct") => {
        println!("{} Running: {}", "→".blue(), cmd.join(" ").cyan());
        std::process::Command::new(&cmd[0])
          .args(&cmd[1..])
          .current_dir(project_root)
          .status()
          .map_err(Into::into)
      }
      Some("npx") => {
        println!(
          "{} Running via npx: npx {}",
          "→".blue(),
          cmd.join(" ").cyan()
        );
        let npx_cmd = ["npx".to_string()]
          .into_iter()
          .chain(cmd.iter().cloned())
          .collect::<Vec<_>>();
        std::process::Command::new(&npx_cmd[0])
          .args(&npx_cmd[1..])
          .current_dir(project_root)
          .status()
          .map_err(Into::into)
      }
      Some("npm_exec") => {
        println!(
          "{} Running via npm exec: npm exec {} -- {}",
          "→".blue(),
          cmd[0],
          cmd[1..].join(" ").cyan()
        );
        let npm_exec_cmd = vec![
          "npm".to_string(),
          "exec".to_string(),
          cmd[0].clone(),
          "--".to_string(),
        ]
        .into_iter()
        .chain(cmd[1..].iter().cloned())
        .collect::<Vec<_>>();
        std::process::Command::new(&npm_exec_cmd[0])
          .args(&npm_exec_cmd[1..])
          .current_dir(project_root)
          .status()
          .map_err(Into::into)
      }
      Some("local_bin") => {
        let local_cmd_path = project_root.join("node_modules").join(".bin").join(&cmd[0]);
        println!(
          "{} Running local binary: {}",
          "→".blue(),
          local_cmd_path.display().to_string().cyan()
        );
        std::process::Command::new(&local_cmd_path)
          .args(&cmd[1..])
          .current_dir(project_root)
          .status()
          .map_err(Into::into)
      }
      Some("corepack") => {
        println!(
          "{} Running via corepack: corepack {} {}",
          "→".blue(),
          cmd[0],
          cmd[1..].join(" ").cyan()
        );
        let corepack_cmd = vec!["corepack".to_string(), cmd[0].clone()]
          .into_iter()
          .chain(cmd[1..].iter().cloned())
          .collect::<Vec<_>>();
        std::process::Command::new(&corepack_cmd[0])
          .args(&corepack_cmd[1..])
          .current_dir(project_root)
          .status()
          .map_err(Into::into)
      }
      #[cfg(windows)]
      Some("cmd") => {
        println!(
          "{} Running via cmd: cmd /C {} {}",
          "→".blue(),
          cmd[0],
          cmd[1..].join(" ").cyan()
        );
        let cmd_args = vec!["/C".to_string(), cmd[0].clone()]
          .into_iter()
          .chain(cmd[1..].iter().cloned())
          .collect::<Vec<_>>();
        std::process::Command::new("cmd")
          .args(&cmd_args)
          .current_dir(project_root)
          .status()
          .map_err(Into::into)
      }
      #[cfg(windows)]
      Some("powershell") => {
        println!(
          "{} Running via PowerShell: powershell -Command \"{}\"",
          "→".blue(),
          cmd.join(" ").cyan()
        );
        let ps_command = format!("& {} {}", cmd[0], cmd[1..].join(" "));
        std::process::Command::new("powershell")
          .args(&["-Command", &ps_command])
          .current_dir(project_root)
          .status()
          .map_err(Into::into)
      }
      _ => {
        // Fallback: try all strategies with detailed output
        self.execute_with_fallback_strategies(cmd, project_root)
      }
    }
  }

  /// Fallback method with all strategies (used when detection fails)
  fn execute_with_fallback_strategies(
    &self,
    cmd: &[String],
    project_root: &std::path::Path,
  ) -> Result<std::process::ExitStatus> {
    println!(
      "{} No working strategy detected, trying all fallbacks...",
      "⚠".yellow()
    );

    // First try: execute command directly
    println!("{} Direct execution attempt", "→".blue());
    match std::process::Command::new(&cmd[0])
      .args(&cmd[1..])
      .current_dir(project_root)
      .status()
    {
      Ok(status) if status.success() => {
        println!("{} Direct execution successful", "✓".green());
        return Ok(status);
      }
      Ok(status) => {
        println!(
          "{} Direct execution failed with exit code: {}",
          "✗".red(),
          status.code().unwrap_or(-1)
        );
      }
      Err(e) => {
        println!("{} Direct execution error: {}", "✗".red(), e);
      }
    }

    // Helper function to check if a command is available (for fallback use)
    fn is_command_available(command: &str) -> bool {
      std::process::Command::new(command)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    }

    // Try remaining strategies in order
    // npx strategy
    if cmd[0] == "pnpm" && is_command_available("npx") {
      println!(
        "{} Trying with npx: npx {}",
        "→".blue(),
        cmd.join(" ").cyan()
      );
      let npx_cmd = ["npx".to_string()]
        .into_iter()
        .chain(cmd.iter().cloned())
        .collect::<Vec<_>>();
      if let Ok(status) = std::process::Command::new(&npx_cmd[0])
        .args(&npx_cmd[1..])
        .current_dir(project_root)
        .status()
      {
        if status.success() {
          println!("{} npx execution successful", "✓".green());
          return Ok(status);
        } else {
          println!(
            "{} npx execution failed with exit code: {}",
            "✗".red(),
            status.code().unwrap_or(-1)
          );
        }
      }
    }

    // npm exec strategy
    if (cmd[0] == "pnpm" || cmd[0] == "yarn") && is_command_available("npm") {
      println!(
        "{} Trying with npm exec: npm exec {} -- {}",
        "→".blue(),
        cmd[0],
        cmd[1..].join(" ").cyan()
      );
      let npm_exec_cmd = vec![
        "npm".to_string(),
        "exec".to_string(),
        cmd[0].clone(),
        "--".to_string(),
      ]
      .into_iter()
      .chain(cmd[1..].iter().cloned())
      .collect::<Vec<_>>();
      if let Ok(status) = std::process::Command::new(&npm_exec_cmd[0])
        .args(&npm_exec_cmd[1..])
        .current_dir(project_root)
        .status()
      {
        if status.success() {
          println!("{} npm exec execution successful", "✓".green());
          return Ok(status);
        } else {
          println!(
            "{} npm exec execution failed with exit code: {}",
            "✗".red(),
            status.code().unwrap_or(-1)
          );
        }
      }
    }

    // cmd.exe strategy (Windows)
    #[cfg(windows)]
    {
      println!(
        "{} Trying with cmd.exe: cmd /C {} {}",
        "→".blue(),
        cmd[0],
        cmd[1..].join(" ").cyan()
      );
      let cmd_args = vec!["/C".to_string(), cmd[0].clone()]
        .into_iter()
        .chain(cmd[1..].iter().cloned())
        .collect::<Vec<_>>();
      if let Ok(status) = std::process::Command::new("cmd")
        .args(&cmd_args)
        .current_dir(project_root)
        .status()
      {
        if status.success() {
          println!("{} cmd execution successful", "✓".green());
          return Ok(status);
        } else {
          println!(
            "{} cmd execution failed with exit code: {}",
            "✗".red(),
            status.code().unwrap_or(-1)
          );
        }
      }
    }

    // Final attempt
    println!("{} Final attempt with original command", "→".blue());
    std::process::Command::new(&cmd[0])
      .args(&cmd[1..])
      .current_dir(project_root)
      .status()
      .map_err(Into::into)
  }

  /// Resolve import path using TypeScript path mappings
  fn resolve_import_path_with_typescript(
    &self,
    import_path: &str,
    ts_paths: &HashMap<String, String>,
  ) -> String {
    // Try to find a matching TypeScript path mapping for imports
    for (alias, _) in ts_paths {
      if import_path.starts_with(alias) {
        // For imports, we want to keep the alias, not resolve to file system path
        return import_path.to_string();
      }
    }

    String::new() // Return empty string if not found
  }

  /// Resolve import path manually (fallback method for imports)
  fn resolve_import_path_manually(&self, import_path: &str) -> Option<String> {
    if import_path.starts_with("$lib") {
      if let Some(lib_path) = &self.config.aliases.lib {
        Some(import_path.replace("$lib", lib_path))
      } else {
        Some(import_path.to_string()) // Keep $lib as is
      }
    } else {
      Some(import_path.to_string())
    }
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use super::*;
  use crate::config::{AliasesConfig, TailwindConfig};

  fn create_test_config() -> Config {
    Config {
      schema: None,
      style: None,
      tailwind: TailwindConfig {
        css: "src/app.css".to_string(),
        base_color: "slate".to_string(),
        config: None,
      },
      aliases: AliasesConfig {
        components: "src/lib/components".to_string(),
        utils: "src/lib/utils".to_string(),
        ui: Some("src/lib/components/ui".to_string()),
        hooks: None,
        lib: Some("src/lib".to_string()),
      },
      registries: HashMap::new(),
      typescript: None,
    }
  }

  #[test]
  fn test_resolve_file_path() {
    let config = create_test_config();
    let installer = ComponentInstaller::new(config).unwrap();

    // Create a test component context for UI components
    let context = ComponentContext {
      name: "button".to_string(),
      component_type: Some("registry:ui".to_string()),
      registry: Some("test".to_string()),
    };

    // Test with component target path format (like "button/button.svelte")
    let path = installer
      .resolve_file_path("button/button.svelte", &context)
      .unwrap();
    assert!(path
      .to_string_lossy()
      .contains("src/lib/components/ui/button/button.svelte"));

    // Test with another component target
    let path = installer
      .resolve_file_path("card/index.ts", &context)
      .unwrap();
    assert!(path
      .to_string_lossy()
      .contains("src/lib/components/ui/card/index.ts"));
  }

  #[test]
  fn test_get_alias_for_component_type() {
    let config = create_test_config();
    let installer = ComponentInstaller::new(config).unwrap();

    // Test registry:ui uses ui alias
    assert_eq!(
      installer.get_alias_for_component_type(Some("registry:ui")),
      "src/lib/components/ui"
    );

    // Test registry:util uses utils alias
    assert_eq!(
      installer.get_alias_for_component_type(Some("registry:util")),
      "src/lib/utils"
    );

    // Test registry:hook uses components alias (since hooks is None in test config)
    assert_eq!(
      installer.get_alias_for_component_type(Some("registry:hook")),
      "src/lib/components"
    );

    // Test registry:lib uses lib alias
    assert_eq!(
      installer.get_alias_for_component_type(Some("registry:lib")),
      "src/lib"
    );

    // Test unknown type uses components alias as fallback
    assert_eq!(
      installer.get_alias_for_component_type(Some("registry:unknown")),
      "src/lib/components"
    );

    // Test None uses components alias as fallback
    assert_eq!(
      installer.get_alias_for_component_type(None),
      "src/lib/components"
    );
  }

  #[test]
  fn test_component_context_creation() {
    let config = create_test_config();
    let installer = ComponentInstaller::new(config).unwrap();

    let component = crate::registry::Component {
      schema: None,
      name: "test-button".to_string(),
      component_type: Some("registry:ui".to_string()),
      dependencies: None,
      dev_dependencies: None,
      registry_dependencies: None,
      files: vec![],
      registry: Some("test-registry".to_string()),
    };

    let context = installer.create_component_context(&component);

    assert_eq!(context.name, "test-button");
    assert_eq!(context.component_type, Some("registry:ui".to_string()));
    assert_eq!(context.registry, Some("test-registry".to_string()));
  }
}
