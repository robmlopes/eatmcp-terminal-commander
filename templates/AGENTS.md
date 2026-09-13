# Diretrizes para Agentes de IA: Terminal e Sistema Operacional

## Uso Obrigatório do EATMCP
Neste projeto e ambiente, toda interação de terminal, inspeção do sistema operacional e gerenciamento de serviços deve ser realizada através do servidor **EATMCP** (`command_execute`, `environment_inspect`, etc.):

1. **Nunca use ferramentas genéricas de shell (`pwsh`, `bash`)** quando as ferramentas `mcp__eatmcp__*` ou `eatmcp_*` estiverem disponíveis.
2. **Descubra o ambiente**: Execute `environment_inspect` antes de rodar comandos específicos de SO para determinar distribuição, shell e gerenciador de pacotes.
3. **Execute com segurança**: Use `command_execute` informando `command` e lista `args`.
4. **Valide os resultados**: Use `service_status`, `network_port_check` ou `filesystem_exists` para certificar-se do sucesso de operações críticas.
