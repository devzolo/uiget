use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Registry configuration - can be either a simple URL string or an object with URL, params, and headers
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum RegistryConfig {
  /// Simple URL string with {name} placeholder
  String(String),
  /// Full registry configuration with URL, params, and headers
  Object {
    /// Registry URL with {name} placeholder
    url: String,
    /// Optional query parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<HashMap<String, String>>,
    /// Optional HTTP headers
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<HashMap<String, String>>,
  },
}

impl RegistryConfig {
  /// Get the URL from the registry configuration
  pub fn url(&self) -> &str {
    match self {
      RegistryConfig::String(url) => url,
      RegistryConfig::Object { url, .. } => url,
    }
  }

  /// Get the params from the registry configuration
  pub fn params(&self) -> Option<&HashMap<String, String>> {
    match self {
      RegistryConfig::String(_) => None,
      RegistryConfig::Object { params, .. } => params.as_ref(),
    }
  }

  /// Get the headers from the registry configuration
  pub fn headers(&self) -> Option<&HashMap<String, String>> {
    match self {
      RegistryConfig::String(_) => None,
      RegistryConfig::Object { headers, .. } => headers.as_ref(),
    }
  }
}

/// Default registries when not specified in config
fn default_registries() -> HashMap<String, RegistryConfig> {
  let mut registries = HashMap::new();
  registries.insert(
    "default".to_string(),
    RegistryConfig::String("https://shadcn-svelte.com/registry/{name}.json".to_string()),
  );
  registries
}

/// Configuration for the uiget CLI tool
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
  #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
  pub schema: Option<String>,

  /// DEPRECATED IN TAILWIND v4! The style for your components.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub style: Option<String>,

  /// Tailwind CSS configuration
  pub tailwind: TailwindConfig,

  /// Import aliases configuration
  pub aliases: AliasesConfig,

  /// Multiple registry configurations by namespace
  #[serde(default = "default_registries")]
  pub registries: HashMap<String, RegistryConfig>,

  /// TypeScript configuration
  #[serde(skip_serializing_if = "Option::is_none")]
  pub typescript: Option<TypeScriptConfig>,
}

/// Tailwind CSS configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TailwindConfig {
  /// Path to the CSS file that imports Tailwind CSS into your project
  pub css: String,

  /// Used to generate the default color palette for your components
  #[serde(rename = "baseColor")]
  pub base_color: String,

  /// DEPRECATED IN TAILWIND v4! The path to your tailwind.config.[js|ts] file
  #[serde(skip_serializing_if = "Option::is_none")]
  pub config: Option<String>,
}

/// Import aliases configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AliasesConfig {
  /// Import alias for your components
  pub components: String,

  /// Import alias for your utility functions
  pub utils: String,

  /// Import alias for your UI components. Defaults to $lib/components/ui
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ui: Option<String>,

  /// Import alias for your hooks. Defaults to $lib/hooks
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hooks: Option<String>,

  /// Import alias for your library
  #[serde(skip_serializing_if = "Option::is_none")]
  pub lib: Option<String>,
}

/// TypeScript configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum TypeScriptConfig {
  Boolean(bool),
  Object {
    /// Path to the tsconfig/jsconfig file
    config: String,
  },
}

/// Internal TypeScript configuration structure for parsing tsconfig.json
#[derive(Debug, Deserialize, Clone)]
pub struct TsConfig {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub extends: Option<String>,
  
  #[serde(rename = "compilerOptions", skip_serializing_if = "Option::is_none")]
  pub compiler_options: Option<CompilerOptions>,
}

/// TypeScript compiler options
#[derive(Debug, Deserialize, Clone)]
pub struct CompilerOptions {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub paths: Option<HashMap<String, Vec<String>>>,
  
  #[serde(rename = "baseUrl", skip_serializing_if = "Option::is_none")]
  pub base_url: Option<String>,
}

/// Resolved path mapping from tsconfig.json
#[derive(Debug, Clone)]
pub struct ResolvedPaths {
  pub paths: HashMap<String, String>,
  #[allow(dead_code)]
  pub base_url: String,
}

