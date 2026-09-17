# EATMCP: Guia de Instalação e Ativação Universal

Este guia descreve como ativar o **EATMCP (Environment-Aware Terminal MCP)** em qualquer harness ou cliente de IA do mercado.

---

## 1. O Problema da Adivinhação de Comandos

Modelos de linguagem frequentemente tentam rodar comandos incompatíveis (ex.: `systemctl` no Windows, `winget` no Alpine Linux, ou flags incorretas de shell). Além disso, quando ferramentas de shell cego (`bash`, `pwsh`, `terminal`) estão presentes, os modelos tendem a ignorar servidores MCP especializados.

O EATMCP resolve isso em três níveis:
1. **Protocolo MCP Assertivo**: Descrições de ferramentas padronizadas em `snake_case` com diretivas claras de prioridade.
2. **Skill Universal**: `skills/eatmcp/SKILL.md` (compatível com Claude Code, Cursor, DSH, Roo Code).
3. **Instruções de Projeto**: Arquivo `AGENTS.md` ou `.cursorrules` na raiz do repositório.

---

## 2. Instalação Automática via CLI

O binário `eatmcp.exe` possui instalador embutido:

```bash
# Exibir os blocos de configuração padrão (127.0.0.1:3001)
eatmcp.exe --show-configs

# Exibir os blocos customizados para outra porta e IP (ex: porta 4000)
eatmcp.exe --show-configs --port 4000 --host 0.0.0.0

# Instalar automaticamente no Claude Desktop com porta customizada
eatmcp.exe --install claude-desktop --port 4000

# Instalar no Cursor com porta customizada
eatmcp.exe --install cursor --port 4000

# Instalar em todos os harnesses detectados na máquina
eatmcp.exe --install all --port 4000
```

---

## 3. Configuração Manual por Harness

### 3.1 Claude Desktop
Arquivo de configuração:
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Linux: `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "eatmcp": {
      "command": "D:\\Projetos\\mcp-terminal-commander\\target\\release\\eatmcp.exe",
      "args": ["--dev-port", "3001"]
    }
  }
}
```

### 3.2 Claude Code (CLI da Anthropic)
Execute no terminal:
```bash
claude mcp add eatmcp -- D:\Projetos\mcp-terminal-commander\target\release\eatmcp.exe --dev-port 3001
```

### 3.3 Cursor
No Cursor, acesse **Settings > Features > MCP > Add New MCP Server**, ou crie `.cursor/mcp.json` na raiz do seu projeto:
```json
{
  "mcpServers": {
    "eatmcp": {
      "command": "D:\\Projetos\\mcp-terminal-commander\\target\\release\\eatmcp.exe",
      "args": ["--dev-port", "3001"]
    }
  }
}
```

### 3.4 Windsurf (Codeium Cascade)
Arquivo de configuração: `~/.codeium/windsurf/mcp_config.json`:
```json
{
  "mcpServers": {
    "eatmcp": {
      "command": "D:\\Projetos\\mcp-terminal-commander\\target\\release\\eatmcp.exe",
      "args": ["--dev-port", "3001"]
    }
  }
}
```

### 3.5 Google Antigravity / Antigravity IDE
Arquivo de configuração global do Antigravity:
- Windows: `%USERPROFILE%\.gemini\config\mcp_config.json` e `%USERPROFILE%\.gemini\antigravity-ide\mcp_config.json`
- Linux/macOS: `~/.gemini/config/mcp_config.json`
- Ou no projeto: `.gemini/mcp_config.json`

```json
{
  "mcpServers": {
    "eatmcp": {
      "command": "D:\\Projetos\\mcp-terminal-commander\\target\\release\\eatmcp.exe",
      "args": ["--dev-port", "3001"]
    }
  }
}
```

Também é possível instalar com um comando direto pelo CLI:
```powershell
target\release\eatmcp.exe --install antigravity
```

### 3.6 DeepSeek Harness (DSH Web)
O plugin já vem pré-configurado no preset do harness em `agent.cordis.yml`:
```yaml
- id: mcp-eatmcp
  name: '@deepseek-ai/dsh-mcp-client'
  config:
    transport: stdio
    serverName: eatmcp
    command: 'D:\Projetos\mcp-terminal-commander\target\release\eatmcp.exe'
    args: ['--dev-port', '3001']
```
E a skill universal instalada em `~/.dsh/skills/eatmcp/SKILL.md`.

---

## 4. Garantindo a Ativação pelo Agente

Para garantir que o agente sempre escolha o EATMCP em vez de tentar adivinhar comandos:

1. **Adicione as diretivas no projeto**:
   Copie `templates/AGENTS.md` para a raiz de projetos onde for trabalhar. Ele orienta o agente a:
   - Inspecionar o ambiente (`environment_inspect`) antes de gerar comandos.
   - Validar riscos com `command_validate`.
   - Executar com `command_execute`.
   - Confirmar o resultado (`service_status`, `network_port_check`, `filesystem_exists`).

2. **Ativação por MCP Prompts**:
   Em clientes compatíveis, utilize o prompt:
   `/eatmcp_terminal_session`
   que injeta as regras de terminal determinístico diretamente no contexto do modelo.
