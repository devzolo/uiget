use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::registry::{Component, ComponentInfo, RegistryIndex};

/// Registry configuration for building components
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RegistryConfig {
  #[serde(rename = "$schema")]
  pub schema: Option<String>,
  /// The name of the registry
  pub name: String,
  /// Registry description
  pub description: Option<String>,
  /// Registry homepage URL
  pub homepage: Option<String>,
  /// Registry documentation URL
  pub docs: Option<String>,
  /// Registry author information
  pub author: Option<RegistryAuthor>,
  /// Available styles for this registry
  pub styles: Option<Vec<String>>,
  /// Default style
  pub default_style: Option<String>,
  /// Component definitions
  pub components: HashMap<String, ComponentDefinition>,
}

/// Registry author information
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RegistryAuthor {
  pub name: String,
  pub email: Option<String>,
  pub url: Option<String>,
}

/// Component definition in the registry configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ComponentDefinition {
  /// Component name
  pub name: String,
  /// Component type (registry:ui, registry:block, etc.)
  #[serde(rename = "type")]
  pub component_type: Option<String>,
  /// Component description
  pub description: Option<String>,
  /// Registry dependencies (other components this depends on)
  #[serde(rename = "registryDependencies")]
  pub registry_dependencies: Option<Vec<String>>,
  /// Development dependencies (npm packages)
  #[serde(rename = "devDependencies")]
  pub dev_dependencies: Option<Vec<String>>,
  /// Dependencies (npm packages)
  pub dependencies: Option<Vec<String>>,
  /// Peer dependencies (npm packages)
  #[serde(rename = "peerDependencies")]
  pub peer_dependencies: Option<Vec<String>>,
  /// File mappings for different styles
  pub files: Option<HashMap<String, Vec<ComponentFileSource>>>,
  /// Default files (used when no style is specified)
  pub default_files: Option<Vec<ComponentFileSource>>,
  /// Tags for categorization
  pub tags: Option<Vec<String>>,
  /// Whether the component is external (not built locally)
  pub external: Option<bool>,
}

/// Component file source definition
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ComponentFileSource {
  /// Source file path (relative to the registry config)
  pub source: String,
  /// Target path in the component output
  pub target: String,
  /// File type (optional)
  #[serde(rename = "type")]
  pub file_type: Option<String>,
}

/// Registry builder for generating shadcn-compatible JSON files
pub struct RegistryBuilder {
  config: RegistryConfig,
  base_path: PathBuf,
  output_path: PathBuf,
}

impl RegistryBuilder {
  /// Create a new registry builder
  pub fn new(config_path: &Path, output_path: &Path) -> Result<Self> {
    let base_path = config_path
      .parent()
      .unwrap_or_else(|| Path::new("."))
      .to_path_buf();

    let config_content = fs::read_to_string(config_path)
      .map_err(|e| anyhow!("Failed to read registry config: {}", e))?;

    let config: RegistryConfig = serde_json::from_str(&config_content)
      .map_err(|e| anyhow!("Failed to parse registry config: {}", e))?;

    Ok(Self {
      config,
      base_path,
      output_path: output_path.to_path_buf(),
    })
  }

  /// Build all registry JSON files
  pub fn build(&self) -> Result<()> {
    // Create output directory
    fs::create_dir_all(&self.output_path)
      .map_err(|e| anyhow!("Failed to create output directory: {}", e))?;

    // Generate index.json
    self.build_index()?;

    // Generate individual component files
    self.build_components()?;

    println!(
      "✓ Registry built successfully to {}",
      self.output_path.display()
    );

    Ok(())
  }

  /// Build the registry index
  fn build_index(&self) -> Result<()> {
    let mut components = Vec::new();

    for (name, definition) in &self.config.components {
      let component_info = ComponentInfo {
        name: name.clone(),
        component_type: definition.component_type.clone(),
        registry_dependencies: definition.registry_dependencies.clone(),
        dev_dependencies: definition.dev_dependencies.clone(),
        relative_url: None,
      };
      components.push(component_info);
    }

    let index = RegistryIndex::Object(
      components
        .into_iter()
        .map(|comp| (comp.name.clone(), comp))
        .collect(),
    );

    let index_path = self.output_path.join("index.json");
    let index_content = serde_json::to_string_pretty(&index)?;
    fs::write(&index_path, index_content)
      .map_err(|e| anyhow!("Failed to write index.json: {}", e))?;

    println!("✓ Generated index.json");

    Ok(())
  }

