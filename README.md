# uiget 🚀

A modern and efficient CLI tool in Rust for managing shadcn/ui components from multiple registries with advanced interactive interface.

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![MIT License](https://img.shields.io/badge/License-MIT-green.svg?style=for-the-badge)](https://choosealicense.com/licenses/mit/)

## ✨ Features

- 🎯 **Advanced interactive menu** with category selection (UI, Blocks, Hooks, Libraries)
- 📦 **Multiple registry support** by namespace with flexible configuration
- 🔄 **Automatic dependency resolution** with intelligent detection
- ⚡ **Colorful and intuitive interface** with visual status indicators
- 🛠️ **Flexible configuration** of aliases and project structure
- 🔍 **Smart search** of components across all registries
- 📊 **Outdated component detection** with content comparison
- ✅ **Compatible with shadcn-svelte** and shadcn/ui schemas
- 🔧 **Full TypeScript support** with automatic path resolution
- 🌐 **Authenticated registries** (custom headers and parameters)
- 📝 **Intelligent placeholder processing** ($UTILS$, $COMPONENTS$, etc.)
- 🎨 **Style support** (new-york, default) for compatible registries

## 📦 Installation

### Via Cargo (Recommended)

```bash
# Install directly from repository
cargo install --git https://github.com/your-username/uiget

# Or install locally after cloning
git clone https://github.com/your-username/uiget
cd uiget
cargo install --path .
```

### Manual Compilation

```bash
# Clone the repository
git clone https://github.com/your-username/uiget
cd uiget

# Build and install
cargo build --release
cargo install --path .
```

### Installation Verification

```bash
# Check if uiget command is available
uiget --help

# Installed version
uiget --version
```

## 🚀 Quick Start

### 1. Initialize Configuration

```bash
# Create default configuration file
uiget init

# With custom options
uiget init --base-color blue --css "src/styles.css"
```

### 2. Interactive Menu

```bash
# Open interactive menu for component selection
uiget add
```

### 3. Add Specific Components

```bash
# Add a specific component
uiget add button

# Add from a specific registry
uiget add button --registry custom
```

## 🎯 Advanced Interactive Menu

`uiget` offers a modern interactive interface that allows efficient component selection:

### Category Selection

Run `uiget add` to open the main menu:

```bash
? What would you like to do?
❯ 🔍 Browse and select individual components
  📦 Select ALL UI Components (52 items)     ← Select ALL instantly
  🧩 Select ALL Blocks (131 items)          ← Select ALL instantly
  🪝 Select ALL Hooks (1 items)             ← Select ALL instantly
  📚 Select ALL Libraries (1 items)         ← Select ALL instantly
  ⚙️ Select ALL Other (5 items)             ← Select ALL instantly
  ❌ Cancel
```

### Visual Status Indicators

Components are displayed with clear visual indicators:

- **✓** - Component installed and up-to-date
- **⚠** - Component installed but outdated
- **→** - Component not installed

### How to Use

1. **↑↓** - Navigate between options
2. **Enter** - Select complete category or open individual browser
3. **Space** - Mark/unmark individual components (in browser mode)
4. **Final Enter** - Confirm and install selection

### Preview and Confirmation

```bash
✅ Selected ALL ui (52 components)
Components to be installed:
  1. accordion      11. card
  2. alert          12. checkbox
  3. alert-dialog   13. collapsible
  4. aspect-ratio   14. command
  5. avatar         15. context-menu
  ... and 37 more

? Install all 52 components? (Y/n)
```

### Multiple Registry Selection

If you have multiple registries configured, uiget will automatically ask which one to use:

```bash
? Select a registry:
❯ default (shadcn-svelte)
  shadcn-ui (shadcn/ui)
  custom (my-registry)
```

## 📋 Available Commands

### Initial Configuration

```bash
# Initialize project
uiget init [--force] [--base-color COLOR] [--css PATH]

# Example with custom options
uiget init --base-color emerald --css "src/styles/globals.css"
```

### Registry Management

```bash
# Add new registry
uiget registry add registry-name https://my-registry.com

# List configured registries
uiget registry list

# Test registry connection
uiget registry test registry-name

# Remove registry
uiget registry remove registry-name
```

### Components

```bash
# Interactive menu (recommended)
uiget add

# Add specific component
uiget add button

# Add from specific registry
uiget add button --registry custom

# Add using namespace (@namespace/component)
uiget add @shadcn-ui/button

# Add forcing overwrite
uiget add button --force

# Add without dependencies
uiget add button --skip-deps

# Search components in all registries
uiget search "data table"

# Search in specific registry
uiget search "form" --registry shadcn-ui

# List all available components
uiget list

# List from specific registry
uiget list --registry custom

# Detailed component information
uiget info button

# Information from specific registry
uiget info button --registry custom

# Remove component (in development)
uiget remove button

# Check outdated components
uiget outdated

# Check outdated in specific registry
uiget outdated --registry custom

# Update component (force reinstall)
uiget add button --force
```

### Advanced Features

```bash
# Use specific configuration file
uiget --config ./custom-config.json add button

# Verbose mode for debugging
uiget --verbose add button

# Combine options
uiget --verbose --config ./config.json add button --force --skip-deps
```

## ⚙️ Configuration

The `uiget.json` file is created in the project directory with the following structure:

```json
{
  "$schema": "https://shadcn-svelte.com/schema.json",
  "style": "default",
  "tailwind": {
    "css": "src/app.css",
    "baseColor": "slate",
    "config": "tailwind.config.js"
  },
  "aliases": {
    "components": "$lib/components",
    "utils": "$lib/utils",
    "ui": "$lib/components/ui",
    "hooks": "$lib/hooks",
    "lib": "$lib"
  },
  "registries": {
    "default": "https://shadcn-svelte.com/registry/{name}.json",
    "shadcn-ui": {
      "url": "https://ui.shadcn.com/registry/{style}/{name}.json",
      "params": {
        "version": "latest"
      },
      "headers": {
        "User-Agent": "uiget-cli"
      }
    }
  },
  "typescript": {
    "config": "tsconfig.json"
  }
}
```

### Advanced Registry Configuration

uiget supports two registry configuration formats:

#### Simple Format (String)

```json
{
  "registries": {
    "my-registry": "https://api.mysite.com/components/{name}.json"
  }
}
```

#### Advanced Format (Object)

```json
{
  "registries": {
    "registry-auth": {
      "url": "https://private-registry.com/api/{name}.json",
      "params": {
        "api_key": "your-api-key",
        "version": "v2"
      },
      "headers": {
        "Authorization": "Bearer your-token",
        "Content-Type": "application/json"
      }
    }
  }
}
```

### TypeScript Configuration

uiget automatically resolves TypeScript paths:

```json
{
  "typescript": true, // Uses default tsconfig.json
  "typescript": {
    // Or specifies custom file
    "config": "jsconfig.json"
  }
}
```

### Key Differences from Original Schema

- **`registry` → `registries`**: Support for multiple registries by namespace
- **Advanced configuration**: Custom headers and parameters for authentication
- **TypeScript resolution**: Automatic integration with tsconfig.json
- **Smart placeholders**: Processing of $UTILS$, $COMPONENTS$, etc.
- **Style support**: Compatibility with registries that use styles (new-york, default)

## 🏗️ Project Structure

```tree
src/
├── main.rs          # Main entry point
├── cli.rs           # CLI command definitions
├── config.rs        # Configuration structures
├── registry.rs      # Registry client
└── installer.rs     # Component installation logic
```

## 📚 Practical Examples

### Multi-Registry Configuration

```bash
# Add different registries
uiget registry add shadcn-ui https://ui.shadcn.com/registry/{style}/{name}.json
uiget registry add my-components https://my-components.dev/api/{name}.json

# Test registry connections
uiget registry test shadcn-ui
uiget registry test my-components

# Install from specific registries
uiget add button --registry shadcn-ui
uiget add custom-card --registry my-components

# Use @namespace/component format
uiget add @shadcn-ui/button
uiget add @my-components/custom-card
```

### Advanced Search and Information

```bash
# Search in all registries
uiget search "form"

# Search in specific registry
uiget search "table" --registry shadcn-ui

# View detailed information
uiget info data-table
uiget info button --registry shadcn-ui

# List components by category
uiget list --registry shadcn-ui
```

### Typical Development Workflow

```bash
# 1. Initialize project with custom configurations
uiget init --base-color violet --css "src/styles/globals.css"

# 2. Add custom registry with authentication
uiget registry add company https://components.company.com/api/{name}.json

# 3. Use interactive menu to install components
uiget add
# Select "📦 Select ALL UI Components" to install all at once

# 4. Check status and updates
uiget list                    # View all available components
uiget outdated               # Check outdated components
uiget add button --force     # Update specific component

# 5. Debug and troubleshooting
uiget --verbose add card     # Verbose mode for debugging
```

### Advanced Scenarios

```bash
# Working with TypeScript
# uiget automatically resolves paths from tsconfig.json
uiget add button  # Placeholders like $UTILS$ are resolved automatically

# Configuration for Svelte projects
uiget init --components "$lib/components" --utils "$lib/utils"

# Configuration for React/Next.js projects
uiget init --components "./components" --utils "./lib/utils"

# Use registry with specific style
uiget registry add shadcn-ny https://ui.shadcn.com/registry/new-york/{name}.json
uiget add button --registry shadcn-ny

# Install without dependencies (useful for development)
uiget add complex-component --skip-deps
```

## 🔧 TypeScript Integration

uiget offers full TypeScript support with advanced features:

### Automatic Path Resolution

uiget automatically reads your `tsconfig.json` and resolves path mappings:

```json
// tsconfig.json
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"],
      "@/components/*": ["./src/components/*"],
      "@/utils/*": ["./src/lib/utils/*"]
    }
  }
}
```

```json
// uiget.json
{
  "aliases": {
    "components": "@/components",
    "utils": "@/utils",
    "ui": "@/components/ui"
  }
}
```

### Intelligent Import Processing

uiget automatically:

1. **Removes .js extensions** in TypeScript projects
2. **Resolves placeholders** based on configuration
3. **Normalizes paths** for the local file system

```typescript
// Before (from registry)
import { cn } from "$UTILS$.js";
import Button from "$COMPONENTS$/ui/button.js";

// After (processed by uiget)
import { cn } from "@/utils";
import Button from "@/components/ui/button";
```

### Extends Support

uiget supports TypeScript configurations with `extends`:

```json
// tsconfig.json
{
  "extends": "./tsconfig.base.json",
  "compilerOptions": {
    "paths": {
      "@/*": ["./src/*"]
    }
  }
}
```

## 🔧 Registry API

Registries must implement the following structure to be compatible with uiget:

### Supported Endpoints

```bash
# Component index (multiple formats supported)
GET /registry/index.json                    # Array format (shadcn-svelte)
GET /r/index.json                          # shadcn/ui format
GET /{style}/index.json                    # With style support

# Individual components
GET /registry/{name}.json                  # Basic format
GET /registry/{style}/{name}.json          # With style
GET /api/components/{name}.json            # Custom API
```

### Index Format

uiget supports two index formats:

#### Array Format (shadcn-svelte)

```json
[
  {
    "name": "button",
    "type": "registry:ui",
    "registryDependencies": ["utils"],
    "devDependencies": ["@types/node"]
  }
]
```

#### Object Format (shadcn/ui)

```json
{
  "button": {
    "name": "button",
    "type": "registry:ui",
    "registryDependencies": ["utils"]
  }
}
```

### Component Format

```json
{
  "name": "button",
  "type": "registry:ui",
  "registryDependencies": ["utils", "cn"],
  "devDependencies": ["@types/react"],
  "files": [
    {
      "target": "ui/button/button.tsx",
      "content": "import { cn } from '$UTILS$';\n\n// Component...",
      "type": "registry:ui"
    },
    {
      "target": "ui/button/index.ts",
      "content": "export { Button } from './button';"
    }
  ]
}
```

### Supported Placeholders

uiget automatically processes the following placeholders:

- **`$UTILS$`** - Resolved to the configured utils alias
- **`$COMPONENTS$`** - Resolved to the configured components alias
- **`$HOOKS$`** - Resolved to the configured hooks alias
- **`$LIB$`** - Resolved to the configured lib alias

### Style Support

For registries that support multiple styles (like shadcn/ui):

```json
{
  "registries": {
    "shadcn-default": "https://ui.shadcn.com/registry/default/{name}.json",
    "shadcn-ny": "https://ui.shadcn.com/registry/new-york/{name}.json"
  }
}
```

### Authentication

For private registries, use the advanced format:

```json
{
  "registries": {
    "private-registry": {
      "url": "https://api.company.com/components/{name}.json",
      "headers": {
        "Authorization": "Bearer your-token-here"
      },
      "params": {
        "version": "latest"
      }
    }
  }
}
```

## 🧪 Development

### Prerequisites

- Rust 1.70+
- Cargo

### Development Commands

```bash
# Run in debug mode
cargo run -- --help

# Run tests
cargo test

# Run with debug logs
RUST_LOG=debug cargo run -- --verbose list

# Check linting
cargo clippy

# Format code
cargo fmt

# Optimized build
cargo build --release
```

### Test Structure

```bash
# Run all tests
cargo test

# Specific tests with output
cargo test test_config_loading -- --nocapture

# Tests with logs
RUST_LOG=debug cargo test
```

## 🤝 Contributing

1. **Fork** the project
2. **Create** a branch for your feature

   ```bash
   git checkout -b feature/new-feature
   ```

3. **Commit** your changes

   ```bash
   git commit -am 'feat: add new feature'
   ```

4. **Push** to the branch

   ```bash
   git push origin feature/new-feature
   ```

5. **Open** a Pull Request

### Contribution Guidelines

- Follow commit conventions ([Conventional Commits](https://www.conventionalcommits.org/))
- Add tests for new features
- Keep code formatted with `cargo fmt`
- Run `cargo clippy` to check warnings

## 📋 Implementation Status

### ✅ Implemented Features

- ✅ **Advanced interactive menu** with automatic categorization
- ✅ **Multiple registries** with namespace support
- ✅ **Automatic dependency resolution**
- ✅ **Colorful interface** with status indicators
- ✅ **Flexible configuration** of aliases and structure
- ✅ **Smart search** across all registries
- ✅ **Outdated component detection**
- ✅ **Full TypeScript support** with path resolution
- ✅ **Authenticated registries** (headers/params)
- ✅ **Placeholder processing** ($UTILS$, $COMPONENTS$, etc.)
- ✅ **Style support** (new-york, default)
- ✅ **@namespace/component format**
- ✅ **shadcn-svelte and shadcn/ui compatibility**
- ✅ **Automatic .js extension removal** in TypeScript

### 🚧 In Development

- 🚧 **Update command** (currently uses `add --force`)
- 🚧 **Remove command** (basic implementation)

### 📋 Future Roadmap

- [ ] Intelligent component caching
- [ ] Project template support
- [ ] Plugin system for extensions
- [ ] Web interface for management
- [ ] VS Code integration
- [ ] Native React/Vue registry support
- [ ] Configuration backup and restore
- [ ] Component versioning
- [ ] Visual update diff

## 🐛 Reporting Bugs

Found a bug? [Open an issue](https://github.com/your-username/uiget/issues) with:

- Detailed problem description
- Steps to reproduce
- uiget version (`uiget --version`)
- Operating system
- Configuration file (without sensitive data)

## 📄 License

This project is licensed under the [MIT License](LICENSE) - see the LICENSE file for details.

## 🙏 Acknowledgments

- [shadcn/ui](https://ui.shadcn.com/) for inspiration
- [shadcn-svelte](https://www.shadcn-svelte.com/) for schema reference
- Rust community for excellent libraries

---

<div align="center"> 
Made with ❤️ in Rust
</div>
