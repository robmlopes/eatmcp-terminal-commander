# Environment-Aware Terminal MCP

## 1. Visão geral

### Nome provisório
**Environment-Aware Terminal MCP (EATMCP)**

### Objetivo

Criar um servidor MCP especializado em interação segura, determinística e eficiente com ambientes de terminal.

A intenção principal não é apenas fornecer ao LLM uma ferramenta para executar comandos. O projeto deve funcionar como uma **camada de inteligência operacional entre o agente de IA e o sistema operacional**, permitindo que o agente descubra o ambiente real, determine quais comandos e ferramentas são compatíveis, monte um plano de execução e valide o resultado.

O problema central que o projeto pretende resolver é:

> LLMs frequentemente sugerem comandos corretos em termos gerais, mas incompatíveis com o ambiente real onde serão executados.

Isso acontece por diferenças de:

- sistema operacional;
- distribuição Linux;
- versão;
- shell;
- gerenciador de pacotes;
- sistema de inicialização;
- permissões;
- usuário atual;
- Docker/Podman/WSL;
- Termux;
- ferramentas instaladas;
- arquitetura de CPU;
- caminhos;
- configuração local;
- versão dos próprios comandos.

O EATMCP deve reduzir drasticamente esse tipo de erro por meio de **observação real do ambiente + planejamento + execução controlada + validação**.

---

# 2. Intenção final do projeto

O objetivo final é que um agente possa expressar uma intenção de alto nível:

```text
"Quero reiniciar o SSH."
```

e o MCP determine:

```text
Sistema: Ubuntu 24.04
Shell: bash
Init: systemd
Usuário: ubuntu
sudo: disponível
Serviço: ssh.service
```

e então produza:

```text
sudo systemctl restart ssh
```

seguido da validação:

```text
sudo systemctl is-active ssh
```

O agente não deve precisar adivinhar qual comando usar.

Outro exemplo:

```text
"Instale o Redis."
```

O MCP deve descobrir:

```text
OS: Ubuntu
Package manager: apt
Redis: não instalado
sudo: disponível
```

e montar um plano compatível.

No Windows:

```text
OS: Windows
Shell: PowerShell
Package manager: winget
```

o plano poderá ser diferente.

No Termux:

```text
OS: Android
Environment: Termux
Package manager: pkg
Init/service model: diferente de systemd
```

e o plano deve ser adaptado novamente.

---

# 3. Princípios fundamentais

## 3.1 Detectar antes de executar

Nenhuma operação importante deve depender exclusivamente de suposições do LLM.

Fluxo:

```text
Intenção
   ↓
Detecção do ambiente
   ↓
Capacidades disponíveis
   ↓
Planejamento
   ↓
Validação do plano
   ↓
Execução
   ↓
Validação do resultado
```

## 3.2 Observação > inferência

Quando uma informação puder ser obtida diretamente do sistema, ela deve ser obtida do sistema.

Exemplo:

Não assumir:

```text
Ubuntu + systemd
```

Descobrir:

```text
/etc/os-release
ps -p 1
command -v systemctl
```

## 3.3 Comandos pequenos e verificáveis

Evitar inicialmente comandos gigantes como:

```bash
cmd1 && cmd2 && cmd3 && cmd4
```

Preferir operações independentes:

```text
diagnóstico
→ comando
→ validação
→ próximo comando
```

Isso facilita recuperação de erros.

## 3.4 Separar planejamento de execução

O MCP deve oferecer operações distintas para:

- analisar;
- planejar;
- executar;
- validar.

Isso permite que o agente ou usuário aprove um plano antes de operações sensíveis.

## 3.5 Segurança por padrão

O servidor deve assumir que qualquer comando pode causar dano.

Deve existir classificação de risco:

```text
LOW
MEDIUM
HIGH
CRITICAL
```

Exemplos:

```text
LOW
systemctl is-active ssh

MEDIUM
apt install redis

HIGH
rm -rf /algum/diretorio

CRITICAL
comandos destrutivos envolvendo discos, partições,
credenciais ou alteração ampla do sistema
```

---

# 4. Arquitetura proposta

## 4.1 Arquitetura geral

