use std::collections::HashMap;

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

/// Component information from registry
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Component {
  #[serde(rename = "$schema")]
  pub schema: Option<String>,
  pub name: String,
  #[serde(rename = "type")]
  pub component_type: Option<String>,
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
  pub target: String,
}

/// Registry index containing available components (array format from shadcn)
pub type RegistryIndex = Vec<ComponentInfo>;

/// Basic component information in the index
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ComponentInfo {
  pub name: String,
  #[serde(rename = "type")]
  pub component_type: Option<String>,
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
  base_url: String,
  namespace: String,
}

impl RegistryClient {
  /// Create a new registry client
  pub fn new(base_url: String, namespace: String) -> Result<Self> {
    let client = Client::builder().user_agent("uiget-cli/0.1.0").build()?;

    // Validate URL
    Url::parse(&base_url)?;

    Ok(Self {
      client,
      base_url,
      namespace,
    })
  }

  /// Fetch the registry index
  pub async fn fetch_index(&self) -> Result<RegistryIndex> {
    let url = format!(
      "{}/registry/index.json",
      self.base_url.trim_end_matches('/')
    );

    let response = self.client.get(&url).send().await?;

    if !response.status().is_success() {
      return Err(anyhow!(
        "Failed to fetch registry index: {}",
        response.status()
      ));
    }

    let index: RegistryIndex = response.json().await?;
    Ok(index)
  }

  /// Fetch a specific component
  pub async fn fetch_component(&self, component_name: &str) -> Result<Component> {
    let url = format!(
      "{}/registry/{}.json",
      self.base_url.trim_end_matches('/'),
      component_name
    );

    let response = self.client.get(&url).send().await?;

    if !response.status().is_success() {
      return Err(anyhow!(
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
    &self.base_url
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

  /// Add a registry
  pub fn add_registry(&mut self, namespace: String, url: String) -> Result<()> {
    let client = RegistryClient::new(url, namespace.clone())?;
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
      .ok_or_else(|| anyhow!("Registry '{}' not found", namespace))?;

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
    // Try default registry first
    if let Some(registry) = self.get_registry("default") {
      if let Ok(component) = registry.fetch_component(component_name).await {
        return Ok(component);
      }
    }

    // Try all other registries
    for (namespace, registry) in &self.registries {
      if namespace == "default" {
        continue;
      }

      if let Ok(component) = registry.fetch_component(component_name).await {
        return Ok(component);
      }
    }

    Err(anyhow!(
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
}
