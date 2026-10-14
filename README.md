# EATMCP — Environment-Aware Terminal MCP

Servidor MCP (Rust) que dá a agentes de IA acesso a terminal, arquivos e informações do sistema **cientes do ambiente** (SO, shell, gerenciador de pacotes), evitando comandos incompatíveis como `systemctl` no Windows ou `winget` no Alpine.

## Requisitos

- Rust 1.75+ (edição 2021) e `cargo`

## Build

```bash
cargo build --release
# binário: target/release/eatmcp (eatmcp.exe no Windows)
```

## Execução

O servidor fala JSON-RPC via **stdio** (logs vão para stderr) e sobe também um dev server web (padrão `127.0.0.1:3001`).

```bash
./target/release/eatmcp                       # stdio + dev server em 127.0.0.1:3001
./target/release/eatmcp --port 4000           # porta customizada
./target/release/eatmcp --host 0.0.0.0        # host customizado
./target/release/eatmcp --web-only            # somente a UI web
./target/release/eatmcp --no-dev              # somente MCP, sem dev server
```

| Opção | Variável de ambiente | Descrição |
|---|---|---|
| `--port` / `--dev-port` | `EATMCP_DEV_PORT` | Porta do dev server |
| `--host` / `--dev-host` | `EATMCP_DEV_HOST` | Host do dev server (padrão `127.0.0.1`) |
| `--web-only` / `--ui-only` | — | Apenas a interface web |
| `--no-dev` | — | Desativa o dev server |
| `--show-configs` | — | Imprime os blocos de configuração para cada harness |
| `--install <alvo>` | — | Instala a configuração no harness (`claude-desktop`, `cursor`, `all`, ...) |

Nível de log: `RUST_LOG=debug ./target/release/eatmcp`.

## Instalação em clientes de IA

```bash
eatmcp --show-configs                       # ver configs geradas
eatmcp --install claude-desktop --port 4000 # instalar no Claude Desktop
eatmcp --install cursor                     # instalar no Cursor
eatmcp --install all                        # todos os harnesses detectados
```

Claude Code:

```bash
claude mcp add eatmcp -- /caminho/para/eatmcp --dev-port 3001
```

Configuração manual (JSON, ex.: Claude Desktop):

```json
{
  "mcpServers": {
    "eatmcp": {
      "command": "/caminho/para/target/release/eatmcp",
      "args": ["--dev-port", "3001"]
    }
  }
}
```

Mais detalhes por harness em [docs/UNIVERSAL_HARNESS_SETUP.md](docs/UNIVERSAL_HARNESS_SETUP.md).

## Ferramentas MCP

| Categoria | Ferramentas |
|---|---|
| Ambiente | `environment_inspect`, `environment_capabilities` |
| Comandos | `command_validate`, `command_execute` |
| Serviços/pacotes | `service_status`, `service_restart`, `package_install` |
| Sistema/rede | `process_list`, `network_port_check` |
| Arquivos | `filesystem_exists`, `file_read`, `file_edit` (diff unificado), `file_write`, `file_search` (glob + regex) |

Fluxo recomendado: chamar `environment_inspect` primeiro e depois `command_execute` com a linha de comando completa (suporta pipes e comandos compostos).

## Skill e instruções para agentes

- Skill universal em [skills/](skills) (Claude Code, Cursor, DSH, Roo Code).
- Para forçar o uso, adicione ao `AGENTS.md`/`.cursorrules` do projeto a regra de que todo comando de terminal deve passar pelo EATMCP.

## Testes

```bash
cargo test
```

## Documentação

- [docs/UNIVERSAL_HARNESS_SETUP.md](docs/UNIVERSAL_HARNESS_SETUP.md)
- [docs/TECHNICAL_EVIDENCE.md](docs/TECHNICAL_EVIDENCE.md)
- [docs/environment-aware-terminal-mcp-planejamento.md](docs/environment-aware-terminal-mcp-planejamento.md)