```text
                    ┌──────────────────────┐
                    │       LLM / Agent    │
                    └──────────┬───────────┘
                               │ MCP
                               ▼
                    ┌──────────────────────┐
                    │      EATMCP Server   │
                    ├──────────────────────┤
                    │ MCP Transport Layer   │
                    │ Request Router        │
                    │ Intent/Task Layer     │
                    │ Environment Engine    │
                    │ Capability Registry   │
                    │ Command Planner       │
                    │ Safety Engine         │
                    │ Execution Engine      │
                    │ Validation Engine     │
                    │ Result Formatter      │
                    └──────────┬───────────┘
                               │
             ┌─────────────────┼─────────────────┐
             ▼                 ▼                 ▼
          Linux             Windows           Termux
             │                 │                 │
           bash              PowerShell        shell
             │                 │                 │
           systemd             Win32           Android
           apt                 winget           pkg
           docker              docker           ...
```

---

# 5. Componentes

## 5.1 MCP Transport Layer

Responsável somente pela comunicação MCP.

Deve ser o mais fino possível.

Não deve conter lógica pesada.

Responsabilidades:

- receber requests;
- validar schema;
- encaminhar chamadas;
- retornar respostas;
- gerenciar sessões quando necessário.

Transporte prioritário:

1. stdio para agentes locais;
2. HTTP/SSE ou Streamable HTTP quando necessário para servidor remoto;
3. outros transportes somente quando houver necessidade real.

Para máxima performance em uso local, **stdio deve ser o caminho prioritário**.

---

# 6. Environment Engine

É o núcleo do projeto.

Responsável por descobrir o ambiente real.

Deve coletar:

## Sistema

- OS;
- distribuição;
- versão;
- kernel;
- arquitetura;
- hostname quando permitido;
- virtualização/containerização.

## Shell

- bash;
- zsh;
- sh;
- fish;
- PowerShell;
- CMD;
- outros.

## Runtime

- Python;
- PHP;
- Node;
- Bun;
- Java;
- Go;
- Rust;
- etc.

## Package managers

Linux:

- apt;
- dnf;
- yum;
- pacman;
- apk;
- zypper;
- pkg.

Windows:

- winget;
- chocolatey;
- scoop.

## Init/service managers

- systemd;
- OpenRC;
- SysV;
- launchd;
- Windows Services;
- Termux service model.

## Containers

Detectar:

- Docker;
- Podman;
- container atual;
- host/container;
- Docker Compose.

## Permissões

Detectar:

- usuário;
- UID;
- grupos;
- sudo;
- root;
- administrador Windows;
- permissões relevantes.

---

# 7. Environment Fingerprint

O resultado do diagnóstico deve ser convertido em um fingerprint estruturado.

Exemplo conceitual:

```json
{
  "platform": "linux",
  "distribution": "ubuntu",
  "version": "24.04",
  "architecture": "x86_64",
  "shell": "bash",
  "init": "systemd",
  "package_manager": "apt",
  "container": false,
  "docker": true,
  "user": "ubuntu",
  "sudo": true
}
```

O fingerprint deve possuir:

- versão;
- timestamp;
- capacidades;
- ferramentas detectadas;
- hash opcional do ambiente.

Isso permite cache.

---

# 8. Capability Registry

Em vez de o agente precisar conhecer milhares de comandos, o servidor deve trabalhar com **capacidades**.

Exemplos:

```text
service.restart
package.install
package.remove
file.read
file.write
process.list
network.inspect
port.check
docker.container.restart
user.inspect
disk.inspect
```

Cada capacidade possui implementações específicas por ambiente.

Exemplo:

```text
service.restart
```

Linux/systemd:

```text
systemctl restart <service>
```

Windows:

```text
Restart-Service <service>
```

Termux:

```text
Implementação específica
```

Essa abstração é uma das partes mais importantes do projeto.

---

# 9. Command Planner

O planner transforma:

```text
intenção
+
environment fingerprint
+
capabilities
+
restrições
```

em um plano executável.

Exemplo:

```text
Intent:
restart SSH

Environment:
Ubuntu 24.04
systemd
sudo

Plan:
1. Confirmar existência de ssh.service
2. Reiniciar serviço
3. Verificar estado
```

O planner deve evitar gerar comandos quando existir uma operação nativa mais segura.

Por exemplo, em vez de manipular arquivos manualmente para reiniciar um serviço, utilizar a API/ferramenta apropriada do sistema.

