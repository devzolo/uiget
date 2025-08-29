# uiget 🚀

Uma ferramenta CLI em Rust moderna e eficiente para gerenciar componentes shadcn/ui de múltiplos registries com interface interativa avançada.

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![MIT License](https://img.shields.io/badge/License-MIT-green.svg?style=for-the-badge)](https://choosealicense.com/licenses/mit/)

## ✨ Características

- 🎯 **Menu interativo avançado** com seleção por categoria
- 📦 **Suporte a múltiplos registries** por namespace
- 🔄 **Resolução automática de dependências**
- ⚡ **Interface colorida e intuitiva**
- 🛠️ **Configuração flexível** de aliases e estrutura de projeto
- 🔍 **Busca inteligente** de componentes
- 📊 **Detecção de componentes desatualizados**
- ✅ **Compatível com schema shadcn-svelte**

## 📦 Instalação

### Via Cargo (Recomendado)

```bash
# Instalar diretamente do repositório
cargo install --git https://github.com/seu-usuario/uiget

# Ou instalar localmente após clonar
git clone https://github.com/seu-usuario/uiget
cd uiget
cargo install --path .
```

### Compilação Manual

```bash
# Clone o repositório
git clone https://github.com/seu-usuario/uiget
cd uiget

# Compile e instale
cargo build --release
cargo install --path .
```

### Verificação da Instalação

```bash
# Verificar se o comando uiget está disponível
uiget --help

# Versão instalada
uiget --version
```

## 🚀 Início Rápido

### 1. Inicializar Configuração

```bash
# Criar arquivo de configuração padrão
uiget init

# Com opções personalizadas
uiget init --base-color blue --css "src/styles.css"
```

### 2. Menu Interativo

```bash
# Abrir menu interativo para seleção de componentes
uiget add
```

### 3. Adicionar Componentes Específicos

```bash
# Adicionar um componente específico
uiget add button

# Adicionar de um registry específico
uiget add button --registry custom
```

## 🎯 Menu Interativo Avançado

O `uiget` oferece uma interface interativa moderna que permite seleção eficiente de componentes:

### Seleção por Categoria

Execute `uiget add` para abrir o menu principal:

```bash
? What would you like to do?
❯ 🔍 Browse and select individual components
  📦 Select ALL UI Components (52 items)     ← Seleciona TODOS instantaneamente
  🧩 Select ALL Blocks (131 items)          ← Seleciona TODOS instantaneamente  
  🪝 Select ALL Hooks (1 items)             ← Seleciona TODOS instantaneamente
  📚 Select ALL Libraries (1 items)         ← Seleciona TODOS instantaneamente
  ❌ Cancel
```

### Como Usar

1. **↑↓** - Navegar entre opções
2. **Enter** - Selecionar categoria completa ou abrir browser individual
3. **Space** - Marcar/desmarcar componentes individuais
4. **Enter final** - Confirmar e instalar seleção

### Preview e Confirmação

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

## 📋 Comandos Disponíveis

### Configuração Inicial

```bash
# Inicializar projeto
uiget init [--force] [--base-color COLOR] [--css PATH]

# Exemplo com opções personalizadas
uiget init --base-color emerald --css "src/styles/globals.css"
```

### Gerenciamento de Registries

```bash
# Adicionar novo registry
uiget registry add nome-registry https://meu-registry.com

# Listar registries configurados
uiget registry list

# Testar conexão com registry
uiget registry test nome-registry

# Remover registry
uiget registry remove nome-registry
```

### Componentes

```bash
# Menu interativo
uiget add

# Adicionar componente específico
uiget add button

# Adicionar de registry específico
uiget add button --registry custom

# Adicionar forçando sobrescrita
uiget add button --force

# Adicionar sem dependências
uiget add button --skip-deps

# Buscar componentes
uiget search "data table"

# Listar todos os componentes disponíveis
uiget list

# Listar de registry específico
uiget list --registry custom

# Informações detalhadas do componente
uiget info button

# Remover componente
uiget remove button

# Verificar componentes desatualizados
uiget outdated
```

## ⚙️ Configuração

O arquivo `uiget.json` é criado no diretório do projeto com a seguinte estrutura:

```json
{
  "$schema": "https://shadcn-svelte.com/schema.json",
  "tailwind": {
    "css": "src/app.css",
    "baseColor": "slate"
  },
  "aliases": {
    "components": "$lib/components",
    "utils": "$lib/utils",
    "ui": "$lib/components/ui",
    "hooks": "$lib/hooks",
    "lib": "$lib"
  },
  "registries": {
    "default": "https://shadcn-svelte.com",
    "custom": "https://meu-registry-personalizado.com"
  },
  "typescript": true
}
```

### Principais Diferenças do Schema Original

- `registry` (string) → `registries` (object): Suporte a múltiplos registries
- Namespace "default" usado como fallback
- Aliases flexíveis para diferentes estruturas de projeto

## 🏗️ Estrutura do Projeto

```tree
src/
├── main.rs          # Ponto de entrada principal
├── cli.rs           # Definições de comandos CLI
├── config.rs        # Estruturas de configuração
├── registry.rs      # Cliente para registries
└── installer.rs     # Lógica de instalação de componentes
```

## 📚 Exemplos Práticos

### Configuração Multi-Registry

```bash
# Adicionar diferentes registries
uiget registry add shadcn-vue https://shadcn-vue.com
uiget registry add meus-componentes https://meus-componentes.dev

# Instalar de registries específicos
uiget add button --registry shadcn-vue
uiget add custom-card --registry meus-componentes
```

### Busca Avançada

```bash
# Buscar em todos os registries
uiget search "form"

# Buscar em registry específico
uiget search "table" --registry shadcn-vue

# Ver informações detalhadas
uiget info data-table
```

### Workflow Típico

```bash
# 1. Inicializar projeto
uiget init --base-color violet

# 2. Adicionar registry personalizado
uiget registry add empresa https://components.empresa.com

# 3. Instalar componentes essenciais via menu
uiget add

# 4. Verificar status
uiget list
uiget outdated
```

## 🔧 API do Registry

Os registries devem implementar a seguinte estrutura:

### Endpoints

```bashl
GET /registry/index.json              # Lista de componentes
GET /registry/components/{name}.json  # Detalhes do componente
```

### Formato do Componente

```json
{
  "name": "button",
  "description": "Um componente de botão customizável",
  "dependencies": ["cn", "lucide-svelte"],
  "registryDependencies": ["utils"],
  "files": [
    {
      "name": "button.svelte",
      "path": "$lib/components/ui/button.svelte",
      "content": "<!-- conteúdo do componente -->",
      "type": "component"
    }
  ],
  "type": "ui"
}
```

## 🧪 Desenvolvimento

### Pré-requisitos

- Rust 1.70+
- Cargo

### Comandos de Desenvolvimento

```bash
# Executar em modo debug
cargo run -- --help

# Executar testes
cargo test

# Executar com logs debug
RUST_LOG=debug cargo run -- --verbose list

# Verificar linting
cargo clippy

# Formatar código
cargo fmt

# Build otimizado
cargo build --release
```

### Estrutura de Testes

```bash
# Executar todos os testes
cargo test

# Testes específicos com output
cargo test test_config_loading -- --nocapture

# Testes com logs
RUST_LOG=debug cargo test
```

## 🤝 Contribuindo

1. **Fork** o projeto
2. **Crie** uma branch para sua feature

   ```bash
   git checkout -b feature/nova-funcionalidade
   ```

3. **Commit** suas mudanças

   ```bash
   git commit -am 'feat: adiciona nova funcionalidade'
   ```

4. **Push** para a branch

   ```bash
   git push origin feature/nova-funcionalidade
   ```

5. **Abra** um Pull Request

### Guidelines de Contribuição

- Siga as convenções de commit ([Conventional Commits](https://www.conventionalcommits.org/))
- Adicione testes para novas funcionalidades
- Mantenha o código formatado com `cargo fmt`
- Execute `cargo clippy` para verificar warnings

## 📋 Roadmap

- [ ] Cache inteligente de componentes
- [ ] Suporte a templates de projeto
- [ ] Plugin system para extensões
- [ ] Interface web para gerenciamento
- [ ] Integração com VS Code
- [ ] Suporte a React/Vue registries

## 🐛 Relatando Bugs

Encontrou um bug? [Abra uma issue](https://github.com/seu-usuario/uiget/issues) com:

- Descrição detalhada do problema
- Passos para reproduzir
- Versão do uiget (`uiget --version`)
- Sistema operacional
- Arquivo de configuração (sem dados sensíveis)

## 📄 Licença

Este projeto está licenciado sob a [Licença MIT](LICENSE) - veja o arquivo LICENSE para detalhes.

## 🙏 Agradecimentos

- [shadcn/ui](https://ui.shadcn.com/) pela inspiração
- [shadcn-svelte](https://www.shadcn-svelte.com/) pelo schema de referência
- Comunidade Rust pelas excelentes bibliotecas

---

<div align="center">
  Feito com ❤️ em Rust
</div>
