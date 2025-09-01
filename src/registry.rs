use std::collections::HashMap;

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::RegistryConfig;

/// Component information from registry
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Component {
  #[serde(rename = "$schema")]
  pub schema: Option<String>,
  pub name: String,
  #[serde(rename = "type")]
  pub component_type: Option<String>,
  #[serde(rename = "dependencies")]
  pub dependencies: Option<Vec<String>>,
  #[serde(rename = "devDependencies")]
  pub dev_dependencies: Option<Vec<String>>,
  #[serde(rename = "registryDependencies")]
  pub registry_dependencies: Option<Vec<String>>,
  pub files: Vec<ComponentFile>,
  #[serde(skip)]
  pub registry: Option<String>,
}

/// Component file information
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ComponentFile {
  pub content: String,
  #[serde(rename = "type")]
  pub file_type: Option<String>,
  #[serde(rename = "target")]
  pub target: Option<String>,
  pub path: Option<String>,
}

impl ComponentFile {
  /// Get the target path, using path field if target is empty or missing
  pub fn get_target_path(&self) -> String {
    if let Some(target) = &self.target {
      if !target.is_empty() {
        return target.clone();
      }
    }

    if let Some(path) = &self.path {
      if !path.is_empty() {
        return path.clone();
      }
    }

    String::new()
  }
}

/// Registry index containing available components
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum RegistryIndex {
  /// Array format (shadcn-svelte style)
  Array(Vec<ComponentInfo>),
  /// Object format (shadcn/ui style)
  Object(std::collections::HashMap<String, ComponentInfo>),
}

impl RegistryIndex {
  /// Convert to vector regardless of format
  pub fn to_vec(self) -> Vec<ComponentInfo> {
    match self {
      RegistryIndex::Array(vec) => vec,
      RegistryIndex::Object(map) => map.into_values().collect(),
    }
  }

  /// Get as slice for iteration
  pub fn as_slice(&self) -> Vec<&ComponentInfo> {
    match self {
      RegistryIndex::Array(vec) => vec.iter().collect(),
      RegistryIndex::Object(map) => map.values().collect(),
    }
  }

  /// Check if empty
  pub fn is_empty(&self) -> bool {
    match self {
      RegistryIndex::Array(vec) => vec.is_empty(),
      RegistryIndex::Object(map) => map.is_empty(),
    }
  }

  /// Get length
  pub fn len(&self) -> usize {
    match self {
      RegistryIndex::Array(vec) => vec.len(),
      RegistryIndex::Object(map) => map.len(),
    }
  }
}

/// Basic component information in the index
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ComponentInfo {
  pub name: String,
  #[serde(rename = "type")]
  pub component_type: Option<String>,
  #[serde(rename = "dependencies")]
  pub dependencies: Option<Vec<String>>,
  #[serde(rename = "registryDependencies")]
  pub registry_dependencies: Option<Vec<String>>,
  #[serde(rename = "devDependencies")]
  pub dev_dependencies: Option<Vec<String>>,
  #[serde(rename = "relativeUrl")]
  pub relative_url: Option<String>,
}

/// Registry client for fetching components
pub struct RegistryClient {
  client: Client,
  config: RegistryConfig,
  namespace: String,
  style: Option<String>,
}

impl RegistryClient {
  /// Create a new registry client with simple URL
  #[allow(dead_code)]
  pub fn new(base_url: String, namespace: String) -> Result<Self> {
    let config = RegistryConfig::String(base_url);
    Self::new_with_config(config, namespace, None)
  }

  /// Create a new registry client with style
  pub fn new_with_style(base_url: String, namespace: String, style: Option<String>) -> Result<Self> {
    let config = RegistryConfig::String(base_url);
    Self::new_with_config(config, namespace, style)
  }

  /// Create a new registry client with full configuration
  pub fn new_with_config(config: RegistryConfig, namespace: String, style: Option<String>) -> Result<Self> {
    let mut client_builder = Client::builder().user_agent("uiget-cli/0.1.0");

    // Add default headers from config if available
    if let Some(headers) = config.headers() {
      let mut header_map = reqwest::header::HeaderMap::new();
      for (key, value) in headers {
        if let (Ok(header_name), Ok(header_value)) = (
          reqwest::header::HeaderName::from_bytes(key.as_bytes()),
          reqwest::header::HeaderValue::from_str(value)
        ) {
          header_map.insert(header_name, header_value);
        }
      }
      client_builder = client_builder.default_headers(header_map);
    }

    let client = client_builder.build()?;

    // Validate URL
    Url::parse(config.url())?;

    Ok(Self {
      client,
      config,
      namespace,
      style,
    })
  }