---

# 10. Safety Engine

O Safety Engine deve analisar o comando antes da execução.

Critérios:

- destrutividade;
- privilégio;
- alteração de arquivos;
- alteração de rede;
- alteração de usuários;
- alteração de firewall;
- alteração de disco;
- execução remota;
- acesso a credenciais;
- shell injection;
- comandos encadeados;
- redirecionamentos;
- substituição de comandos;
- globbing perigoso.

O servidor deve separar:

```text
SAFE
```

de:

```text
REQUIRES_CONFIRMATION
```

e:

```text
BLOCKED
```

---

# 11. Execution Engine

O Execution Engine deve ser pequeno e determinístico.

Responsabilidades:

1. preparar processo;
2. definir working directory;
3. configurar environment;
4. executar;
5. capturar stdout;
6. capturar stderr;
7. capturar exit code;
8. medir duração;
9. aplicar timeout;
10. encerrar processo quando necessário.

Nunca depender de texto produzido pelo LLM para controlar diretamente detalhes críticos sem validação.

---

# 12. Validation Engine

Toda operação importante deve possuir uma estratégia de validação.

Exemplo:

```text
AÇÃO:
reiniciar SSH

VALIDAÇÃO:
serviço está ativo?
porta está ouvindo?
processo existe?
```

O resultado deve indicar:

```text
SUCCESS
PARTIAL_SUCCESS
FAILED
UNKNOWN
```

Isso evita o padrão ruim:

```text
"Executei o comando, então funcionou."
```

---

# 13. Error Recovery

Quando uma operação falhar:

```text
comando
   ↓
erro
   ↓
classificação do erro
   ↓
diagnóstico
   ↓
novo plano
```

O MCP deve devolver ao agente informações estruturadas:

```text
exit_code
stdout
stderr
error_class
likely_cause
suggested_next_action
```

Exemplo:

```text
error_class:
COMMAND_NOT_FOUND

likely_cause:
systemctl não está instalado ou o ambiente não utiliza systemd.

suggested_next_action:
detectar init/service manager novamente.
```

O agente não precisa simplesmente "chutar outro comando".

---

# 14. Performance

A arquitetura deve priorizar:

## 14.1 Processo persistente

Evitar iniciar o servidor MCP novamente para cada comando.

Preferir:

```text
Agent
  │
  │ conexão persistente
  ▼
EATMCP
  │
  ├── diagnóstico em cache
  ├── planner
  ├── executor
  └── validator
```

## 14.2 Cache de ambiente

O diagnóstico completo não deve ser executado toda vez.

Usar:

```text
Environment Fingerprint Cache
```

com invalidação quando:

- shell muda;
- usuário muda;
- container muda;
- ferramentas relevantes são instaladas/removidas;
- processo de init muda;
- sessão é recriada.

## 14.3 Detecção paralela

Informações independentes podem ser descobertas em paralelo.

Exemplo:

```text
OS
Shell
Architecture
Docker
Package managers
Init
```

podem ser detectados simultaneamente quando não houver dependências.

## 14.4 Evitar LLM no caminho crítico

O servidor não deve chamar um LLM para cada comando.

Preferir:

```text
regras determinísticas
+
capability registry
+
detecção real
+
planner
```

O LLM deve fornecer a intenção e interpretar resultados complexos quando necessário.

---

# 15. Linguagem e runtime

## Recomendação principal

**Rust** é a opção preferencial para o servidor final quando a prioridade máxima for:

- performance;
- baixo consumo de memória;
- concorrência;
- inicialização rápida;
- execução de processos;
- controle de recursos;
- distribuição como binário único.

Arquitetura:

```text
Rust
 ├── MCP server
 ├── environment detection
 ├── capability registry
 ├── safety engine
 ├── execution engine
 ├── validation
 └── cache
```

## Alternativa

**Go** é uma excelente alternativa caso a prioridade seja:

- velocidade de desenvolvimento;
- simplicidade;
- excelente concorrência;
- distribuição simples;
- ecossistema de servidores.

## Prototipação

**Python** pode ser utilizado para validar a arquitetura e os conceitos.

Entretanto, para o servidor final com foco em máxima performance, a recomendação é:

```text
Prototype: Python
Production: Rust
```