impl Default for Config {
  fn default() -> Self {
    let mut registries = HashMap::new();
    registries.insert(
      "default".to_string(),
      RegistryConfig::String("https://shadcn-svelte.com/registry/{name}.json".to_string()),
    );

    Self {
      schema: Some("https://shadcn-svelte.com/schema.json".to_string()),
      style: None,
      tailwind: TailwindConfig {
        css: "src/app.css".to_string(),
        base_color: "slate".to_string(),
        config: None,
      },
      aliases: AliasesConfig {
        components: "$lib/components".to_string(),
        utils: "$lib/utils".to_string(),
        ui: Some("$lib/components/ui".to_string()),
        hooks: Some("$lib/hooks".to_string()),
        lib: Some("$lib".to_string()),
      },
      registries,
      typescript: Some(TypeScriptConfig::Boolean(true)),
    }
  }
}

impl Config {
  /// Load configuration from a file
  pub fn load_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
    if !path.exists() {
      return Ok(Self::default());
    }

    let content = std::fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
  }

  /// Save configuration to a file
  pub fn save_to_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
    let content = serde_json::to_string_pretty(self)?;
    std::fs::write(path, content)?;
    Ok(())
  }

  /// Get registry configuration by namespace
  pub fn get_registry(&self, namespace: &str) -> Option<&RegistryConfig> {
    self
      .registries
      .get(namespace)
      .or_else(|| self.registries.get("default"))
      .or_else(|| self.registries.get("@default"))
  }

  /// Get registry URL by namespace
  #[allow(dead_code)]
  pub fn get_registry_url(&self, namespace: &str) -> Option<&str> {
    self.get_registry(namespace).map(|config| config.url())
  }

  /// Add or update a registry with a simple URL
  pub fn set_registry(&mut self, namespace: String, url: String) {
    self.registries.insert(namespace, RegistryConfig::String(url));
  }

  /// Add or update a registry with full configuration
  #[allow(dead_code)]
  pub fn set_registry_config(&mut self, namespace: String, config: RegistryConfig) {
    self.registries.insert(namespace, config);
  }

  /// Add or update a registry with URL, params, and headers
  #[allow(dead_code)]
  pub fn set_registry_with_config(
    &mut self,
    namespace: String,
    url: String,
    params: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
  ) {
    let config = RegistryConfig::Object { url, params, headers };
    self.registries.insert(namespace, config);
  }

  /// Resolve TypeScript configuration and path mappings
  pub fn resolve_typescript_paths(&self) -> anyhow::Result<Option<ResolvedPaths>> {
    match &self.typescript {
      Some(TypeScriptConfig::Boolean(true)) => {
        // Default to tsconfig.json in current directory
        self.resolve_tsconfig_paths("tsconfig.json")
      }
      Some(TypeScriptConfig::Object { config }) => {
        self.resolve_tsconfig_paths(config)
      }
      _ => Ok(None),
    }
  }

  /// Resolve paths from a specific tsconfig file
  fn resolve_tsconfig_paths(&self, config_path: &str) -> anyhow::Result<Option<ResolvedPaths>> {
    let config_path = Path::new(config_path);
    
    if !config_path.exists() {
      return Ok(None);
    }

    let resolved_config = self.resolve_tsconfig_with_extends(config_path)?;
    
    if let Some(compiler_options) = resolved_config.compiler_options {
      if let Some(paths) = compiler_options.paths {
        let base_url = compiler_options.base_url.unwrap_or_else(|| ".".to_string());
        let resolved_paths = self.resolve_path_mappings(paths, config_path, &base_url)?;
        
        return Ok(Some(ResolvedPaths {
          paths: resolved_paths,
          base_url,
        }));
      }
    }

    Ok(None)
  }

  /// Resolve tsconfig.json with extends support
  fn resolve_tsconfig_with_extends(&self, config_path: &Path) -> anyhow::Result<TsConfig> {
    let content = std::fs::read_to_string(config_path)?;
    
    // Parse JSON5 content (supports comments, trailing commas, etc.)
    let mut config: TsConfig = json5::from_str(&content)
      .map_err(|e| anyhow::anyhow!("Failed to parse tsconfig.json: {}", e))?;

    // Handle extends
    if let Some(extends_path) = &config.extends {
      let base_dir = config_path.parent().unwrap_or(Path::new("."));
      let extended_config_path = base_dir.join(extends_path);
      
      if extended_config_path.exists() {
        let extended_config = self.resolve_tsconfig_with_extends(&extended_config_path)?;
        
        // Merge compiler options
        if let Some(extended_compiler_options) = extended_config.compiler_options {
          if let Some(ref mut compiler_options) = config.compiler_options {
            // Merge paths
            if let Some(extended_paths) = extended_compiler_options.paths {
              let current_paths = compiler_options.paths.get_or_insert_with(HashMap::new);
              for (key, value) in extended_paths {
                current_paths.entry(key).or_insert(value);
              }
            }
            
            // Use base_url from extended config if not present
            if compiler_options.base_url.is_none() {
              compiler_options.base_url = extended_compiler_options.base_url;
            }
          } else {
            config.compiler_options = Some(extended_compiler_options);
          }
        }
      }
    }

    Ok(config)
  }

  /// Resolve path mappings to absolute file system paths
  fn resolve_path_mappings(
    &self,
    paths: HashMap<String, Vec<String>>,
    config_path: &Path,
    base_url: &str,
  ) -> anyhow::Result<HashMap<String, String>> {
    let mut resolved_paths = HashMap::new();
    let config_dir = config_path.parent().unwrap_or(Path::new("."));
    let base_path = config_dir.join(base_url);

    for (alias, targets) in paths {
      // Take the first target path for simplicity
      if let Some(target) = targets.first() {
        // Remove wildcard suffix from alias and target
        let clean_alias = alias.trim_end_matches("/*").trim_end_matches("*");
        let clean_target = target.trim_end_matches("/*").trim_end_matches("*");
        
        // Resolve relative paths
        let resolved_target = if clean_target.starts_with("./") || clean_target.starts_with("../") {
          base_path.join(clean_target)
        } else {
          base_path.join(clean_target)
        };

        // Simplify the path without canonicalizing (which can cause UNC path issues on Windows)
        let simplified_target = self.simplify_path(&resolved_target);

        // Convert to relative path from current working directory  
        let current_dir = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
        let relative_target = if let Ok(relative) = simplified_target.strip_prefix(&current_dir) {
          relative.to_path_buf()
        } else {
          simplified_target
        };

        // Convert to string and normalize path separators
        if let Some(target_str) = relative_target.to_str() {
          let normalized_str = target_str.replace('\\', "/");
          // Clean up redundant "./" at the beginning
          let clean_str = if normalized_str.starts_with("./") {
            &normalized_str[2..]
          } else {
            &normalized_str
          };
          
          resolved_paths.insert(
            clean_alias.to_string(),
            clean_str.to_string()
          );
        }
      }
    }

    Ok(resolved_paths)
  }

  /// Simplify a path by resolving .. and . components without canonicalizing
  fn simplify_path(&self, path: &Path) -> PathBuf {
    let mut components = Vec::new();
    
    for component in path.components() {
      match component {
        std::path::Component::Normal(name) => {
          components.push(name);
        }
        std::path::Component::ParentDir => {
          if !components.is_empty() {
            components.pop();
          }
        }
        std::path::Component::CurDir => {
          // Skip current directory components
        }
        std::path::Component::RootDir | std::path::Component::Prefix(_) => {
          // Keep root and prefix components for absolute paths
          components.clear(); // Reset for absolute path
          if let std::path::Component::Prefix(_) = component {
            components.push(component.as_os_str());
          }
        }
      }
    }
    
    let mut result = PathBuf::new();
    for component in components {
      result.push(component);
    }
    
    if result.as_os_str().is_empty() {
      PathBuf::from(".")
    } else {
      result
    }
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use super::*;

  #[test]
  fn test_config_serialization() {
    let mut registries = HashMap::new();
    registries.insert(
      "default".to_string(),
      RegistryConfig::String("https://shadcn-svelte.com/registry/{name}.json".to_string()),
    );
    registries.insert(
      "custom".to_string(), 
      RegistryConfig::String("https://my-registry.com/registry/{name}.json".to_string())
    );

    let config = Config {
      schema: Some("https://shadcn-svelte.com/schema.json".to_string()),
      style: None,
      tailwind: TailwindConfig {
        css: "src/app.css".to_string(),
        base_color: "slate".to_string(),
        config: None,
      },
      aliases: AliasesConfig {
        components: "$lib/components".to_string(),
        utils: "$lib/utils".to_string(),
        ui: Some("$lib/components/ui".to_string()),
        hooks: None,
        lib: None,
      },
      registries,
      typescript: Some(TypeScriptConfig::Boolean(true)),
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    let deserialized: Config = serde_json::from_str(&json).unwrap();

    assert_eq!(config.tailwind.css, deserialized.tailwind.css);
    assert_eq!(config.registries.len(), deserialized.registries.len());
  }

  #[test]
  fn test_get_registry_url() {
    let mut config = Config::default();
    config.set_registry(
      "custom".to_string(),
      "https://custom-registry.com".to_string(),
    );

    assert_eq!(
      config.get_registry_url("custom"),
      Some("https://custom-registry.com")
    );
    assert_eq!(
      config.get_registry_url("nonexistent"),
      Some("https://shadcn-svelte.com/registry/{name}.json")
    );
  }

  #[test]
  fn test_registry_config_schema() {
    // Test simple string format
    let string_config = RegistryConfig::String("https://example.com/{name}".to_string());
    assert_eq!(string_config.url(), "https://example.com/{name}");
    assert!(string_config.params().is_none());
    assert!(string_config.headers().is_none());

    // Test object format with all fields
    let mut params = HashMap::new();
    params.insert("api_key".to_string(), "test-key".to_string());
    
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer token".to_string());

    let object_config = RegistryConfig::Object {
      url: "https://api.example.com/components/{name}".to_string(),
      params: Some(params.clone()),
      headers: Some(headers.clone()),
    };

    assert_eq!(object_config.url(), "https://api.example.com/components/{name}");
    assert_eq!(object_config.params(), Some(&params));
    assert_eq!(object_config.headers(), Some(&headers));

    // Test serialization/deserialization
    let json_string = serde_json::to_string(&string_config).unwrap();
    let json_object = serde_json::to_string(&object_config).unwrap();

    let deserialized_string: RegistryConfig = serde_json::from_str(&json_string).unwrap();
    let deserialized_object: RegistryConfig = serde_json::from_str(&json_object).unwrap();

    assert_eq!(deserialized_string.url(), string_config.url());
    assert_eq!(deserialized_object.url(), object_config.url());
    assert_eq!(deserialized_object.params(), object_config.params());
    assert_eq!(deserialized_object.headers(), object_config.headers());
  }

  #[test]
  fn test_config_with_new_registry_schema() {
    let mut config = Config::default();
    
    // Add simple string registry
    config.set_registry("simple".to_string(), "https://simple.com".to_string());
    
    // Add complex registry with params and headers
    let mut params = HashMap::new();
    params.insert("version".to_string(), "v1".to_string());
    
    let mut headers = HashMap::new();
    headers.insert("User-Agent".to_string(), "uiget-test".to_string());
    
    config.set_registry_with_config(
      "complex".to_string(),
      "https://api.complex.com/registry/{name}".to_string(),
      Some(params),
      Some(headers),
    );

    // Test retrieval
    assert_eq!(config.get_registry_url("simple"), Some("https://simple.com"));
    assert_eq!(config.get_registry_url("complex"), Some("https://api.complex.com/registry/{name}"));

    let complex_config = config.get_registry("complex").unwrap();
    assert!(complex_config.params().is_some());
    assert!(complex_config.headers().is_some());

    // Test serialization
    let json = serde_json::to_string_pretty(&config).unwrap();
    let deserialized: Config = serde_json::from_str(&json).unwrap();

    assert_eq!(config.registries.len(), deserialized.registries.len());
    assert_eq!(
      config.get_registry_url("simple"),
      deserialized.get_registry_url("simple")
    );
    assert_eq!(
      config.get_registry_url("complex"),
      deserialized.get_registry_url("complex")
    );
  }

  #[test]
  fn test_style_configuration() {
    let mut config = Config::default();
    
    // Test that style can be set and retrieved
    config.style = Some("new-york".to_string());
    assert_eq!(config.style, Some("new-york".to_string()));

    // Test serialization with style
    let json = serde_json::to_string_pretty(&config).unwrap();
    let deserialized: Config = serde_json::from_str(&json).unwrap();

    assert_eq!(config.style, deserialized.style);
  }
}
