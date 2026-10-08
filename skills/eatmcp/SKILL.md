---
name: eatmcp
description: Use when executing terminal commands, inspecting host environment, managing OS services, installing packages, checking TCP ports, or testing file existence. Directs the AI agent to operate deterministically through the EATMCP server rather than running blind shell commands.
---

# EATMCP: Protocolo de Interação com Terminal e Sistema Operacional

Você está equipado com o **EATMCP (Environment-Aware Terminal MCP)**.
Quando for necessário executar comandos de terminal, inspecionar o sistema ou gerenciar serviços, **SEMPRE utilize as ferramentas do EATMCP** (`command_execute`, `environment_inspect`, `service_status`, etc.) em vez de ferramentas de shell cego (`pwsh`, `bash`, `cmd`, `terminal`).

## Princípios de Operação

1. **Observar antes de executar (`environment_inspect`)**:
   - Antes de sugerir ou rodar comandos dependentes de ambiente, inspecione o fingerprint real (SO, shell, gerenciador de pacotes, init, privilégios).
   - Nunca adivinhe que o sistema é Linux ou Windows: leia o fingerprint do ambiente.

2. **Validar comandos mutativos (`command_validate`)**:
   - Para operações destrutivas ou administrativas, verifique o nível de risco (`LOW`, `MEDIUM`, `HIGH`, `CRITICAL`).
   - Comandos classificados como `CRITICAL` são bloqueados automaticamente pela política de segurança.

3. **Execução direta de processos (`command_execute`)**:
   - Prefira passar o programa (`command`) e os argumentos em lista (`args`), sem invocar shell intermediário por padrão.
   - Isso evita problemas de escape, shell injection e diferenças de sintaxe entre shells.

4. **Validação pós-execução (`service_status`, `network_port_check`, `filesystem_exists`)**:
   - Toda alteração deve ser validada. Se você reiniciou um serviço com `service_restart`, valide se ele ficou ativo.
   - Se levantou um servidor, valide a porta com `network_port_check`.

## Mapeamento de Ferramentas

| Intenção | Ferramenta EATMCP |
| :--- | :--- |
| Descobrir SO, Shell, Gerenciador de Pacotes | `environment_inspect` |
| Listar capacidades e comandos nativos | `environment_capabilities` |
| Validar risco de um comando | `command_validate` |
| Executar comando / processo | `command_execute` |
| Inspecionar ou reiniciar serviço | `service_status` / `service_restart` |
| Instalar pacote de software | `package_install` |
| Listar processos e uso de memória | `process_list` |
| Testar se porta TCP está aberta | `network_port_check` |
| Verificar se arquivo ou pasta existe | `filesystem_exists` |
| Ler arquivo de texto otimizado (offset, limit, anti-blowup) | `file_read` |
| Editar arquivo via substituição atômica de texto | `file_edit` |
| Escrever arquivo diretamente em disco (buffer atômico) | `file_write` |
| Buscar arquivos e texto ultra-denso (glob + grep com .gitignore) | `file_search` |