  /// Fetch the registry index
  pub async fn fetch_index(&self) -> Result<RegistryIndex> {
    // Try different possible index endpoints
    let mut index_urls = vec![];

    // For shadcn/ui, use the correct index endpoint: ui.shadcn.com/r/index.json
    if self.config.url().contains("ui.shadcn.com") {
      index_urls.push("https://ui.shadcn.com/r/index.json".to_string());
    }

    // For other registries with {style} URLs, try {style}/index.json
    if self.config.url().contains("{style}") && !self.config.url().contains("ui.shadcn.com") {
      index_urls.push(self.config.url().replace("{name}", "index"));
    }

    // Try other common patterns
    index_urls.extend(vec![
      self.config.url().replace("{name}", "index"),
      format!("{}/index.json", self.config.url().trim_end_matches('/')).replace("/{name}.json", ""),
      format!("{}/registry/index.json", self.config.url().trim_end_matches('/')).replace("/{name}.json", ""),
    ]);

    for mut url in index_urls {
      // Replace {style} placeholder if style is provided (except for the main shadcn index)
      if let Some(style) = &self.style {
        if !url.starts_with("https://ui.shadcn.com/r/index.json") {
          url = url.replace("{style}", style);
        }
      }

      let mut request_builder = self.client.get(&url);

      // Add query parameters if available
      if let Some(params) = self.config.params() {
        for (key, value) in params {
          request_builder = request_builder.query(&[(key, value)]);
        }
      }

      if let Ok(response) = request_builder.send().await {
        if response.status().is_success() {
          if let Ok(index) = response.json::<RegistryIndex>().await {
            return Ok(index);
          }
        }
      }
    }

    // If no index endpoint works, return empty index
    Ok(RegistryIndex::Array(vec![]))
  }

  /// Get a fallback list of known shadcn/ui components
  /// This is used when the registry doesn't provide a public index endpoint
  #[allow(dead_code)]
  fn get_shadcn_ui_fallback_components(&self) -> RegistryIndex {
    // TODO: Implement fallback components list
    let components = vec![];
    RegistryIndex::Array(components)
  }

  /// Fetch a specific component
  pub async fn fetch_component(&self, component_name: &str) -> Result<Component> {
    // Replace {name} placeholder with component name
    let mut url = self.config.url().replace("{name}", component_name);

    // Replace {style} placeholder if style is provided
    if let Some(style) = &self.style {
      url = url.replace("{style}", style);
    }

    let mut request_builder = self.client.get(&url);

    // Add query parameters if available
    if let Some(params) = self.config.params() {
      for (key, value) in params {
        request_builder = request_builder.query(&[(key, value)]);
      }
    }

    let response = request_builder.send().await?;

    if !response.status().is_success() {
      return Err(anyhow::anyhow!(
        "Failed to fetch component '{}': {}",
        component_name,
        response.status()
      ));
    }

    let mut component: Component = response.json().await?;
    component.registry = Some(self.namespace.clone());

    Ok(component)
  }

  /// Search components by name or type
  pub async fn search_components(&self, query: &str) -> Result<Vec<ComponentInfo>> {
    let index = self.fetch_index().await?;

    let query_lower = query.to_lowercase();
    let filtered: Vec<ComponentInfo> = index
      .to_vec()
      .into_iter()
      .filter(|comp| {
        comp.name.to_lowercase().contains(&query_lower)
          || comp
            .component_type
            .as_ref()
            .map(|comp_type| comp_type.to_lowercase().contains(&query_lower))
            .unwrap_or(false)
      })
      .collect();

    Ok(filtered)
  }

  /// Get the namespace of this registry
  #[allow(dead_code)]
  pub fn namespace(&self) -> &str {
    &self.namespace
  }

  /// Get the base URL of this registry
  #[allow(dead_code)]
  pub fn base_url(&self) -> &str {
    self.config.url()
  }

  /// Get the registry configuration
  #[allow(dead_code)]
  pub fn config(&self) -> &RegistryConfig {
    &self.config
  }

  /// Get the style
  #[allow(dead_code)]
  pub fn style(&self) -> Option<&String> {
    self.style.as_ref()
  }
}

/// Registry manager for handling multiple registries
pub struct RegistryManager {
  registries: HashMap<String, RegistryClient>,
}

impl RegistryManager {
  /// Create a new registry manager
  pub fn new() -> Self {
    Self {
      registries: HashMap::new(),
    }
  }

  /// Add a registry with simple URL
  #[allow(dead_code)]
  pub fn add_registry(&mut self, namespace: String, url: String) -> Result<()> {
    let client = RegistryClient::new(url, namespace.clone())?;
    self.registries.insert(namespace, client);
    Ok(())
  }