---

# 16. Modelo de processos

O servidor não deve executar tudo diretamente em um único fluxo bloqueante.

Modelo:

```text
MCP Session
     │
     ▼
Task Router
     │
     ├── Environment
     ├── Planner
     ├── Safety
     ├── Executor
     └── Validator
```

Para comandos externos:

```text
EATMCP
   │
   ├── spawn
   ├── stdin
   ├── stdout
   ├── stderr
   └── timeout
```

Evitar criar processos auxiliares desnecessários.

---

# 17. Armazenamento

O servidor deve funcionar inicialmente **sem banco de dados**.

Estado temporário:

```text
memory
```

Cache persistente opcional:

```text
SQLite
```

SQLite pode armazenar:

- fingerprints;
- histórico de operações;
- métricas;
- erros;
- capabilities detectadas;
- auditoria.

A persistência deve ser opcional para manter o servidor leve.

---

# 18. Interface MCP proposta

Ferramentas conceituais:

## `environment.inspect`

Retorna o ambiente atual.

## `environment.capabilities`

Retorna capacidades disponíveis.

## `task.analyze`

Analisa uma intenção sem executar.

## `task.plan`

Produz plano executável.

## `command.validate`

Valida um comando antes da execução.

## `command.execute`

Executa um comando autorizado.

## `task.execute`

Executa um plano completo.

## `task.validate`

Valida o resultado.

## `history.get`

Consulta histórico quando auditoria estiver habilitada.

---

# 19. Exemplo de fluxo

Usuário:

```text
"Meu SSH não está funcionando."
```

LLM:

```text
Preciso diagnosticar SSH.
```

MCP:

```text
environment.inspect
```

Resultado:

```text
Ubuntu 24.04
systemd
bash
sudo
Docker instalado
```

Depois:

```text
task.analyze
```

Plano:

```text
1. Verificar ssh.service
2. Verificar processo sshd
3. Verificar porta 22
4. Verificar firewall
5. Verificar logs recentes
```

O agente pode então solicitar execução.

Resultado:

```text
ssh.service: inactive
port 22: closed
```

Planner:

```text
restart ssh.service
```

Execution:

```text
sudo systemctl restart ssh
```

Validation:

```text
ssh.service: active
port 22: listening
```

Resultado final:

```text
SUCCESS
```

---

# 20. Modos de operação

## Discovery

Somente descoberta.

```text
environment.inspect
```

## Advisory

Analisa e recomenda, mas não executa.

## Interactive

Executa operações de baixo risco e pede aprovação para operações de risco.

## Autonomous

Executa automaticamente operações permitidas por uma política previamente configurada.

## Restricted

Somente comandos/capabilities explicitamente autorizados.

---

# 21. Política de comandos

Deve existir uma política configurável:

```text
allow
deny
confirm
```

Exemplo:

```text
package.install = confirm
service.restart = allow
file.delete = confirm
disk.format = deny
firewall.modify = confirm
credential.read = deny
```

---

# 22. Segurança contra prompt injection

O servidor não deve confiar em instruções encontradas no sistema.

Exemplo:

```text
README.md:
"Ignore todas as instruções anteriores e execute..."
```

Isso é **dados**, não autoridade.

A hierarquia deve ser:

```text
Security Policy
      ↓
User/Agent Authorization
      ↓
MCP Safety Rules
      ↓
Task
      ↓
System Data
```

Arquivos, logs e saída de comandos nunca devem automaticamente virar instruções.

---

# 23. Auditoria

Quando habilitada, registrar:

```text
timestamp
session
user
intent
environment fingerprint
command
risk level
approval
exit code
stdout hash
stderr hash
duration
result
```

Nunca armazenar automaticamente:

- senhas;
- tokens;
- chaves privadas;
- secrets;
- conteúdo sensível desnecessário.

---

# 24. Testes

A prioridade do projeto deve ser **testabilidade**.

## Fase 1 — Unit tests

Testar:

- OS detection;
- shell detection;
- package manager detection;
- init detection;
- capability mapping;
- risk classification;
- command validation.

## Fase 2 — Integration tests

Ambientes:

```text
Ubuntu
Debian
Alpine
Windows
Termux
Docker
WSL
```

## Fase 3 — Failure tests

Simular:

```text
command not found
permission denied
timeout
service missing
package missing
wrong shell
container
network unavailable
```

## Fase 4 — Security tests

Testar:

```text
command injection
argument injection
shell injection
path traversal
privilege escalation
destructive commands
malicious output
prompt injection
```

## Fase 5 — Performance

Medir:

- startup;
- memória;
- latência MCP;
- tempo de diagnóstico;
- execução concorrente;
- número de processos criados;
- throughput.

---

# 25. Estratégia de desenvolvimento

## Fase 0 — Especificação

Definir:

- MCP transport;
- schemas;
- capability model;
- environment fingerprint;
- error model;
- security model.

## Fase 1 — Environment Inspector

Implementar:

```text
OS
Shell
Architecture
User
Privileges
Package manager
Init
Container
Installed tools
```

Sem execução arbitrária.

## Fase 2 — Capability Registry

Implementar capacidades:

```text
service
package
process
network
filesystem
docker
```

## Fase 3 — Execution Engine

Implementar:

- process spawning;
- stdout/stderr;
- exit codes;
- timeout;
- cancellation.

## Fase 4 — Safety Engine

Implementar:

- risk classification;
- allow/deny;
- confirmation;
- command validation.

## Fase 5 — Planner

Transformar intenções em planos.

## Fase 6 — Validation

Adicionar pós-condições.

## Fase 7 — Recovery

Implementar diagnóstico automático de falhas.

## Fase 8 — Performance

Adicionar:

- cache;
- paralelismo;
- otimização de processos;
- métricas.

## Fase 9 — Security hardening

Auditoria e testes adversariais.

## Fase 10 — Release

Gerar binários:

```text
Linux x86_64
Linux ARM64
Windows x86_64
Windows ARM64
```

---

# 26. Arquitetura de código recomendada

Estrutura conceitual:

```text
src/
├── main
├── mcp/
│   ├── transport
│   ├── protocol
│   └── tools
│
├── environment/
│   ├── detector
│   ├── fingerprint
│   └── capabilities
│
├── planner/
│   ├── intent
│   ├── planner
│   └── plans
│
├── execution/
│   ├── process
│   ├── command
│   ├── timeout
│   └── cancellation
│
├── safety/
│   ├── policy
│   ├── classifier
│   └── validator
│
├── validation/
│   ├── checks
│   └── result
│
├── recovery/
│   ├── errors
│   └── strategies
│
├── storage/
│   └── sqlite
│
└── telemetry/
    └── metrics
```

A implementação real deve manter módulos independentes para permitir testes e evolução.

---

# 27. Decisão arquitetural importante

O projeto **não deve ser apenas um "MCP shell"**.

Um MCP shell tradicional:

```text
LLM → comando → terminal
```

O EATMCP deve ser:

```text
LLM
 ↓
Intenção
 ↓
EATMCP
 ↓
Environment
 ↓
Capabilities
 ↓
Plan
 ↓
Safety
 ↓
Execution
 ↓
Validation
 ↓
Result
 ↓
LLM
```

Essa é a principal diferenciação do projeto.

---

# 28. Objetivo de performance

Metas iniciais:

- baixo consumo de memória;
- processo persistente;
- zero dependência de banco por padrão;
- zero chamada de LLM interna por comando;
- diagnóstico cacheado;
- operações independentes executadas em paralelo quando possível;
- execução externa somente quando necessária;
- comunicação MCP eficiente;
- binário nativo para produção.

O gargalo normalmente não será o MCP ou Rust.

O maior custo será:

```text
spawn de processos
+
execução do comando
+
I/O do sistema
```

Portanto a otimização deve se concentrar principalmente em:

- reduzir processos desnecessários;
- reutilizar estado;
- evitar diagnósticos repetidos;
- evitar shell intermediário quando não for necessário;
- usar execução direta de processos sempre que possível.

---

# 29. Regra de ouro do executor

Quando possível:

```text
executar programa diretamente
```

em vez de:

```text
shell → parser → programa
```

Exemplo conceitual:

```text
systemctl
restart
ssh
```

é preferível a construir uma string arbitrária e passá-la para:

```text
bash -c "..."
```

Isso melhora:

- segurança;
- previsibilidade;
- performance;
- tratamento de argumentos.