  /// Build individual component files
  fn build_components(&self) -> Result<()> {
    let default_styles = vec!["default".to_string()];
    let styles = self.config.styles.as_ref().unwrap_or(&default_styles);

    for (name, definition) in &self.config.components {
      // Skip external components
      if definition.external.unwrap_or(false) {
        continue;
      }

      for style in styles {
        self.build_component(name, definition, style)?;
      }
    }

    Ok(())
  }

  /// Build a single component for a specific style
  fn build_component(
    &self,
    name: &str,
    definition: &ComponentDefinition,
    style: &str,
  ) -> Result<()> {
    // Get files for this style
    let file_sources = if let Some(files) = &definition.files {
      files.get(style).or_else(|| files.get("default"))
    } else {
      None
    }
    .or(definition.default_files.as_ref())
    .ok_or_else(|| {
      anyhow!(
        "No files defined for component '{}' with style '{}'",
        name,
        style
      )
    })?;

    // Build component files
    let mut component_files = Vec::new();
    for file_source in file_sources {
      let source_path = self.base_path.join(&file_source.source);
      
      if !source_path.exists() {
        return Err(anyhow!(
          "Source file '{}' not found for component '{}'",
          file_source.source,
          name
        ));
      }

      let content = fs::read_to_string(&source_path)
        .map_err(|e| anyhow!("Failed to read source file '{}': {}", file_source.source, e))?;

      let component_file = crate::registry::ComponentFile {
        content,
        file_type: file_source.file_type.clone(),
        target: Some(file_source.target.clone()),
        path: None,
      };

      component_files.push(component_file);
    }

    // Create component
    let component = Component {
      schema: Some("https://ui.shadcn.com/schema.json".to_string()),
      name: name.to_string(),
      component_type: definition.component_type.clone(),
      dev_dependencies: definition.dev_dependencies.clone(),
      registry_dependencies: definition.registry_dependencies.clone(),
      files: component_files,
      registry: None,
    };

    // Write component file
    let component_dir = if style == "default" {
      self.output_path.clone()
    } else {
      self.output_path.join(style)
    };

    fs::create_dir_all(&component_dir)
      .map_err(|e| anyhow!("Failed to create component directory: {}", e))?;

    let component_path = component_dir.join(format!("{}.json", name));
    let component_content = serde_json::to_string_pretty(&component)?;
    fs::write(&component_path, component_content)
      .map_err(|e| anyhow!("Failed to write component file: {}", e))?;

    let relative_path = component_path.strip_prefix(&self.output_path).unwrap_or(&component_path);
    println!("✓ Generated {}", relative_path.display());

    Ok(())
  }

  /// Get the registry configuration
  pub fn config(&self) -> &RegistryConfig {
    &self.config
  }

  /// Get the base path
  pub fn base_path(&self) -> &Path {
    &self.base_path
  }

  /// Get the output path
  pub fn output_path(&self) -> &Path {
    &self.output_path
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Write;

  #[test]
  fn test_registry_config_parsing() {
    let config_json = r#"{
      "name": "test-registry",
      "description": "A test registry",
      "styles": ["default", "new-york"],
      "default_style": "default",
      "components": {
        "button": {
          "name": "button",
          "type": "registry:ui",
          "description": "A button component",
          "registryDependencies": ["utils"],
          "default_files": [
            {
              "source": "src/button.tsx",
              "target": "ui/button.tsx"
            }
          ]
        }
      }
    }"#;

    let config: RegistryConfig = serde_json::from_str(config_json).unwrap();
    assert_eq!(config.name, "test-registry");
    assert_eq!(config.styles.as_ref().unwrap().len(), 2);
    assert!(config.components.contains_key("button"));
  }

  #[test]
  fn test_registry_builder_creation() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let config_path = temp_dir.path().join("registry.json");
    let output_path = temp_dir.path().join("output");

    let config = RegistryConfig {
      schema: None,
      name: "test".to_string(),
      description: None,
      homepage: None,
      docs: None,
      author: None,
      styles: None,
      default_style: None,
      components: HashMap::new(),
    };

    let mut file = fs::File::create(&config_path)?;
    file.write_all(serde_json::to_string(&config)?.as_bytes())?;

    let builder = RegistryBuilder::new(&config_path, &output_path)?;
    assert_eq!(builder.config().name, "test");

    Ok(())
  }
}