  /// Add a registry with simple URL and style
  pub fn add_registry_with_style(&mut self, namespace: String, url: String, style: Option<String>) -> Result<()> {
    let client = RegistryClient::new_with_style(url, namespace.clone(), style)?;
    self.registries.insert(namespace, client);
    Ok(())
  }

  /// Add a registry with full configuration
  #[allow(dead_code)]
  pub fn add_registry_config(&mut self, namespace: String, config: RegistryConfig) -> Result<()> {
    let client = RegistryClient::new_with_config(config, namespace.clone(), None)?;
    self.registries.insert(namespace, client);
    Ok(())
  }

  /// Add a registry with full configuration and style
  pub fn add_registry_config_with_style(&mut self, namespace: String, config: RegistryConfig, style: Option<String>) -> Result<()> {
    let client = RegistryClient::new_with_config(config, namespace.clone(), style)?;
    self.registries.insert(namespace, client);
    Ok(())
  }

  /// Get a registry by namespace
  pub fn get_registry(&self, namespace: &str) -> Option<&RegistryClient> {
    self.registries.get(namespace)
  }

  /// Get all registry namespaces
  pub fn namespaces(&self) -> Vec<&String> {
    self.registries.keys().collect()
  }

  /// Fetch component from specific registry
  pub async fn fetch_component(&self, namespace: &str, component_name: &str) -> Result<Component> {
    let registry = self
      .get_registry(namespace)
      .ok_or_else(|| anyhow::anyhow!("Registry '{}' not found", namespace))?;

    registry.fetch_component(component_name).await
  }

  /// Search components across all registries
  pub async fn search_all(&self, query: &str) -> Result<HashMap<String, Vec<ComponentInfo>>> {
    let mut results = HashMap::new();

    for (namespace, registry) in &self.registries {
      match registry.search_components(query).await {
        Ok(components) => {
          if !components.is_empty() {
            results.insert(namespace.clone(), components);
          }
        }
        Err(e) => {
          eprintln!(
            "Warning: Failed to search in registry '{}': {}",
            namespace, e
          );
        }
      }
    }

    Ok(results)
  }

  /// Fetch component from any registry (tries default first)
  pub async fn fetch_component_auto(&self, component_name: &str) -> Result<Component> {
    // Try default registries first (both "default" and "@default")
    for default_namespace in ["default", "@default"] {
      if let Some(registry) = self.get_registry(default_namespace) {
        if let Ok(component) = registry.fetch_component(component_name).await {
          return Ok(component);
        }
      }
    }

    // Try all other registries
    for (namespace, registry) in &self.registries {
      if namespace == "default" || namespace == "@default" {
        continue;
      }

      if let Ok(component) = registry.fetch_component(component_name).await {
        return Ok(component);
      }
    }

    Err(anyhow::anyhow!(
      "Component '{}' not found in any registry",
      component_name
    ))
  }
}

impl Default for RegistryManager {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_registry_client_creation() {
    let client = RegistryClient::new("https://example.com".to_string(), "test".to_string());
    assert!(client.is_ok());

    let client = client.unwrap();
    assert_eq!(client.namespace(), "test");
    assert_eq!(client.base_url(), "https://example.com");
  }

  #[test]
  fn test_invalid_url() {
    let client = RegistryClient::new("not-a-url".to_string(), "test".to_string());
    assert!(client.is_err());
  }

  #[test]
  fn test_registry_manager() {
    let mut manager = RegistryManager::new();

    let result = manager.add_registry("test".to_string(), "https://example.com".to_string());
    assert!(result.is_ok());

    assert!(manager.get_registry("test").is_some());
    assert!(manager.get_registry("nonexistent").is_none());

    let namespaces = manager.namespaces();
    assert_eq!(namespaces.len(), 1);
    assert!(namespaces.contains(&&"test".to_string()));
  }

  #[test]
  fn test_registry_client_with_style() {
    let style = Some("new-york".to_string());
    let client = RegistryClient::new_with_style(
      "https://example.com/styles/{style}/{name}.json".to_string(),
      "test".to_string(),
      style.clone()
    );

    assert!(client.is_ok());
    let client = client.unwrap();
    assert_eq!(client.namespace(), "test");
    assert_eq!(client.base_url(), "https://example.com/styles/{style}/{name}.json");
    assert_eq!(client.style(), style.as_ref());
  }

  #[test]
  fn test_registry_manager_with_style() {
    let mut manager = RegistryManager::new();
    let style = Some("new-york".to_string());

    let result = manager.add_registry_with_style(
      "test".to_string(),
      "https://example.com/styles/{style}/{name}.json".to_string(),
      style.clone()
    );
    assert!(result.is_ok());

    let registry = manager.get_registry("test");
    assert!(registry.is_some());

    let registry = registry.unwrap();
    assert_eq!(registry.style(), style.as_ref());
  }
}