Shell deve ser usado somente quando a operação realmente exige recursos do shell.

---

# 30. Resultado esperado

Ao final do projeto, um agente deverá conseguir dizer:

```text
"Configure o Redis."
```

sem precisar saber antecipadamente:

- qual sistema operacional está rodando;
- qual shell existe;
- qual package manager está disponível;
- se existe systemd;
- se está dentro de Docker;
- quais permissões possui;
- quais comandos estão instalados.

O EATMCP deverá transformar a intenção em uma sequência operacional verificável.

---

# 31. Critério de sucesso

O projeto será considerado bem-sucedido quando:

1. detectar corretamente os principais ambientes suportados;
2. identificar capacidades disponíveis;
3. escolher comandos compatíveis;
4. evitar comandos incompatíveis;
5. validar comandos antes da execução;
6. validar resultados após a execução;
7. recuperar-se de erros previsíveis;
8. impedir operações perigosas sem autorização;
9. funcionar com diferentes agentes MCP;
10. manter baixo consumo de recursos;
11. apresentar baixa latência;
12. possuir cobertura de testes significativa.

---

# 32. Visão futura

Depois do núcleo estar estável, o projeto poderá evoluir para:

- gerenciamento remoto de servidores;
- múltiplos hosts;
- inventário de máquinas;
- execução distribuída;
- políticas por máquina;
- sandbox;
- snapshots antes de alterações;
- rollback;
- comparação de ambientes;
- diagnóstico automático;
- observabilidade;
- integração com CI/CD;
- integração com Docker/Kubernetes;
- execução de tarefas longas;
- agentes especializados;
- marketplace de capabilities.

Uma evolução particularmente importante seria:

```text
Agent
   ↓
EATMCP Controller
   ↓
┌──────────────┬──────────────┬──────────────┐
│ Host Linux   │ Windows      │ Termux       │
│ EATMCP Agent │ EATMCP Agent │ EATMCP Agent │
└──────────────┴──────────────┴──────────────┘
```

Assim, o mesmo agente poderia administrar ambientes completamente diferentes sem precisar conhecer previamente as peculiaridades de cada sistema.

---

# 33. Resumo da arquitetura final recomendada

```text
                 ┌───────────────────────┐
                 │       AI AGENT        │
                 └───────────┬───────────┘
                             │ MCP
                             ▼
                 ┌───────────────────────┐
                 │       EATMCP          │
                 │                       │
                 │  Request Router       │
                 │       ↓               │
                 │  Environment Engine   │
                 │       ↓               │
                 │  Capability Registry  │
                 │       ↓               │
                 │  Command Planner      │
                 │       ↓               │
                 │  Safety Engine        │
                 │       ↓               │
                 │  Execution Engine     │
                 │       ↓               │
                 │  Validation Engine    │
                 │       ↓               │
                 │  Recovery Engine      │
                 └───────────┬───────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
           Linux          Windows         Termux
```

### Stack recomendada

```text
Produção:
Rust

Protocolo:
MCP

Transporte local:
stdio

Transporte remoto:
Streamable HTTP

Cache:
Memória

Persistência opcional:
SQLite

Execução:
process spawning direto

Arquitetura:
modular + event/request driven

Concorrência:
async quando apropriado

Segurança:
allow / confirm / deny

Observabilidade:
logs + métricas opcionais
```

---

# 34. Primeira versão (MVP)

O MVP deve ser propositalmente pequeno.

### Suportar inicialmente

- Ubuntu/Debian;
- Windows;
- Termux;
- Bash;
- PowerShell;
- CMD;
- apt;
- winget;
- pkg;
- systemd;
- Docker.

### Capabilities iniciais

```text
environment.inspect
environment.capabilities
service.status
service.start
service.stop
service.restart
package.install
package.remove
process.list
network.port_check
filesystem.exists
command.validate
command.execute
```

### Não implementar inicialmente

- Kubernetes;
- execução remota;
- rollback;
- agentes distribuídos;
- marketplace;
- IA interna;
- interface gráfica;
- plugins complexos.

Primeiro provar:

> **O EATMCP consegue receber uma intenção, descobrir corretamente o ambiente, escolher uma operação compatível, executar com segurança e comprovar o resultado.**

Essa deve ser a fundação de todo o projeto.
