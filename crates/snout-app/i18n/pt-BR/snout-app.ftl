# SnoutCode's interface in Brazilian Portuguese. English (i18n/en) is the source: same ids, same
# arguments. Say it the way a developer in this language would, not word for word.

## Languages

language-system = Sistema ({ $language })

## Light or dark

theme-system = Sistema
theme-light = Claro
theme-dark = Escuro

## Settings

settings-nothing-matches = Nada encontrado
settings-group-build-tools = Ferramentas de build
settings-group-frameworks = Frameworks
settings-group-tools = Ferramentas
settings-page-appearance = Aparência
settings-page-keymap = Atalhos de teclado
settings-page-editor = Editor
settings-page-version-control = Controle de versão
settings-page-terminal = Terminal
settings-page-agents = Agentes
settings-blurb-appearance = O idioma, as cores, o modo claro ou escuro e as fontes da janela e do editor.
settings-blurb-keymap = As teclas de cada comando. Adicione, remova ou volte aos padrões.
settings-blurb-editor = Como os arquivos são indentados, quebrados e exibidos enquanto você digita.
settings-blurb-version-control = Como o SnoutCode mantém o Git dos seus projetos atualizado.
settings-blurb-gradle = Quanto tempo os daemons do Gradle do SnoutCode ficam sem um build.
settings-blurb-spring = Os perfis com os quais a janela Spring avalia os beans.
settings-blurb-terminal = A aparência dos terminais, o que eles executam e o aplicativo de terminal para abrir pastas.
settings-blurb-agents = Os agentes de código que o SnoutCode acompanha e o que o console dele executa.

settings-section-language = Idioma
settings-language = Idioma da interface
settings-section-theme = Tema
settings-colors = Cores
settings-light-or-dark = Claro ou escuro
settings-dark-only = { $name } não tem versão clara: continua escuro, seja qual for este ajuste.
settings-section-interface = Interface
settings-font = Fonte
settings-size = Tamanho
settings-section-editor = Editor
settings-section-quitting = Encerramento
settings-confirm-quit = Confirmar antes de encerrar
settings-confirm-quit-note = O SnoutCode sempre pergunta quando encerrar faria perder alterações não salvas ou pararia algo em execução.
settings-section-updates = Atualizações
settings-update-check = Verificar atualizações automaticamente
settings-update-check-note = Uma nova versão é baixada em segundo plano e instalada quando você reinicia ou encerra. Em Ajuda você pode verificar atualizações a qualquer momento.

## The menu bar

menu-run = Executar
menu-snoutcode = SnoutCode
menu-services = Serviços
menu-file = Arquivo
menu-open-recent = Abrir recente
menu-edit = Editar
menu-view = Exibir
menu-git = Git
menu-tool-window-actions = Ações das janelas de ferramentas
menu-appearance = Aparência
menu-code = Código
menu-navigate = Navegar
menu-refactor = Refatorar
menu-build = Compilar
menu-tools = Ferramentas
menu-tool-settings = Ajustes das ferramentas
menu-window = Janela
menu-help = Ajuda

## Commands: the menu items and the command palette

cmd-new-project = Novo projeto…
cmd-open-project = Abrir projeto…
cmd-close-project = Fechar projeto
cmd-connect-to-ssh-host-or-wsl = Conectar a um host SSH ou WSL…
cmd-connect-to-ssh-host = Conectar a um host SSH…
cmd-close-remote-connection = Fechar conexão remota
cmd-clear-recent-projects = Limpar projetos recentes
cmd-search-everywhere = Pesquisar em todo lugar…
cmd-settings = Configurações…
cmd-open-settings-json = Abrir configurações (JSON)
cmd-open-project-settings-json = Abrir configurações do projeto (JSON)
cmd-go-to-file = Ir para arquivo…
cmd-recent-files = Arquivos recentes…
cmd-new-file = Novo arquivo…
cmd-go-to-class = Ir para classe…
cmd-go-to-line = Ir para linha…
cmd-find-in-files = Localizar nos arquivos…
cmd-replace-in-files = Substituir nos arquivos…
cmd-undo-replace-in-files = Desfazer substituição nos arquivos
cmd-find-in-file = Localizar no arquivo
cmd-replace-in-file = Substituir no arquivo
cmd-save = Salvar
cmd-close-tab = Fechar aba
cmd-split-editor-right = Dividir editor à direita
cmd-toggle-comment = Alternar comentário
cmd-duplicate-line = Duplicar linha
cmd-move-line-up = Mover linha para cima
cmd-move-line-down = Mover linha para baixo
cmd-toggle-soft-wrap = Alternar quebra de linha
cmd-toggle-parameter-name-hints = Alternar dicas de nomes de parâmetros
cmd-go-to-definition = Ir para definição
cmd-go-to-declaration-or-usages = Ir para declaração ou usos
cmd-go-to-implementations = Ir para implementações
cmd-find-references = Localizar referências
cmd-rename-symbol = Renomear símbolo…
cmd-extract-method = Extrair método
cmd-extract-variable = Extrair variável
cmd-extract-constant = Extrair constante
cmd-extract-field = Extrair campo
cmd-inline = Embutir
cmd-show-context-actions = Mostrar ações de contexto…
cmd-apply-top-fix = Aplicar a primeira correção
cmd-format-document = Formatar documento
cmd-format-selection = Formatar seleção
cmd-organize-imports = Organizar imports
cmd-show-parameter-hints = Mostrar dicas de parâmetros
cmd-trigger-completion = Acionar autocompletar
cmd-go-to-symbol-in-workspace = Ir para símbolo no workspace…
cmd-go-to-symbol-in-file = Ir para símbolo no arquivo…
cmd-file-structure = Estrutura do arquivo…
cmd-structure = Estrutura
cmd-type-hierarchy = Hierarquia de tipos
cmd-call-hierarchy = Hierarquia de chamadas
cmd-hierarchy = Hierarquia
cmd-show-problems = Mostrar problemas
cmd-run = Executar
cmd-debug = Depurar
cmd-run-context-configuration = Executar a configuração do contexto
cmd-debug-context-configuration = Depurar a configuração do contexto
cmd-stop = Parar
cmd-rerun = Executar novamente
cmd-rerun-failed-tests = Executar novamente os testes com falha
cmd-build-project = Compilar projeto
cmd-rebuild-project = Recompilar projeto
cmd-run-gradle-task = Executar tarefa Gradle…
cmd-run-maven-goal = Executar goal Maven…
cmd-show-the-build-window = Mostrar a janela de build
cmd-show-the-gradle-window = Mostrar a janela Gradle
cmd-show-the-maven-window = Mostrar a janela Maven
cmd-terminal = Terminal
cmd-output = Saída
cmd-settings-keymap = Configurações: atalhos de teclado
cmd-settings-terminal = Configurações: terminal
cmd-settings-coding-agents = Configurações: agentes de código
cmd-settings-gradle-and-maven = Configurações: Gradle e Maven
cmd-edit-configurations = Editar configurações…
cmd-edit-environments = Editar ambientes…
cmd-show-the-tests-window = Mostrar a janela de testes
cmd-record-at-cursor = Gravar no cursor
cmd-pick-locator = Escolher localizador
cmd-stop-recording = Parar a gravação
cmd-open-ui-mode = Abrir o modo UI
cmd-toggle-breakpoint = Alternar ponto de interrupção
cmd-toggle-breakpoint-enabled = Habilitar ou desabilitar ponto de interrupção
cmd-edit-breakpoint-condition = Editar condição do ponto de interrupção…
cmd-view-breakpoints = Ver pontos de interrupção…
cmd-evaluate-expression = Avaliar expressão…
cmd-run-to-cursor = Executar até o cursor
cmd-step-over = Contornar
cmd-step-into = Intervir
cmd-step-out = Sair da função
cmd-pause-program = Pausar o programa
cmd-resume-program = Retomar o programa
cmd-clear-output = Limpar a saída
cmd-sync-gradle-and-maven-projects = Sincronizar projetos Gradle e Maven
cmd-project-structure = Opções do projeto…
cmd-refresh-gradle-tasks = Atualizar tarefas Gradle
cmd-collapse-all-gradle-tasks = Recolher todas as tarefas Gradle
cmd-reload-maven-project = Recarregar projeto Maven
cmd-read-the-maven-goals-again = Ler os goals Maven novamente
cmd-collapse-all-maven-goals = Recolher todos os goals Maven
cmd-spring-beans-and-endpoints = Beans e endpoints Spring
cmd-read-the-spring-model-again = Ler o modelo Spring novamente
cmd-go-to-spring-bean-or-usages = Ir para bean Spring ou usos…
cmd-spring-active-profiles = Perfis ativos do Spring…
cmd-collapse-the-spring-window = Recolher a janela Spring
cmd-reload-all-from-disk = Recarregar tudo do disco
cmd-select-opened-file = Selecionar o arquivo aberto
cmd-collapse-the-project-tree = Recolher a árvore do projeto
cmd-hide-the-side-panel = Ocultar o painel lateral
cmd-sort-structure-by-name = Ordenar a estrutura por nome
cmd-show-processes = Mostrar processos
cmd-show-the-agent-log = Mostrar o registro do agente
cmd-stop-idle-processes = Parar os processos inativos
cmd-restart-the-java-engine = Reiniciar o mecanismo Java
cmd-send-selection-to-claude = Enviar a seleção para o Claude
cmd-accept-claudes-change = Aceitar a alteração do Claude
cmd-reject-claudes-change = Rejeitar a alteração do Claude
cmd-next-claude-change = Próxima alteração do Claude
cmd-previous-claude-change = Alteração anterior do Claude
cmd-accept-claude-change-at-caret = Aceitar a alteração do Claude no cursor
cmd-revert-claude-change-at-caret = Reverter a alteração do Claude no cursor
cmd-accept-all-claude-changes = Aceitar todas as alterações do Claude
cmd-revert-all-claude-changes = Reverter todas as alterações do Claude
cmd-show-agent-changes = Mostrar as alterações dos agentes
cmd-annotate-with-agents = Anotar com os agentes
cmd-annotate-with-git-blame = Anotar com Git Blame
cmd-show-history = Mostrar histórico
cmd-show-history-of-file = Mostrar histórico do arquivo
cmd-undo = Desfazer
cmd-redo = Refazer
cmd-zoom-in = Ampliar
cmd-zoom-out = Reduzir
cmd-actual-size = Tamanho real
cmd-show-project = Mostrar o projeto
cmd-show-git = Mostrar Git
cmd-toggle-agent-console = Alternar o console do agente
cmd-find-repositories-in-folder = Localizar repositórios em uma pasta…
cmd-clone-from-github = Clonar do GitHub…
cmd-new-group = Novo grupo…
cmd-refresh = Atualizar
cmd-pull = Pull
cmd-fetch = Fetch
cmd-push = Push
cmd-commit = Commit
cmd-draft-commit-message = Redigir a mensagem de commit
cmd-stage = Preparar
cmd-unstage = Remover da preparação
cmd-discard-changes = Descartar alterações…
cmd-check-out = Mudar para…
cmd-new-branch = Nova branch…
cmd-side-by-side-diff = Diff lado a lado
cmd-unified-diff = Diff unificado
cmd-compact-diff = Diff compacto
cmd-theme-match-the-system = Tema: igual ao sistema
cmd-theme-dark = Tema: escuro
cmd-theme-light = Tema: claro
cmd-open-on-github = Abrir no GitHub
cmd-open-in-external-terminal = Abrir em um terminal externo
cmd-about-snoutcode = Sobre o SnoutCode
cmd-check-for-updates = Verificar atualizações…
cmd-release-notes = Notas da versão
cmd-quit-snoutcode = Encerrar o SnoutCode
cmd-clear-recent = Limpar recentes
cmd-exit = Sair
cmd-quit = Sair
cmd-zoom = Zoom
cmd-maximize = Maximizar
cmd-command-palette = Paleta de comandos…
cmd-cut = Recortar
cmd-copy = Copiar
cmd-paste = Colar
cmd-select-all = Selecionar tudo
cmd-find = Localizar
cmd-replace = Substituir
cmd-project = Projeto
cmd-git = Git
cmd-agent-console = Console do agente
cmd-soft-wrap = Quebra de linha
cmd-parameter-name-hints = Dicas de nomes de parâmetros
cmd-processes = Processos
cmd-agent-log = Registro do agente
cmd-build-window = Janela de build
cmd-gradle = Gradle
cmd-maven = Maven
cmd-move-to-trash = Mover para a lixeira…
cmd-agent-changes = Alterações dos agentes
cmd-keymap = Atalhos de teclado…
cmd-terminal-more = Terminal…
cmd-coding-agents = Agentes de código…
cmd-gradle-and-maven = Gradle e Maven…
cmd-minimize = Minimizar
cmd-snoutcode-on-github = SnoutCode no GitHub
cmd-report-an-issue = Relatar um problema
cmd-snoutdata-website = Site da SnoutData
cmd-open-recent-project = Abrir recente: { $name }

## The platform's file manager

machine-mac = este Mac
machine-windows = este PC
machine-linux = este computador
reveal-mac = Mostrar no Finder
reveal-windows = Mostrar no Explorador
reveal-linux = Mostrar em Arquivos

## settings

settings-search-settings = Buscar configurações
settings-found-on-path-and-the-usual-places = Procurado no PATH e nos locais de costume
settings-its-programs-name = O nome do programa
settings-your-login-shell-shell = Seu shell de login ($SHELL)
settings-no-arguments = Sem argumentos
settings-name-value-then-return = NOME=valor e depois Enter
settings-search-commands-or-shortcuts = Pesquisar comandos ou atalhos
settings-overridden-by-this-project = Substituído por este projeto
settings-set-for-this-project = Definido para este projeto
settings-reset-to-your-setting = Voltar ao seu ajuste
settings-your-setting = Seu ajuste
settings-user = Usuário
settings-your-settings-json = Seu settings.json
settings-for-only-kept-in-in-the-project-what = Somente para { $root }, salvo em { $file } no projeto. O que ele define substitui suas configurações.
settings-your-settings-for-every-project-a-project-can = Suas configurações, para todos os projetos. Um projeto pode definir algumas delas para si, em Opções do projeto (⌘;).
settings-in-every-build = Em todas as versões
settings-reflow-kitty-keys-links = Refluxo, teclas kitty, links
settings-unavailable = Indisponível
settings-the-same-in-every-project = Igual em todos os projetos
settings-settings-are-yours-a-project-cannot-set-them = Os ajustes de { $title } são seus: um projeto não pode defini-los por conta própria.
settings-show-your-settings = Mostrar suas configurações
settings-indentation = Indentação
settings-tab-size = Tamanho da tabulação
settings-for-a-file-whose-own-indentation-cannot-be = Para um arquivo cuja indentação não pode ser determinada.
settings-display = Exibição
settings-soft-wrap-long-lines = Quebrar linhas longas
settings-show-whitespace = Mostrar espaços em branco
settings-show-parameter-names-of-literal-arguments-java = Mostrar os nomes dos parâmetros dos argumentos literais (Java)
settings-hover = Informação ao passar o mouse
settings-show-hover = Mostrar
settings-delay = Atraso
settings-navigation = Navegação
settings-click-with = Clicar com
settings-preview = Prévia
settings-fetch-every = Fetch a cada
settings-the-projects-are-fetched-in-the-background-to = O fetch dos projetos é feito em segundo plano, para contar o que há para trazer.
settings-annotations = Anotações
settings-show-who-wrote-each-class-and-method-ada = Mostrar quem escreveu cada classe e método (“Ada Lovelace +2”)
settings-show-the-current-lines-author-and-commit-after = Mostrar o autor e o commit da linha atual depois do fim dela
settings-good-to-know = Bom saber
settings-a-background-fetch-leaves-your-branches-as-they = Um fetch em segundo plano não mexe nas suas branches: só conta os commits a trazer.
settings-click-the-branch-in-the-title-bar-for = Clique na branch da barra de título para Nova branch, Fetch, Pull e Push, e para mudar para qualquer branch.
settings-2-shows-the-git-view-in-the-message = ⌘2 mostra a visão do Git. No campo da mensagem, ⌘↩ faz o commit.
settings-each-builds-own-your-settings = O de cada build (pelo nível Java dele)
settings-every-build = Todos os builds
settings-jdk-per-project = JDK por projeto
settings-no-gradle-build-is-open = Nenhum build Gradle está aberto.
settings-automatic-the-builds-java-level = Automático (o nível de Java do build)
settings-the-java-server-restarts-on-the-chosen-jdk = O servidor Java usará o JDK escolhido na próxima vez que iniciar.
settings-gradle-daemons = Daemons Gradle
settings-stop-idle-daemons-after = Parar se inativos
settings-double-click-a-task-in-the-gradle-tool = Clique duas vezes em uma tarefa na janela Gradle para executá-la. A saída aparece na janela de build.
settings-the-processes-window-lists-the-gradle-daemons-running = A janela Processos lista os daemons Gradle em execução e para os inativos quando você pede.
settings-project-structure-downloads-jdks-and-sets-the-projects = Opções do projeto (⌘;) baixa JDKs e define o do projeto.
settings-the-leaf-on-the-right-stripe-opens-the = A folha na barra direita abre a janela Spring: os beans e os endpoints que o SnoutCode encontrou.
settings-spring-active-profiles-p-changes-the-profiles-from = Perfis ativos do Spring (⌃⌥P) muda os perfis de qualquer lugar.
settings-a-bean-the-active-profiles-leave-out-is = Um bean deixado de fora pelos perfis ativos fica esmaecido, e nenhuma injeção o usa.
settings-active-profiles = Perfis ativos
settings-open-a-project-to-choose-its-profiles = Abra um projeto para escolher os perfis dele.
settings-any-no-profile-chosen = Qualquer um (nenhum perfil escolhido)
settings-those-the-application-files-make-active = Os ativados pelos arquivos da aplicação
settings-these-profiles = Estes perfis
settings-evaluate-beans-under = Avaliar os beans com
settings-profiles = Perfis
settings-choose = Escolher
settings-looking-for-it = Procurando…
settings-that-file-is-not-there = Esse arquivo não existe.
settings-not-found-install-it-from-the-console-or = Não encontrado. Instale pelo console ou escolha o programa.
settings-browse = Procurar…
settings-sees-the-editor-its-hooks-report-each-edit = Vê o editor; os hooks dele informam cada edição e prompt
settings-its-hooks-report-each-edit-and-prompt = Os hooks dele informam cada edição e prompt
settings-a-plugin-of-snoutcodes-reports-each-edit-and = Um plugin do SnoutCode informa cada edição e prompt
settings-files-seen-on-disk-while-it-works-in = Arquivos vistos no disco enquanto ele trabalha no console
settings-its-colour-caret-and-marks = Cor, cursor e marcas
settings-the-console-runs = O console executa
settings-programs = Programas
settings-your-agent = Seu agente
settings-its-name = Nome dele
settings-any-command-line-agent-the-console-runs-it = Qualquer agente de linha de comando: o console o executa no projeto com /bin/sh -c.
settings-j-shows-or-hides-the-console-the-session = ⌘J mostra ou oculta o console. A sessão continua enquanto ele está oculto.
settings-k-sends-the-selection-or-the-whole-file = ⌥⌘K envia a seleção, ou o arquivo inteiro, ao agente.
settings-in-a-proposed-changes-tab-accepts-it-and = Na aba de uma alteração proposta, ⌘↩ aceita e Esc rejeita.
settings-the-agent-changes-window-keeps-every-edit-an = A janela Alterações dos agentes guarda cada edição de um agente, por prompt, para aceitar ou reverter.
settings-ghostty-is-unavailable = O Ghostty está indisponível
settings-terminal-engine = Mecanismo do terminal
settings-use-the-editors = Usar a do editor
settings-the-editors-size = O tamanho do editor
settings-blink = Piscar
settings-look = Aparência
settings-line-height = Altura da linha
settings-cursor = Cursor
settings-remove-this-variable = Remover esta variável
settings-shell = Shell
settings-program = Programa
settings-empty-the-login-shell-of-your-account-on = Vazio: o shell de login da sua conta na máquina remota.
settings-empty-your-login-shell-on-a-remote-machine = Vazio: seu shell de login. Em uma máquina remota, seu shell de lá.
settings-arguments = Argumentos
settings-separated-by-spaces-l-starts-a-login-shell = Separados por espaços. -l inicia um shell de login.
settings-environment = Ambiente
settings-add = Adicionar
settings-added-to-the-terminals-and-the-claude-code = Adicionado aos terminais e ao console do Claude Code que você iniciar depois de uma alteração.
settings-behaviour = Comportamento
settings-scrollback = Histórico
settings-lines-for-terminals-started-after-a-change = linhas, para terminais iniciados depois de uma alteração
settings-copy-text-as-soon-as-it-is-selected = Copiar o texto assim que for selecionado
settings-use-option-as-meta-f-moves-a-word = Usar Option como Meta (⌥F avança uma palavra, não digita ƒ)
settings-external-terminal = Terminal externo
settings-open-in = Abrir em
settings-command = Comando
settings-running-now = Em execução
settings-a-terminal-running-another-engine-keeps-it-until = Um terminal com outro mecanismo o mantém até iniciar de novo (Novo, no cabeçalho dele).
settings-press-the-keys-or-esc = Pressione as teclas, ou Esc
settings-none = Nenhum
settings-was-the-shortcut-of-here-it-now-runs = { $keys } era o atalho de { $join }. Aqui agora ele executa este comando.
settings-remove-it-there = Removê-lo de lá
settings-no-command-matches = Nenhum comando corresponde.
settings-reset-all = Redefinir tudo
settings-adds-a-shortcut-press-its-keys-changes-apply = + adiciona um atalho: pressione as teclas. As alterações valem com Aplicar ou OK.
settings-settings = Configurações
settings-edit-as-json = Editar como JSON
settings-open-these-settings-file-in-a-tab-with = Abrir o arquivo destas configurações em uma aba, com autocompletar e verificações
settings-cancel = Cancelar
settings-apply = Aplicar
settings-cannot-be-read = Não é possível ler { $file }
settings-the-values-it-had-before-stay-in-use = Os valores anteriores continuam em uso até que seja corrigido. { $e }
settings-in-shop = { $shell } em shop
settings-engine = Mecanismo { $shown }
settings-found = Encontrado: { $path }
settings-editors = A do editor ({ $editor_font })
settings-not-installed = { $chosen } (não instalado)
settings-in = { $kind } em { $root }
settings-reset = Redefinir
settings-also-bound-to = Também atribuído a { $join }
settings-no-jdk = Nenhum JDK encontrado em { $machine }.
settings-after-a-delay = Depois de um atraso
settings-only-while-is-held = Só com ⌘ pressionado
settings-block = Bloco
settings-bar = Barra
settings-underline = Sublinhado
settings-hover-hint = Para problemas e suas correções, e o que é um símbolo. ⌘ sempre mostra na hora, com o link para a declaração.
settings-click-hint = Segure para sublinhar um nome como link e clique para ir à declaração (na declaração, aos usos). ⌥⌘-clique leva às implementações.
settings-blame-hint = Do blame do Git do arquivo. O autor da linha aparece quando o cursor fica parado pelo tempo de espera (Editor ▸ Informação ao passar o mouse). Anotar com Git Blame fica no menu de contexto da margem e no menu Git.
settings-daemons-hint = Os daemons Gradle do SnoutCode param após esse tempo sem build. Um novo valor vale para os daemons iniciados depois que o mecanismo Java reiniciar.
settings-profiles-hint = A janela Spring avalia os beans com esses perfis e esmaece os que ficam de fora. O menu de perfis dela muda esta mesma opção.
settings-agent-colours-hint = Cada agente escreve na sua cor: um cursor com o nome dele enquanto digita, uma marca na margem ao lado das linhas alteradas e um ponto nas abas e arquivos alterados até você revisá-los.
settings-programs-hint = Deixe em branco e o SnoutCode encontra cada programa, no seu PATH e onde os instaladores o colocam. Clique em Procurar para escolher um.
settings-terminal-scope-hint = A aparência, o mecanismo e as teclas dos terminais, e o terminal externo, são seus: são os mesmos em todos os projetos.
settings-engine-hint = O mecanismo lê o que roda nos terminais e no console do Claude Code. Uma mudança vale para os terminais abertos depois dela; os já abertos mantêm o mecanismo com que começaram.
settings-ghostty-preview-hint = O Ghostty também desenha riscada a chamada obsoleta, a linha ondulada de aviso e o link do relatório de testes.
settings-custom-terminal-hint = Executado com /bin/sh. { "{dir}" } é a pasta; em uma máquina remota, { "{ssh}" } é o comando ssh que abre um shell lá.
settings-external-terminal-hint = Abrir em um terminal externo, no menu Arquivo e no menu de um arquivo ou pasta, abre nessa pasta. Em um projeto remoto, abre uma sessão ssh lá.
settings-unit-minutes = minutos

## dialogs

dialogs-discard-body = { $count ->
    [one] Este arquivo volta a ser como está no HEAD. Arquivos não rastreados são excluídos.
   *[other] Estes { $count } arquivos voltam a ser como estão no HEAD. Arquivos não rastreados são excluídos.
    }
dialogs-trash-body = { $count ->
    [one] Este arquivo vai para a lixeira. Arquivos rastreados aparecem então como excluídos, prontos para o commit.
   *[other] Estes { $count } arquivos vão para a lixeira. Arquivos rastreados aparecem então como excluídos, prontos para o commit.
    }
dialogs-delete-body = { $count ->
    [one] Este arquivo é excluído do disco. Arquivos rastreados aparecem então como excluídos, prontos para o commit.
   *[other] Estes { $count } arquivos são excluídos do disco. Arquivos rastreados aparecem então como excluídos, prontos para o commit.
    }
dialogs-unsaved-files = { $count ->
    [one] Você tem alterações não salvas em um arquivo.
   *[other] Você tem alterações não salvas em { $count } arquivos.
    }
dialogs-version = Versão { $cargo_pkg_version }
dialogs-git-stopped-and-said-why = O Git parou e disse por quê.
dialogs-you-have-uncommitted-changes-what-should-happen-to = Você tem alterações sem commit. O que deve acontecer com elas?
dialogs-a-line-of-work-of-its-own-from = Uma linha de trabalho própria, a partir de onde você escolher.
dialogs-its-commits-stay-wherever-else-they-are-reachable = Os commits dela continuam onde ainda forem acessíveis.
dialogs-it-goes-for-everyone-who-uses-the-remote = Ela some para todos que usam o remoto. As branches locais não mudam.
dialogs-copy-a-remote-repository-to-this-computer-and = Copie um repositório remoto para este computador e abra-o.
dialogs-is-waiting-in-the-console-for-your-answer = { $agent } aguarda sua resposta no console.
dialogs-the-window-closes-and-the-java-engine-stops = A janela fecha, e o mecanismo Java para junto.
dialogs-what-runs-now-stops-when-snoutcode-quits = O que está em execução vai parar quando o SnoutCode encerrar.
dialogs-clone-repository = Clonar repositório
dialogs-local-branches = Branches locais
dialogs-remote-branches = Branches remotas
dialogs-tags = Tags
dialogs-unsaved = Não salvo
dialogs-these-stop = Isto vai parar:
dialogs-dont-ask-again-settings-appearance-confirm-before-quitti = Não perguntar de novo (Configurações, Aparência: Confirmar antes de encerrar)
dialogs-discard-and-quit = Descartar e encerrar
dialogs-save-all-and-quit = Salvar tudo e encerrar
dialogs-an-ide-for-java-gradle-and-spring-boot = Um IDE para Java, Gradle e Spring Boot, com Git e Claude Code integrados.
dialogs-made-by-snoutdata-mit-license = Feito pela SnoutData. Licença MIT.
dialogs-this-cannot-be-undone = Isto não pode ser desfeito.
dialogs-discard = Descartar
dialogs-you-can-still-get-them-back-from-the = Você ainda poderá recuperá-los da lixeira.
dialogs-move-to-trash = Mover para a lixeira
dialogs-delete = Excluir
dialogs-stash-check-out-re-apply = Stash, mudar e reaplicar
dialogs-put-the-changes-aside-switch-then-bring-them = Deixa as alterações de lado, muda e depois as traz de volta. Funciona mesmo se tocarem arquivos diferentes no destino.
dialogs-check-out-keep-changes = Mudar e manter as alterações
dialogs-carry-the-changes-across-as-they-are-git = Leva as alterações como estão. O Git recusa se elas fossem sobrescritas.
dialogs-this-leaves-head-detached-create-a-branch-if = Isto deixa o HEAD desanexado. Crie uma branch se quiser manter commits novos.
dialogs-and-have-each-moved-on-and-this-repositorys = { $branch } e { $upstream } avançaram cada uma por si, e as configurações deste repositório não dizem como combiná-las.
dialogs-merge = Merge
dialogs-join-both-histories-with-a-merge-commit-nothing = Une os dois históricos com um commit de merge. Nada é reescrito.
dialogs-rebase = Rebase
dialogs-replay-your-commits-on-top-of-the-remote = Reaplica seus commits sobre os remotos, para um histórico linear.
dialogs-and-have-no-commit-in-common-so-they = { $branch } e { $upstream } não têm nenhum commit em comum, então não podem ser unidas. Isso costuma indicar que o repositório remoto foi recriado ou que o histórico dele foi reescrito.
dialogs-resetting-makes-match-exactly-your-current-commits-stay = Redefinir faz { $branch } ficar exatamente igual a { $upstream }. Seus commits atuais ficam em uma branch de backup, então nada se perde.
dialogs-commit-or-stash-your-uncommitted-changes-first-resetting = Faça commit ou stash das suas alterações antes; redefinir as descartaria.
dialogs-reset-to = Redefinir para { $upstream }
dialogs-name = Nome
dialogs-starting-at = A partir de
dialogs-check-out-the-new-branch = Mudar para a nova branch
dialogs-create = Criar
dialogs-renaming = Renomeando { $old }
dialogs-goes-into-the-new-group = { $repo } entra no novo grupo.
dialogs-groups-keep-related-repositories-together-drag-repositor = Os grupos reúnem repositórios relacionados. Arraste repositórios para um deles.
dialogs-rename = Renomear
dialogs-goes-away = será excluída.
dialogs-delete-even-if-it-is-not-fully-merged = Excluir mesmo que não esteja totalmente unida
dialogs-also-delete-on-the-remote = Excluir também { $up } no remoto
dialogs-removing-it-from-the-remote-affects-everyone-who = Removê-la do remoto afeta todos que a usam.
dialogs-delete-branch = Excluir branch
dialogs-repository-url = URL do repositório
dialogs-into-folder = Na pasta
dialogs-lands-in = Destino:
dialogs-clone = Clonar
dialogs-no-matching-branch-tag-or-commit = Nenhuma branch, tag ou commit corresponde
dialogs-uncommitted-changes = Alterações sem commit:
dialogs-keep-them = Mantê-las
dialogs-stash-and-re-apply = Stash e reaplicar
dialogs-check-out = Mudar
dialogs-dont-allow = Não permitir
dialogs-allow-for-this-session = Permitir nesta sessão
dialogs-allow = Permitir
dialogs-close = Fechar
dialogs-a-branch-a-tag-or-any-commit = Uma branch, uma tag ou qualquer commit.
dialogs-something-went-wrong = Algo deu errado
dialogs-discard-changes = Descartar as alterações?
dialogs-check-out-2 = Mudar para { $t }?
dialogs-check-out-3 = Mudar de branch
dialogs-new-branch = Nova branch
dialogs-delete-branch-2 = Excluir a branch?
dialogs-delete-branch-from = Excluir a branch de { $remote }?
dialogs-move-to-the-trash = Mover para a lixeira?
dialogs-delete-files = Excluir os arquivos?
dialogs-both-sides-have-new-commits = Os dois lados têm commits novos
dialogs-no-shared-history = Sem histórico em comum
dialogs-git-could-not-sign-in-to = O Git não conseguiu entrar em { $host }
dialogs-git-could-not-sign-in = O Git não conseguiu entrar
dialogs-sign-in-helper-missing = O auxiliar de credenciais deste repositório precisa de { $program }, que não está instalado aqui.
dialogs-sign-in-helper-broken = O auxiliar de credenciais deste repositório não consegue rodar neste computador.
dialogs-sign-in-rejected = O servidor recusou o usuário e o token que o Git enviou.
dialogs-sign-in-none = O Git não tem usuário nem token para este servidor.
dialogs-sign-in-no-access = O servidor diz que o repositório não existe, ou que esta conta não pode vê-lo.
dialogs-sign-in-ssh-key = O servidor não aceitou nenhuma das chaves SSH deste computador.
dialogs-sign-in-host-key = O SSH ainda não conhece a chave de host deste servidor, ou a chave mudou.
dialogs-sign-in-fix-helper = Isso acontece quando um repositório foi copiado de outro computador com suas configurações, ou quando o login salvo aqui se perdeu. Instale o programa, ou remova o auxiliar e entre de novo com suas próprias credenciais.
dialogs-sign-in-fix-rejected = O token salvo pode ter expirado, sido revogado ou ser de outra conta. Entre de novo, ou crie um token novo e salve-o.
dialogs-sign-in-fix-none = Entre uma vez com um auxiliar de credenciais que salve o login, ou mude o remoto para SSH.
dialogs-sign-in-fix-no-access = Confira o endereço do remoto, e se a conta com que o Git entra pode ver o repositório. Com várias contas, a salva pode ser a errada.
dialogs-sign-in-fix-ssh-key = Adicione a chave pública deste computador à sua conta no servidor, ou carregue sua chave no agente SSH.
dialogs-sign-in-fix-host-key = Conecte uma vez num terminal, confira a impressão digital que o servidor mostra e aceite. Uma chave que mudou pode significar que o servidor foi trocado, ou que alguém está no meio.
dialogs-sign-in-agent = { $agent } recebe o que o Git disse, encontra a causa com você e guia você até a correção.
dialogs-fix-with = Corrigir com { $agent }
dialogs-open-terminal = Abrir terminal
dialogs-rename-group = Renomear grupo
dialogs-new-group = Novo grupo
dialogs-quit-snoutcode = Encerrar o SnoutCode?
dialogs-allow-to = Permitir que { $agent } { $what }?
dialogs-its-upstream = a branch upstream dela
dialogs-delete-from = Excluir de { $remote }
dialogs-allow-for-this-session-lets-it-use-again = Permitir nesta sessão deixa ele usar { $tool } de novo sem perguntar.

## welcome

welcome-minutes-ago = { $count ->
    [one] há { $count } minuto
   *[other] há { $count } minutos
    }
welcome-hours-ago = { $count ->
    [one] há { $count } hora
   *[other] há { $count } horas
    }
welcome-days-ago = { $count ->
    [one] há { $count } dia
   *[other] há { $count } dias
    }
welcome-weeks-ago = { $count ->
    [one] há { $count } semana
   *[other] há { $count } semanas
    }
welcome-months-ago = { $count ->
    [one] há { $count } mês
   *[other] há { $count } meses
    }
welcome-just-now = agora mesmo
welcome-yesterday = ontem
welcome-search-recent-projects = Pesquisar projetos recentes
welcome-welcome = Boas-vindas
welcome-go-to-file = Ir para arquivo
welcome-recent-files = Arquivos recentes
welcome-search-everywhere = Pesquisar em todo lugar
welcome-find-in-files = Localizar nos arquivos
welcome-new-file = Novo arquivo
welcome-no-files-open = Nenhum arquivo aberto
welcome-open-one-from-the-project-tree-or = Abra um na árvore do projeto, ou:
welcome-recent-files-2 = Arquivos recentes
welcome-clone-repository = Clonar repositório…
welcome-from-a-git-url = De uma URL do Git
welcome-java-or-spring-boot-app = App Java ou Spring Boot
welcome-no-recent-project-matches = Nenhum projeto recente corresponde.
welcome-no-recent-projects-yet-open-a-folder-to = Ainda não há projetos recentes. Abra uma pasta para começar.
welcome-recent-projects = Projetos recentes
welcome-a-folder-on = Uma pasta em { $host }
welcome-folder-on-machine = Uma pasta em { $machine }

## tabs

tabs-unpin-tab = Desafixar aba
tabs-pin-tab = Fixar aba
tabs-close-others = Fechar as outras
tabs-close-tabs-to-the-left = Fechar abas à esquerda
tabs-close-tabs-to-the-right = Fechar abas à direita
tabs-close-all = Fechar todas
tabs-close-all-but-pinned = Fechar todas menos as fixadas
tabs-keep-open = Manter aberta
tabs-reload-from-disk = Recarregar do disco
tabs-copy-path = Copiar caminho
tabs-clear = Limpar
tabs-sync-the-gradle-project = Sincronizar o projeto Gradle
tabs-read-the-tasks-again = Ler as tarefas novamente
tabs-collapse-all = Recolher tudo
tabs-reload-the-maven-project = Recarregar o projeto Maven
tabs-read-the-goals-again = Ler os goals novamente
tabs-stop-idle-processes-idle-gradle-daemons-leftovers-and = Parar os processos inativos: daemons Gradle inativos, sobras e mecanismos de projetos fechados
tabs-options = Opções
tabs-hide = Ocultar

## projects

projects-filter = Filtrar
projects-open-local-repository = Abrir repositório local…
projects-find-repositories-in-folder = Localizar repositórios em uma pasta…
projects-no-matches = Nenhum resultado
projects-rename = Renomear…
projects-delete-group = Excluir grupo
projects-new-group = Novo grupo…
projects-drag-repositories-here = Arraste repositórios para cá
projects-add-a-repository-or-a-group = Adicionar um repositório ou um grupo
projects-repositories = Repositórios
projects-no-repositories-yet-use-or-o = Ainda não há repositórios. Use + ou ⌘O.
projects-copy-path = Copiar caminho
projects-open-as-project = Abrir como projeto
projects-move-to-group = Mover para um grupo
projects-no-group = Sem grupo
projects-remove-from-list = Remover da lista

## status

status-snoutcodes-processes-running-of-memory-click-to-show = Processos do SnoutCode: { $count } em execução, { $memory } de memória. Clique para vê-los.
status-in-the-background = Em segundo plano
status-the-editor-works-meanwhile = O editor continua disponível.
status-reconnect = Reconectar
status-changes-the-agents-made-that-wait-for-your = Alterações dos agentes aguardando sua revisão
status-review = Revisar
status-restart = Reiniciar
status-read-only = Somente leitura
status-java-support-unavailable = Suporte a Java indisponível
status-chars-lines = { $chars } caracteres, { $lines } linhas
status-selected = { $chars } selecionados
status-and-more = e mais { $count }

## search

search-go-to-file = Ir para arquivo
search-line-or-line-column = Linha, ou linha:coluna
search-type-a-command = Digite um comando
search-run-a-gradle-task-or-maven-goal-its = Executar uma tarefa Gradle ou um goal Maven: nome ou caminho
search-spring-profiles-pick-one-to-turn-it-on = Perfis Spring: escolha um para ativar ou desativar
search-go-to-symbol = Ir para símbolo
search-filter-the-files-symbols = Filtrar os símbolos do arquivo
search-go-to-class-in-the-project-its-libraries = Ir para classe, no projeto, nas bibliotecas e no JDK
search-new-file-its-path-in-the-project-like = Novo arquivo: caminho no projeto, como src/main/java/App.java
search-no-file-matches = Nenhum arquivo corresponde
search-no-command-matches = Nenhum comando corresponde
search-no-task-matches = Nenhuma tarefa corresponde
search-no-profile-matches = Nenhum perfil corresponde
search-type-a-name = Digite um nome
search-no-symbol-matches = Nenhum símbolo corresponde
search-type-a-class-name-or-its-capitals = Digite um nome de classe, ou as maiúsculas dele
search-no-class-matches = Nenhuma classe corresponde
search-no-recent-file-matches = Nenhum arquivo recente corresponde
search-return-makes-the-file-and-opens-it = Enter cria o arquivo e o abre
search-type-a-line-number-or-line-column = Digite um número de linha, ou linha:coluna
search-go-to-line-column = Ir para a linha { $line }, coluna { $column }
search-try-fewer-letters-or-another-word = Tente menos letras ou outra palavra.

## find

find-matches = { $count ->
    [one] { $count } ocorrência
   *[other] { $count } ocorrências
    }
find-files = { $count ->
    [one] { $count } arquivo
   *[other] { $count } arquivos
    }
find-matches-in-files = { $matches } em { $files }
find-replaced = Substituídas { $matches } em { $files }
find-put-back = Restaurados: { $files }
find-to-replace = , { $ticked } a substituir
find-first-only = {" "}(somente as primeiras)
find-skipped = ; { $files } alterados nesse meio-tempo e deixados como estão
find-undo-hint = . Editar ▸ Desfazer substituição nos arquivos restaura.
find-not-a-regular-expression = Não é uma expressão regular: { $error }
find-the-regular-expression-is-too-large = A expressão regular é grande demais
find-module = Módulo
find-directory = Diretório
find-open-files = Arquivos abertos
find-text-to-find = Texto a localizar
find-replace-with = Substituir por
find-file-mask-java-test-java = Máscara: *.java, !*Test.java
find-folder-from-the-projects = Pasta, a partir da do projeto
find-match-case = Diferenciar maiúsculas
find-whole-words = Palavras inteiras
find-regular-expression-1-in-the-replacement-names-a = Expressão regular ($1 na substituição indica um grupo)
find-hide-replace = Ocultar Substituir
find-replace-in-files-r = Substituir nos arquivos (⇧⌘R)
find-choose = Escolher…
find-find-in-the-projects-files = Localizar nos arquivos do projeto
find-type-a-word-a-phrase-or-a-regular = Digite acima uma palavra, uma frase ou uma expressão regular.
find-nothing-found = Nada encontrado
find-try-fewer-letters-another-scope-or-a-wider = Tente menos letras, outro escopo ou uma máscara mais ampla.
find-searching = Pesquisando…
find-replace-selected = Substituir selecionados
find-replace-all = Substituir tudo
find-open = Abrir
find-replace-in-files = Substituir nos arquivos
find-no-replacement-to-undo = Nenhuma substituição para desfazer
find-not-a-file-mask = Não é uma máscara de arquivo: { $kind }
find-module-2 = Módulo: { $name }
find-of = { $ticked } de { $count }

## files

files-no-project-open = Nenhum projeto aberto
files-open-a-project-to-see-its-files = Abra um projeto para ver os arquivos.
files-the-connection-is-lost = A conexão foi perdida
files-node-package = Pacote Node: { $path }
files-the-files-on-come-back-when-it-reconnects = Os arquivos de { $host } voltam quando ele reconectar.

## project

project-no-classes-to-show = Nenhuma classe para mostrar
project-reading-the-build-model = Lendo o modelo de build
project-external-libraries = Bibliotecas externas
project-the-build-model-could-not-be-read = Não foi possível ler o modelo de build: { $e }
project-uses = usa { $join }
project-showing-the-last-model-that-synced = { $reason }{"\u000A"}Exibindo o último modelo sincronizado.
project-compiled-by = Compilado por { $join }
project-did-not-sync = Não sincronizou: { $reason }{"\u000A"}{"\u000A"}{ $note }
project-did-not-sync-2 = Não sincronizou: { $reason }
project-excluded = Excluído: { $path }
project-and-more = { $builds } e mais { $n }

## model

model-build-files = Arquivos de build
model-maven-files = Arquivos Maven
model-gradle-scripts = Scripts Gradle
model-detached-at = desanexado em { $count }
model-detached-head = HEAD desanexado
model-does-not-exist = { $path } não existe
model-ready = Pronto
model-no-upstream = {"  "}(sem upstream)
model-opened = { $asked } aberto
model-snoutcode-could-not-connect-to = O SnoutCode não conseguiu se conectar a { $connection }. { $why }
model-showing-in-git = Mostrando { $root } no Git
model-closed = { $closed } fechado
model-could-not-save-settings = Não foi possível salvar as configurações: { $e }
model-your-settings-json-has-an-error-so-the = Seu settings.json tem um erro, então as configurações em uso continuam: { $e }
model-your-settings-are-read-again = Suas configurações foram lidas novamente
model-could-not-save-the-projects-settings = Não foi possível salvar as configurações do projeto: { $e }
model-finished = { $label } concluído
model-cloned-into = Clonado em { $root }
model-failed = { $label } falhou
model-failed-2 = { $label } falhou:{"\u000A"}{ $e }
model-no-git-repositories-within-levels-of = Nenhum repositório Git a até { $SCAN_DEPTH } níveis de{"\u000A"}{ $root }
model-found-repositories-in-new = { $count } repositórios encontrados em { $root } ({ $added } novos)
model-asking-the-agent-to-draft-the-commit-message = Pedindo ao agente para redigir a mensagem de commit…
model-the-agent-has-not-sent-a-commit-message = O agente ainda não enviou uma mensagem de commit. A resposta dele está no console.
model-asks-you-something-in-the-console-answer-it = { $name } pergunta algo no console. Responda lá e a redação continua.
model-is-writing-the-commit-message = { $name } está escrevendo a mensagem de commit…
model-the-agent-finished-without-writing-a-commit-message = O agente terminou sem escrever uma mensagem de commit. A resposta dele está no console.
model-no-commit-message-yet = Ainda não há mensagem de commit
model-drafted-the-commit-message = { $name } redigiu a mensagem de commit
model-refreshed = Atualizado
model-open-project = Abrir projeto
model-initialized-repository-in = Repositório inicializado em { $root }
model-find-repositories-in-folder = Localizar repositórios em uma pasta
model-searching-for-repositories = Procurando repositórios
model-staged = { $count ->
    [one] 1 arquivo preparado
   *[other] { $count } arquivos preparados
    }
model-discarded = { $count ->
    [one] 1 arquivo descartado
   *[other] { $count } arquivos descartados
    }
model-trashed = { $count ->
    [one] 1 arquivo movido para a lixeira
   *[other] { $count } arquivos movidos para a lixeira
    }
model-deleted = { $count ->
    [one] 1 arquivo excluído
   *[other] { $count } arquivos excluídos
    }
model-unstaged = Removido da preparação
model-added-to-gitignore = { $pattern } adicionado ao .gitignore
model-added-rules-to-gitignore = { $n } regras adicionadas ao .gitignore
model-committed = Commit feito: { $summary }
model-stopped-drafting = Redação interrompida
model-checked-out = Mudou para { $target }
model-local-changes-conflicted = As alterações locais entraram em conflito
model-created-branch = Branch { $name } criada
model-deleted-branch = Branch { $name } excluída
model-delete = Excluir { $r }
model-pull-merge = Pull (merge)
model-pull-rebase = Pull (rebase)
model-reset-to = { $head } redefinida para { $upstream }
model-your-previous-commits-are-kept-on-the-branch = Seus commits anteriores ficam na branch { $backup }. Exclua-a quando não precisar mais deles.
model-now-matches = { $head } agora corresponde a { $upstream }
model-commit = Commit { $short_id } { $summary }
model-not-in-the-repository = { $path } não está no repositório que a visão Git mostra
model-sources = Fontes
model-test-sources = Fontes de teste
model-test-resources = Recursos de teste
model-generated-sources = Fontes geradas
model-the-build-did-not-sync = O build não sincronizou
model-sync = Sincronização: { $root }
model-the-gradle-sync-failed = A sincronização Gradle falhou

## branch

branch-search-branches = Pesquisar branches
branch-local = Local
branch-remote = Remoto

## panels

panels-files = Arquivos
panels-no-output-yet = Ainda não há saída
panels-what-git-and-the-tools-say-shows-here = O que o Git e as ferramentas dizem aparece aqui enquanto trabalham.
panels-open-git = Abrir Git

## title

title-no-project = Sem projeto
title-current-file = Arquivo atual
title-yours = As suas
title-reading-the-configurations = Lendo as configurações…
title-configurations-appear-once-the-tests-are-listed = As configurações aparecem depois que os testes forem listados
title-configurations-appear-once-the-java-engine-is-ready = As configurações aparecem quando o mecanismo Java estiver pronto
title-stop-f2 = Parar (⌘F2)
title-search-everywhere-a = Pesquisar em todo lugar (⇧⌘A)
title-settings = Configurações (⌘,)
title-the-current-file = o arquivo atual
title-rerun-r = Executar { $what } novamente (⌃R)
title-run-r = Executar { $what } (⌃R)
title-debug-d = Depurar { $what } (⌃D)

## stripes

stripes-left = Esquerda
stripes-right = Direita
stripes-bottom = Inferior
stripes-move-to = Mover para
stripes-build = Build
stripes-problems-m = Problemas (⇧⌘M)
stripes-agent-console-j = Console do agente (⌘J)
stripes-spring = Spring
stripes-notifications = Notificações
stripes-structure-7 = Estrutura (⌘7)
stripes-hierarchy-h-h = Hierarquia (⌃H, ⌃⌥H)
stripes-agent-view = Visão do agente
stripes-tests = Testes
stripes-project-1 = Projeto (⌘1)
stripes-git-2 = Git (⌘2)

## git

git-state = Estado
git-relative-directory = Diretório relativo
git-file-filter = Filtrar arquivos
git-message-author-sha-or-ref = Mensagem, autor, SHA ou ref.
git-commit-message = Mensagem de commit
git-message = Mensagem
git-author = Autor
git-date = Data
git-push = Push ({ $n })
git-hide = Ocultar { $short }
git-console-in-this-repository-j = Console do { $agent } neste repositório (⌘J)
git-no-github-remote = Sem remoto do GitHub
git-open-local-repository-o = Abrir repositório local (⌘O)
git-git-push-sets-upstream-on-first-push = git push (define o upstream no primeiro push)
git-move-to-the-trash = Mover para a lixeira (⌘⌫)
git-branch-tag-or-commit-o = Branch, tag ou commit (⇧⌘O)
git-new-branch = Nova branch
git-no-repository-open = Nenhum repositório aberto
git-local-branches = Branches locais
git-tags = Tags ({ $count })
git-branches = Branches
git-check-out-o = Mudar… (⇧⌘O)
git-double-click-to-check-out = { $b }{ $u }{"\u000A"}{ $target } { $subject }{"\u000A"}{"\u000A"}Clique duas vezes para mudar
git-double-click-to-check-out-detached = { $target } { $subject }{"\u000A"}{ $date }{"\u000A"}{"\u000A"}Clique duas vezes para mudar (HEAD desanexado)
git-delete = Excluir…
git-delete-from = Excluir de { $remote }…
git-check-out-as-local = Mudar para a branch local “{ $local_name }”
git-delete-branch = Excluir branch…
git-delete-from-the-remote = Excluir do remoto…
git-create-branch-from-here = Criar branch a partir daqui…
git-copy-name = Copiar nome
git-check-out-detached = Mudar (HEAD desanexado)
git-create-branch-from-tag = Criar branch a partir da tag…
git-tag = tag { $name }
git-nothing-to-commit-on = Nada para commitar em { $head }
git-nothing-to-commit = Nada para commitar
git-show-all-files = Mostrar todos os arquivos
git-in = em { $dir }/
git-the-working-tree-is-clean = A árvore de trabalho está limpa
git-discard = Descartar…
git-add-to-gitignore = Adicionar ao .gitignore
git-see-console = Ver o console
git-writing = Escrevendo…
git-writes-the-message-from-building-on-anything-you = { $agent } escreve a mensagem a partir de { $what }, aproveitando o que você digitou. Se não estiver em execução, ele inicia no console.
git-draft = Redigir
git-amend = Emendar
git-amend-last-commit = Emendar o último commit
git-commits = Faz commit de { $what }  ⌘↩
git-files-selected = { $count } arquivos selecionados
git-select-a-file-to-see-its-changes = Selecione um arquivo para ver as alterações
git-changes = Alterações
git-nothing-staged = Nada preparado
git-stage-files-to-see-what-the-next-commit = Prepare arquivos para ver o que o próximo commit contém.
git-journal = Histórico
git-click-a-commit-to-see-what-it-changed = Clique em um commit para ver o que ele alterou
git-history-of = Histórico de { $name }
git-history-of-tooltip = Os commits que mudaram { $path }, seguindo renomeações. Clique para ver o Histórico inteiro de novo.
git-no-commits-changed-it = Nenhum commit mudou este arquivo
git-it-is-not-committed-yet = Ele é novo, ou nunca teve commit nesta branch.
git-no-commits-yet = Ainda não há commits
git-stage-some-files-and-make-the-first-one = Prepare alguns arquivos e faça o primeiro
git-show-changes = Mostrar alterações
git-check-out-this-commit-detached = Mudar para este commit (HEAD desanexado)
git-create-branch-here = Criar branch aqui…
git-copy-sha = Copiar SHA
git-copy-message = Copiar mensagem
git-working-tree = Árvore de trabalho
git-back-esc = Voltar (Esc)
git-the-commit-could-not-be-read = Não foi possível ler o commit
git-from = { $directory }  (de { $orig })
git-this-commit-changes-no-files = Este commit não altera nenhum arquivo
git-committer = Committer
git-committed = Commit em
git-parents = Pais
git-parent = Pai
git-none-this-is-the-first-commit = nenhum, este é o primeiro commit
git-select-a-file = Selecione um arquivo
git-nothing = (nada)
git-this-commit = Este commit
git-git-could-not-read-this-folder = O Git não conseguiu ler esta pasta
git-the-project-works-without-it-but-its-changes = O projeto funciona sem ele, mas as alterações e o histórico não podem ser exibidos.
git-this-folder-has-no-git-repository = Esta pasta não tem repositório Git
git-the-project-works-without-git-start-a-repository = O projeto funciona sem Git. Crie um repositório para acompanhar alterações e histórico.
git-try-again = Tentar novamente
git-initialize-a-repository-here = Inicializar um repositório aqui
git-find-repositories-inside = Localizar repositórios aqui dentro
git-open-a-project-to-see-its-changes-branches = Abra um projeto para ver as alterações, branches e histórico.
git-open-project-o = Abrir projeto…   ⌘O
git-unified = Unificado
git-side-by-side = Lado a lado
git-compact-hide-unchanged-lines-away-from-changes = Compacto: ocultar linhas inalteradas longe das alterações
git-compact = Compacto
git-hide-unchanged-lines-away-from-changes = Ocultar linhas inalteradas longe das alterações
git-no-diff = Sem diff
git-binary-file = Arquivo binário
git-its-versions-differ-as-bytes = As versões diferem em bytes.
git-too-large-to-show = Grande demais para exibir
git-the-file-is-too-large-to-diff-here = O arquivo é grande demais para diff aqui.
git-no-differences = Sem diferenças
git-and-are-the-same = { $left } e { $right } são iguais.
git-changed = { $count ->
    [one] { $count } alterado
   *[other] { $count } alterados
    }
git-changed-staged = { $count ->
    [one] { $count } alterado
   *[other] { $count } alterados
    }, { $staged_count ->
    [one] { $staged_count } preparado
   *[other] { $staged_count } preparados
    }

## console

console-console = Console
console-the-installer-is-still-at-work-end-stops = O instalador ainda está trabalhando; Encerrar o interrompe
console-ended = { $name } terminou
console-ended-resume = { $name } terminou; Continuar o retoma
console-your-own-agent-has-no-command-yet-give = Seu agente ainda não tem comando. Defina um em Configurações, Agentes, e clique em Redigir de novo.
console-find-in-terminal = Localizar no terminal
console-of = { $i } de { $n }
console-previous-match-return = Ocorrência anterior (Enter)
console-next-match-shift-return = Próxima ocorrência (Shift+Enter)
console-close-esc = Fechar (Esc)
console-open-a-project-to-run-a-shell-in = Abra um projeto para executar um shell nele.
console-not-running = Parado
console-starting = Iniciando
console-connected = Conectado
console-not-connected = Não conectado
console-running = Em execução
console-working = Trabalhando
console-installing = Instalando
console-ended-2 = Encerrado
console-is-not-running = { $agent } não está em execução
console-is-starting = { $agent } está iniciando
console-sees-the-editor-its-selection-open-files-and = { $agent } vê o editor: seleção, arquivos abertos e problemas; propõe alterações ali para você aceitar ou rejeitar, e as edições aparecem à medida que ele as faz
console-runs-but-has-not-connected-to-the-editor = { $agent } está em execução, mas ainda não se conectou ao editor
console-runs-the-files-it-changes-show-as-it = { $agent } está em execução; os arquivos que ele altera aparecem enquanto trabalha
console-is-at-work-on-your-prompt = { $agent } está trabalhando no seu prompt
console-is-being-installed = { $agent } está sendo instalado
console-has-ended = { $agent } terminou
console-the-shell = O shell
console-follow = Seguir
console-following-the-editor-goes-to-the-files-the = Seguindo: o editor vai para os arquivos que o agente altera, enquanto ele digita
console-not-following-the-editor-stays-where-you-are = Sem seguir: o editor fica onde você está; pontos marcam o que o agente alterou
console-start-again-with-a-new-conversation = Recomeçar com uma nova conversa
console-start-a-new-shell = Iniciar um novo shell
console-new = Novo
console-continue = Continuar
console-start-again-on-the-most-recent-conversation-in = Retomar a conversa mais recente desta pasta
console-end = Encerrar
console-end-this-session = Encerrar esta sessão
console-hide-j-the-session-keeps-running = Ocultar (⌘J); a sessão continua
console-starting-the-shell = Iniciando o shell…
console-starting-2 = Iniciando { $agent }…
console-the-shell-is-not-running = O shell não está em execução
console-start-a-shell-in-the-project = Inicie um shell no projeto.
console-start-shell = Iniciar shell
console-start-claude = Iniciar o Claude
console-start-codex = Iniciar o Codex
console-start-opencode = Iniciar o opencode
console-start-amp = Iniciar o Amp
console-start-agent = Iniciar agente
console-it-sees-the-editor-and-its-edits-type = Ele vê o editor, e as edições se digitam sozinhas, na cor dele, para você revisar.
console-the-files-it-changes-type-themselves-in-in = Os arquivos que ele altera se digitam sozinhos, na cor dele, para você revisar.
console-continue-last-session = Continuar a última sessão
console-install-here = Instalar executa o instalador aqui no console. Se estiver instalado em outro lugar, defina o caminho em Configurações, Agentes.
console-install-here-node = Instalar executa o instalador aqui no console. Ele precisa do Node.js. Se estiver instalado em outro lugar, defina o caminho em Configurações, Agentes.
console-is-not-installed = { $agent } não está instalado
console-set-its-path-in-settings-agents = Defina o caminho em Configurações, Agentes.
console-your-own-agent-has-no-command-yet = Seu agente ainda não tem comando
console-name-the-command-line-agent-to-run-here = Informe o agente de linha de comando a executar aqui e o nome a exibir, em Configurações, Agentes.
console-agent-settings = Configurações do agente
console-set-its-path = Definir o caminho
console-s-developers-recommend-running-it-in-wsl = Os desenvolvedores do { $name } recomendam executá-lo no WSL.
console-looking-for-in-wsl = Procurando { $name } no WSL…
console-open-a-folder-in = Abrir uma pasta em { $d }
console-is-installed-in = { $name } está instalado em { $and_value }
console-open-a-folder-in-and-the-console-runs = { $makers } Abra uma pasta em { $has } e o console executará o { $name } lá.
console-this-pc-has-no-wsl-distribution-yet-wsl = { $makers } Este PC ainda não tem distribuição WSL: `wsl --install` no PowerShell instala uma. Para executar o { $name } no próprio Windows, instale com a linha abaixo ou defina o caminho.
console-on-windows-itself = No próprio Windows
console-install-runs-its-installer-in-then-open-a = { $makers } Instalar executa o instalador em { $first }; depois abra uma pasta lá e o console executará o { $name } nela. No próprio Windows, ele é instalado com a segunda linha.
console-install-in = Instalar em { $d }
console-in-wsl = No WSL
console-the-installer = O instalador
console-try-again = Tentar novamente
console-start-a-new-session = Iniciar uma nova sessão
console-open-link = Abrir link
console-copy-link = Copiar link
console-install-claude-code = Instalar o Claude Code
console-install-codex = Instalar o Codex
console-install-opencode = Instalar o opencode
console-install-amp = Instalar o Amp
console-install = Instalar
console-starts-with-the-next-session = { $agent } iniciará na próxima sessão
console-draft-stopped-the-console-started-a-new-session = Redação interrompida: o console iniciou uma nova sessão. Clique em Redigir para pedir ao { $name }.
console-could-not-start = Não foi possível iniciar { $program }: { $e }
console-is-not-installed-yet-install-it-in-the = { $agent } ainda não está instalado. Instale no console e clique em Redigir de novo.
console-is-not-running-2 = { $agent } não está em execução.
console-set-up = { $a } (configurar)
console-install-2 = { $a } (instalar)
console-open-a-project-to-run-in-it = Abra um projeto para executar o { $agent } nele.

## workspace

workspace-could-not-make = Não foi possível criar { $path }: { $e }
workspace-do-you-want-to-save-the-changes-you = Deseja salvar as alterações feitas em { $one }?
workspace-do-you-want-to-save-the-changes-you-2 = Deseja salvar as alterações feitas em { $count } arquivos?
workspace-your-changes-are-lost-if-you-dont-save = Suas alterações serão perdidas se você não as salvar.
workspace-dont-save = Não salvar
workspace-open-a-project-to-see-its-structure = As opções do projeto são para um projeto: esta pasta não tem nenhum build Gradle, Maven ou Node
workspace-settings-on-machine = Seu settings.json está em { $machine }: abra-o em uma janela em { $machine }
workspace-these-settings-are-not-kept-in-a-file = Estas configurações não ficam em um arquivo
workspace-open-a-project-to-edit-its-settings = Abra um projeto para editar as configurações
workspace-the-projects-settings-are-read-again = As configurações do projeto foram lidas novamente
workspace-has-an-error-so-its-last-good-values = { $file } tem um erro, então os últimos valores válidos continuam: { $e }
workspace-reading-the-projects-folders = Lendo as pastas do projeto
workspace-reading-the-open-file-and-the-projects-folders = Lendo o arquivo aberto e as pastas do projeto
workspace-reading-of-open-files-and-the-projects-folders = Lendo { $n } de { $total } arquivos abertos e as pastas do projeto
workspace-nothing-changed-on-disk = Nada mudou no disco
workspace-1-file-changed-on-disk = 1 arquivo mudou no disco
workspace-files-changed-on-disk = { $n } arquivos mudaram no disco
workspace-drop-a-folder-to-open-it = Solte uma pasta para abri-la

## tools

tools-problems = Problemas
tools-references = Referências
tools-show-output = Mostrar a saída
tools-no-notifications = Nenhuma notificação
tools-errors-from-git-and-the-tools-show-here = Os erros do Git e das ferramentas aparecem aqui.

## state

state-modified = Modificado
state-added = Adicionado
state-deleted = Excluído
state-renamed = Renomeado
state-copied = Copiado
state-type-changed = Tipo alterado
state-untracked = Não rastreado
state-conflicted = Em conflito
state-staged = Preparado
state-partly-staged = Parcialmente preparado

## splash

splash-starting-snoutcode = Iniciando o SnoutCode

## updates

updates-snoutcode-update = Atualização do SnoutCode
updates-updated-to-v = Atualizado para v{ $version }
updates-dismiss = Dispensar
updates-later = Mais tarde
updates-download = Baixar
updates-restart-to-install-the-update = Reiniciar para instalar a atualização
updates-restart-to-update = Reiniciar para atualizar

## background

background-reading-the-gradle-model = Lendo o modelo Gradle
background-compiling-scala = Compilando Scala
background-starting-java = Iniciando Java
background-indexing = Indexando
background-restarting-java = Reiniciando Java
background-waiting-for-snout-engine = Aguardando o snout-engine
background-attempt-of-5 = {" "}(tentativa { $n } de 5)
background-the-java-engine-stopped-unexpectedly-and-is-starting = O mecanismo Java parou inesperadamente e está reiniciando{ $again }: { $doing }
background-java-stopped-starting-again = Java parou, reiniciando
background-building-snout-engine = Compilando o snout-engine

## quit

quit-debugging = Depuração
quit-gradle-build = Build Gradle
quit-compiling-the-scala-classes = Compilando as classes Scala
quit-claude-code-editing = Claude Code, editando { $name }
quit-the-connection-to-and-what-the-project-runs = A conexão com { $host } e o que o projeto executa lá
quit-a-file-could-not-be-saved-so-snoutcode = Um arquivo não pôde ser salvo, então o SnoutCode não encerrou

## terminal

terminal-custom-command = Comando personalizado
terminal-command-prompt = Prompt de Comando
terminal-the-custom-external-terminal-has-no-command-set = O terminal externo personalizado não tem comando: defina um em Configurações ▸ Ferramentas ▸ Terminal.
terminal-could-not-run-the-external-terminal-command = Não foi possível executar o comando do terminal externo: { $e }
terminal-could-not-open-the-external-terminal = Não foi possível abrir o terminal externo: { $e }
terminal-could-not-open-the-external-terminal-2 = Não foi possível abrir o terminal externo: { $trim }

## debug

debug-breakpoints-go-in-a-java-kotlin-scala-typescript = Pontos de interrupção ficam em um arquivo Java, Kotlin, Scala, TypeScript ou JavaScript
debug-there-is-no-breakpoint-on-this-line-f8 = Não há ponto de interrupção nesta linha: ⌘F8 define um
debug-no-spring-bean-injection-or-endpoint-on-this = Nenhum bean, injeção ou endpoint Spring nesta linha
debug-run-to-cursor-works-while-the-debugger-is = Executar até o cursor funciona com o depurador parado
debug-put-the-caret-on-a-line-of-code = Coloque o cursor em uma linha de código
debug-nothing-to-pause-no-program-is-running-in = Nada para pausar: nenhum programa está em execução no depurador
debug-nothing-ran-yet-to-run-again = Nada foi executado ainda para repetir
debug-no-failed-tests-to-run-again = Nenhum teste com falha para repetir
debug-the-maven-goals-are-being-read-try-again = Os goals Maven estão sendo lidos; tente de novo em instantes
debug-no-maven-goals-the-project-has-no-maven = Nenhum goal Maven: o projeto não tem build Maven ou ele está sendo lido
debug-the-gradle-tasks-are-being-read-try-again = As tarefas Gradle estão sendo lidas; tente de novo em instantes
debug-no-gradle-tasks-yet-the-projects-build-is = Ainda não há tarefas Gradle: o build do projeto está sendo lido
debug-on = Ativado
debug-any-profile-every-bean-may-be-there = Qualquer perfil: qualquer bean pode estar presente
debug-as-the-application-files-say = Conforme os arquivos da aplicação ({ $join })
debug-the-maven-build-is-being-read-try-again = O build Maven está sendo lido; tente de novo em instantes
debug-stop-only-when-this-is-true-as-i = Parar só quando isto for verdadeiro, como `i > 3`
debug-if = se { $c }
debug-remove = Remover
debug-no-breakpoints = Nenhum ponto de interrupção
debug-click-left-of-a-line-number-or-press = Clique à esquerda de um número de linha, ou pressione ⌘F8 em um editor, para definir um.
debug-condition-of = Condição de { $path }:{ $line }
debug-breakpoints-count = { $count ->
    [one] { $count } ponto de interrupção
   *[other] { $count } pontos de interrupção
    }
debug-breakpoints-disabled = { $breakpoints }, { $off } desabilitados
debug-untick-one-to-keep-it-without-stopping-there = Desmarque um para mantê-lo sem parar ali. Um clique duplo mostra a linha.
debug-remove-all = Remover todos
debug-go-to-line = Ir para a linha
debug-an-expression-as-total-2 = Uma expressão, como `total * 2`
debug-evaluate-works-while-the-program-is-stopped-at = Avaliar funciona com o programa parado em um ponto de interrupção: depure (⌃D) e defina um (⌘F8).
debug-press-to-evaluate-it-in-the-frame-the = Pressione ⏎ para avaliar no quadro em que o depurador parou.
debug-add-it-to-the-watches-to-open-its = Adicione às inspeções (⌘⏎) para abrir os campos na janela de depuração.
debug-evaluate-expression = Avaliar expressão
debug-java-in-the-frame-the-debugger-stopped-in = Java, no quadro em que o depurador parou.
debug-add-to-watches = Adicionar às inspeções
debug-evaluate = Avaliar
debug-could-not-connect-the-debugger = Não foi possível conectar o depurador: { $e }
debug-debugger-attached = Depurador anexado
debug-the-debugger-could-not-attach = O depurador não conseguiu anexar: { $e }
debug-nothing-to-reload = Nada para recarregar
debug-reloaded = { $classes } recarregada
debug-reloaded-classes = { $n } classes recarregadas
debug-could-not-reload-the-changed-classes = Não foi possível recarregar as classes alteradas: { $e }

## lang

lang-symbols-come-from-the-java-server-which-starts = Os símbolos vêm do servidor Java, que inicia com um arquivo Java
lang-file-structure-shows-the-symbols-of-a-java = Estrutura do arquivo mostra os símbolos de um arquivo Java
lang-classes-come-from-the-java-engine-which-starts = As classes vêm do mecanismo Java, que inicia com um projeto Gradle
lang-hierarchies-come-from-the-java-engine-in-a = As hierarquias vêm do mecanismo Java, em um arquivo Java
lang-downloading-jdk = Baixando JDK { $major }… { $pct }%
lang-downloading-jdk-2 = Baixando JDK { $major }…
lang-jdk-could-not-be-downloaded = Não foi possível baixar o JDK { $major }: { $error }
lang-this-project-needs-jdk-download-it = Este projeto precisa do JDK { $major }. Baixar?
lang-jdk-installed-too-new = Este projeto precisa do JDK { $major }: o Gradle dele roda no máximo no Java { $max }, e os JDKs encontrados aqui são { $installed }. Baixar o JDK { $major }?
lang-jdk-installed-too-old = Este projeto precisa do JDK { $major }, e os JDKs encontrados aqui são mais antigos ({ $installed }). Baixar?
lang-use-another-jdk = Usar outro JDK…
lang-not-now = Agora não
lang-downloading-node = Baixando Node { $prompt }… { $pct }%
lang-downloading-node-2 = Baixando Node { $prompt }…
lang-node-could-not-be-downloaded = Não foi possível baixar o Node { $prompt }: { $error }
lang-this-project-needs-node-which-is-not-on = Este projeto precisa do Node { $prompt }, que não está nesta máquina. Baixar para o SnoutCode?
lang-installing-the-packages-of-the-build-window-shows = Instalando os pacotes de { $dir }… A janela de build mostra o andamento.
lang-installing-the-projects-packages-the-build-window-shows = Instalando os pacotes do projeto… A janela de build mostra o andamento.
lang-installing-playwrights-browsers-the-build-window-shows-h = Instalando os navegadores do Playwright… A janela de build mostra o andamento.
lang-the-packages-of-are-not-installed-so-it = Os pacotes de { $dir } não estão instalados, então ainda não pode ser executado.
lang-this-playwright-projects-packages-are-not-installed-so = Os pacotes deste projeto Playwright não estão instalados, então os testes ainda não podem ser executados.
lang-this-projects-packages-are-not-installed-so-it = Os pacotes deste projeto não estão instalados, então ainda não pode ser executado.
lang-install-packages = Instalar pacotes ({ $command })
lang-playwright-needs-browsers-that-are-not-downloaded = O Playwright { $version } precisa de navegadores que não foram baixados: { $join }.
lang-install-browsers = Instalar navegadores
lang-pom-changed = Um pom.xml mudou. Recarregar o projeto Maven?
lang-build-files-changed = Arquivos de build mudaram. Sincronizar?
lang-gradle-files-changed = Arquivos Gradle mudaram. Sincronizar?
lang-the-sync-failed = A sincronização falhou: { $e }
lang-reload = Recarregar
lang-sync = Sincronizar
lang-library-classes-open-once-the-java-engine-is = As classes de biblioteca abrem quando o mecanismo Java estiver pronto
lang-the-java-engine-does-not-know = O mecanismo Java não conhece { $qualified }

## record

record-works-in-a-playwright-project = { $what } funciona em um projeto Playwright
record-open-a-spec-file-and-put-the-caret = { $what }: abra um arquivo de spec e coloque o cursor onde deve ficar
record-a-recording-is-going-stop-it-first = Há uma gravação em andamento: pare-a primeiro
record-it-did-not-start = Não iniciou
record-the-recorder-stopped-the-build-window-says-why = O gravador parou: a janela de build diz por quê
record-the-recording-ended-with-no-steps = A gravação terminou sem passos
record-recorded-1-step-into = 1 passo gravado em { $unwrap_or_default }
record-recorded-steps-into = { $n } passos gravados em { $unwrap_or_default }
record-picking-is-on-click-an-element-in-the = Seleção ativa: clique em um elemento no navegador
record-pick-locator-stopped = Escolher localizador parou
record-picked = { $locator } escolhido
record-ui-mode-works-in-a-playwright-project = O modo UI funciona em um projeto Playwright
record-ui-mode-opens-in-your-browser = O modo UI abre no seu navegador

## remote

remote-could-not-connect-to = Não foi possível conectar a { $name }: { $why }
remote-trust-this-host = Confiar neste host?
remote-key-passphrase = Senha da chave
remote-no = Não
remote-yes = Sim
remote-password = Senha:
remote-user-host-a-host-of-your-ssh-config = usuário@host, um Host da sua configuração ssh, ou WSL: Ubuntu
remote-user-host-or-a-host-of-your-ssh = usuário@host, ou um Host da sua configuração ssh
remote-recent = Recentes
remote-from-your-ssh-config = Da sua configuração ssh
remote-connecting-to = Conectando a { $h }…
remote-connecting-to-2 = Conectando a { $host }…
remote-the-first-time-takes-a-little-longer-snoutcode = A primeira vez demora um pouco mais: o SnoutCode instala o servidor lá.
remote-no-hosts-in-your-ssh-config-type-one = Nenhum host na sua configuração ssh: digite um.
remote-connect-to-ssh-host-or-wsl = Conectar a um host SSH ou WSL
remote-connect-to-ssh-host = Conectar a um host SSH
remote-snoutcode-installs-its-server-on-the-host-the = O SnoutCode instala o servidor no host na primeira vez (Linux, x86_64 ou aarch64).
remote-host = Host
remote-connect = Conectar
remote-on = { $path } em { $host }
remote-connected-to = Conectado a { $name }
remote-reconnecting-to = Reconectando a { $name }…
remote-reconnecting-to-attempt = Reconectando a { $name }… (tentativa { $attempt })
remote-the-connection-to-is-lost = A conexão com { $name } foi perdida
remote-disconnected-from = Desconectado de { $name }
remote-the-linux-server-builds-are-missing-run-scripts = Os builds do servidor Linux estão faltando: execute scripts/build-server-linux.sh
remote-connecting-to-3 = Conectando a { $name }
remote-connecting = Conectando
remote-installing-snout-server = Instalando o snout-server
remote-java-engine-on = Mecanismo Java em { $name }
remote-looking-for-the-engine = Procurando o mecanismo
remote-could-not-install-it = Não foi possível instalar: { $e }
remote-open-folder-on = Abrir uma pasta em { $name }
remote-no-folders-here = Nenhuma pasta aqui
remote-show-hidden-folders = Mostrar pastas ocultas

## run

run-is-not-among-the-projects-sources = { $class } não está entre os fontes do projeto
run-the-file-in-front-has-no-tests-main = O arquivo em primeiro plano não tem testes, classe principal nem script para executar
run-nothing-to-run-here-put-the-caret-in = Nada para executar aqui: coloque o cursor em uma classe principal, um teste ou um script
run-root = { $name } (raiz)
run-reading-the-gradle-tasks = Lendo as tarefas Gradle
run-the-builds-are-being-configured = Os builds estão sendo configurados.
run-the-tasks-could-not-be-read = Não foi possível ler as tarefas
run-no-tasks-yet = Ainda não há tarefas
run-the-tasks-appear-once-the-java-engine-has = As tarefas aparecem depois que o mecanismo Java importar o projeto.
run-no-gradle-tasks = Nenhuma tarefa Gradle
run-this-project-has-no-gradle-build = Este projeto não tem build Gradle.
run-maven-wrapper = Maven { $version } (wrapper)
run-lifecycle = Ciclo de vida
run-plugins = Plugins
run-delete-the-build-output-target = Excluir a saída do build (target)
run-check-the-project-is-correct-and-complete = Verificar se o projeto está correto e completo
run-compile-the-main-sources = Compilar os fontes principais
run-run-the-unit-tests = Executar os testes de unidade
run-package-the-compiled-code-a-jar-or-war = Empacotar o código compilado (jar ou war)
run-run-the-checks-on-the-package-integration-tests = Executar as verificações no pacote, incluindo testes de integração
run-put-the-package-in-the-local-repository = Colocar o pacote no repositório local
run-make-the-projects-site = Gerar o site do projeto
run-copy-the-package-to-the-remote-repository = Copiar o pacote para o repositório remoto
run-reading-the-maven-goals = Lendo os goals Maven
run-the-plugins-goals-are-being-read = Os goals dos plugins estão sendo lidos.
run-the-goals-could-not-be-read = Não foi possível ler os goals
run-no-goals-yet = Ainda não há goals
run-the-goals-appear-once-the-java-engine-has = Os goals aparecem depois que o mecanismo Java importar o projeto.
run-no-maven-builds = Nenhum build Maven
run-this-project-has-no-pom-xml = Este projeto não tem pom.xml.
run-offline = Offline
run-run-maven-offline-o-only-the-local-repository = Executar o Maven offline (-o): somente o repositório local
run-skip-tests = Ignorar testes
run-run-goals-without-the-tests-dskiptests = Executar os goals sem os testes (-DskipTests)
run-active-in-the-pom = ativo no POM
run-failed = { $label } (com falha)
run-paused-running = Pausado, em execução há { $time }
run-running = Em execução há { $time }
run-finished-in = Concluído em { $time }
run-failed-after = Falhou após { $time }
run-stopped-after = Parado após { $time }
run-tests-count = { $count ->
    [one] { $parts }, { $count } teste
   *[other] { $parts }, { $count } testes
    }
run-the-java-engine-is-not-ready-yet = O mecanismo Java ainda não está pronto
run-the-java-engine-stopped-and-this-run-with = O mecanismo Java parou, e esta execução junto.
run-could-not-save-the-output-to = Não foi possível salvar a saída em { $path }: { $e }
run-open-a-project-to-run-it = Abra um projeto para executá-lo.
run-has-no-gradle-or-maven-build-runs-tests = { $project } não tem build Gradle ou Maven: execuções, testes e o depurador precisam de um.
run-java-support-is-off = O suporte a Java está desativado.
run-the-java-engine-is-not-running = O mecanismo Java não está em execução: { $why }
run-starting = Iniciando { $c }
run-running-2 = Executando { $t }
run-running-3 = Executando { $config }
run-testing = Testando { $label }
run-running-4 = Executando { $task }
run-running-mvn-in = Executando mvn { $goals } em { $module }
run-attaching-to-the-node-listening-on-port = Anexando ao Node que escuta na porta { $port }
run-waiting-for-the-java-engine-the-run-starts = Aguardando o mecanismo Java ({ $doing }): a execução começa assim que ele estiver pronto.
run-working-directory = Diretório de trabalho: { $cwd }
run-the-java-engine-is-ready = O mecanismo Java está pronto.
run-working-directory-2 = Diretório de trabalho: { $dir }
run-finished = Concluído
run-failed-3 = Falhou
run-stopped = Parado
run-with-exit-code = {" "}com o código de saída { $code }
run-the-scala-compile-failed-see-problems = A compilação Scala falhou: veja Problemas
run-compiling-the-changed-scala = Compilando o Scala alterado
run-reading-the-tasks = Lendo as tarefas
run-the-engine-does-not-know-this-files-module = O mecanismo ainda não conhece o módulo deste arquivo
run-downloading-the-javascript-debugger-js-debug = Baixando o depurador JavaScript (js-debug { $version })
run-the-debugger-could-not-start-the-program = O depurador não conseguiu iniciar o programa: { $e }
run-detached = Desanexado
run-could-not-connect-the-debugger-to-a-node = Não foi possível conectar o depurador a um processo Node: { $e }
run-npm-is-missing-beside-node = O npm não está junto do Node
run-is-not-installed-the-projects-lockfile-is-s = { $name } não está instalado (o lockfile do projeto é do { $name })
run-attach-to-port = Anexar à porta { $port }
run-the-configuration-names-no-script = A configuração não indica nenhum script
run-the-configuration-names-no-file-to-run = A configuração não indica nenhum arquivo para executar
run-is-not-there = { $file } não está lá
run-open-a-project-first = Abra um projeto primeiro
run-no-node-fits-this-project-download-it-from = Nenhum Node serve para este projeto: baixe primeiro pela barra acima do editor.
run-did-not-start = { $program } não iniciou: { $e }
run-process-finished-with-exit-code = Processo concluído com o código de saída { $c }
run-process-ended-by-a-signal = Processo encerrado por um sinal
run-all-tests = Todos os testes
run-tests-in = Testes em { $p }
run-open-a-playwright-project-first = Abra um projeto Playwright primeiro
run-passed = { $passed ->
    [one] { $passed } aprovado
   *[other] { $passed } aprovados
    }
run-skipped = { $skipped ->
    [one] { $skipped } ignorado
   *[other] { $skipped } ignorados
    }
run-failed-2 = { $failed } com falha
tests-watch-label = Observar

## structure

structure-included-build = build incluído
structure-compile = Compilação
structure-runtime = Execução
structure-test = Teste
structure-annotation-processor = Processador de anotações
structure-what-the-main-code-compiles-against-compile-provided = Com o que o código principal compila (escopos compile, provided e system).
structure-what-the-main-code-runs-with-compile-and = Com o que o código principal executa (escopos compile e runtime).
structure-what-the-tests-compile-and-run-with-every = Com o que os testes compilam e executam (todos os escopos).
structure-what-the-main-code-compiles-against-compileclasspath = Com o que o código principal compila (compileClasspath).
structure-what-the-main-code-runs-with-runtimeclasspath = Com o que o código principal executa (runtimeClasspath).
structure-what-the-tests-compile-and-run-with-the = Com o que os testes compilam e executam (o runtimeClasspath do source set de teste).
structure-the-annotation-processors-javac-runs-on-the-main = Os processadores de anotações que o javac executa no código principal.
structure-project = projeto { $path }
structure-the-gradle-model-could-not-be-read = Não foi possível ler o modelo Gradle: { $e }
structure-the-gradle-model-is-not-json = O modelo Gradle não é JSON: { $e }
structure-the-gradle-model-is-of-an-older-format = O modelo Gradle tem um formato antigo: sincronize de novo
structure-edit-in-build-file = Editar no arquivo de build
structure-sync-gradle = Sincronizar Gradle
structure-the-pages-fill-in-as-soon-as-the = As páginas são preenchidas assim que o modelo da última sincronização for lido.
structure-no-gradle-model-yet = Ainda não há modelo Gradle
structure-this-project-has-not-been-synced-as-a = Este projeto não foi sincronizado como build Gradle. Sincronize para ver os módulos e dependências.
structure-the-gradle-model-could-not-be-read-2 = Não foi possível ler o modelo Gradle
structure-java = Java
structure-looking-for-jdks = Procurando JDKs…
structure-not-set-the-jdks-own = Não definido (o do JDK)
structure-java-to-per-module = Java { $lo } a { $hi }, por módulo
structure-language-level = Nível de linguagem
structure-gradle-and-maven = Gradle e Maven
structure-unknown = Desconhecido
structure-did-not-sync = Não sincronizou
structure-maven-version = Versão do Maven
structure-gradle-version = Versão do Gradle
structure-maven-jvm = JVM do Maven
structure-gradle-jvm = JVM do Gradle
structure-not-known-until-the-build-syncs = Desconhecido até o build sincronizar
structure-the-jdk-maven-runs-on-and-compiles-with = O JDK em que o Maven executa e compila, escolhido pelo nível de Java do build: o JDK acima, quando definido.
structure-the-jvm-the-last-sync-ran-gradle-on = A JVM em que a última sincronização executou o Gradle: o JDK acima, quando definido.
structure-included-build-2 = Build incluído
structure-spring-boot = Spring Boot
structure-build-file = Arquivo de build
structure-not-set = Não definido
structure-toolchain = toolchain { $t }
structure-target-compatibility = Compat. de destino
structure-this-project-compiles-no-java-it-has-no = Este projeto não compila Java: não tem source sets.
structure-java-bytecode = Java { $a }, bytecode { $b }
structure-content-roots = Raízes de conteúdo
structure-not-there-yet = ainda não existe
structure-output-folders = Pastas de saída
structure-resources = Recursos
structure-classes = Classes
structure-no-module-compiles-java = Nenhum módulo compila Java
structure-dependencies-show-for-the-projects-that-apply-a = As dependências aparecem nos projetos que aplicam um plugin Java.
structure-conflicts-only = Somente conflitos
structure-no-scope-dependencies = Sem dependências: { $scope }
structure-declares-nothing-for-this-scope = { $m } não declara nada para este escopo. { $m_2 }
structure-declared-in-all = { $declared } declaradas, { $count } no total.
structure-version-conflicts = { $count ->
    [one] { $count } conflito de versões
   *[other] { $count } conflitos de versões
    }
structure-unresolved = { $unresolved_count } não resolvidas
structure-no-version-lost-a-conflict-here = Nenhuma versão perdeu um conflito aqui.
structure-try-fewer-letters-of-the-group-name-or = Tente menos letras do grupo, nome ou versão.
structure-on = em { $h }
structure-on-machine = em { $machine }
structure-download-jdk = Baixar JDK
structure-found-where-jdks-are-installed-and-added-by = Encontrados onde os JDKs são instalados { $machine }, ou adicionados por você.
structure-add-from-folder = Adicionar de uma pasta…
structure-downloading-jdk = Baixando JDK { $major }
structure-no-jdk-found = Nenhum JDK encontrado
structure-no-jdk-is-installed-where-jdks-usually-are = Nenhum JDK instalado { $machine } nos locais de costume. Baixe um ou adicione a pasta de um.
structure-downloaded = Baixado
structure-detected = Detectado
structure-this-projects = O deste projeto
structure-remove-from-the-list = Remover da lista
structure-no-jdk-there-now = Não há mais JDK ali
structure-removing-an-added-jdk-takes-it-off-the = Remover um JDK adicionado o tira da lista; os arquivos continuam onde estão.
structure-selected-jdk = JDK selecionado
structure-version = Versão
structure-home = Local
structure-kept-in = Guardado em
structure-your-settings = Suas configurações
structure-found-by-snoutcode = Encontrado pelo SnoutCode
structure-this-project = Este projeto
structure-runs-on-it = Executa nele
structure-use-for-this-project = Usar neste projeto
structure-not-detected = Não detectado
structure-spring-settings = Configurações do Spring
structure-none-with-the-spring-boot-plugin = Nenhum com o plugin Spring Boot
structure-in-the-spring-model = { $beans } no modelo Spring
structure-read-once-the-java-engine-is-ready = Lido quando o mecanismo Java estiver pronto
structure-profiles-known = Perfis conhecidos
structure-configuration-files = Arquivos de configuração
structure-no-application-properties-or-application-yml-in-the = Nenhum application.properties ou application.yml nos recursos
structure-no-spring-here = Sem Spring aqui
structure-no-module-applies-the-spring-boot-plugin-and = Nenhum módulo aplica o plugin Spring Boot, e o modelo Spring não tem beans.
structure-not-found = Não encontrado
structure-claude-code-settings = Configurações do Claude Code
structure-claude-was-not-found-on-path-or-the = claude não foi encontrado no PATH nem nos locais de costume
structure-ide-connection = Conexão com o IDE
structure-no-session-connected-a-claude-started-in-the = Nenhuma sessão conectada. Um claude iniciado no console se conecta sozinho.
structure-1-session-connected-it-sees-the-open-files = 1 sessão conectada: ela vê os arquivos abertos, a seleção e os problemas
structure-sessions-connected = { $n } sessões conectadas
structure-the-project-as-a-whole-the-jdk-it = O projeto como um todo: você escolhe o JDK, o resto é com o Maven.
structure-every-maven-module-of-every-build-with-its = Todos os módulos Maven de cada build, com source sets, raízes e saídas.
structure-what-each-module-declares-and-what-that-brings = O que cada módulo declara e o que isso traz, conforme o Maven resolveu.
structure-the-project-as-a-whole-the-jdk-it-2 = O projeto como um todo: você escolhe o JDK, o resto é com o Gradle.
structure-every-gradle-project-of-every-build-with-its = Todos os projetos Gradle de cada build, com source sets, raízes e saídas.
structure-what-each-module-declares-and-what-that-brings-2 = O que cada módulo declara e o que isso traz, conforme o Gradle resolveu.
structure-the-jdks-snoutcode-can-run-gradle-and-the = Os JDKs em que o SnoutCode pode executar o Gradle e o mecanismo Java.
structure-frameworks-and-tools-as-they-stand-read-only = Frameworks e ferramentas, como estão. Somente leitura.
structure-unresolved-2 = Não resolvida
structure-show-in-modules = Mostrar { $name } em Módulos
structure-show-in-build-file = Mostrar no arquivo de build
structure-show-the-declaration-that-brings-it-in = Mostrar a declaração que o traz
structure-show-its-dependencies = Mostrar as dependências
structure-a-newer-version-asked-for-elsewhere-won = uma versão mais nova pedida em outro lugar venceu
structure-dependency-management-chose-it = o gerenciamento de dependências a escolheu
structure-it-is-forced = ela é forçada
structure-a-constraint-asks-for-it = uma restrição a pede
structure-another-request-won = outro pedido venceu
structure-was-asked-for-here-is-used = { $asked } foi pedida aqui; { $version } é usada: { $why }
structure-conflict = Conflito
structure-won-over = venceu { $join }
structure-show-in-version-catalog = Mostrar no catálogo de versões
structure-modules-from-sync = { $count ->
    [one] { $name } · { $count } módulo da última sincronização do { $tool }
   *[other] { $name } · { $count } módulos da última sincronização do { $tool }
    }
structure-project-structure = Opções do projeto
structure-syncing = Sincronizando…
structure-reload-maven = Recarregar Maven
structure-read-the-maven-builds-again = Ler os builds Maven novamente
structure-read-the-gradle-builds-again = Ler os builds Gradle novamente
structure-read-the-builds-again = Ler os builds novamente
structure-facets = Facetas
structure-project-settings = Opções do projeto
structure-platform-settings = Plataforma
structure-search = Buscar
structure-filter-dependencies = Filtrar por grupo, nome ou versão
structure-jdk-listed-already = O JDK { $version } já está na lista
structure-jdk-added = JDK { $version } adicionado. Aplicar o mantém.
structure-no-jdk-in = Nenhum JDK em { $dir }: escolha a pasta que contém a pasta bin com o Java
structure-add-jdk = Adicionar JDK
structure-jdk-removed = Removido da lista. Aplicar mantém a alteração.
structure-jdk-downloaded = JDK { $version } baixado em { $home }
structure-download-failed = O download falhou: { $e }
structure-build-file-not-found = O arquivo de build não foi encontrado
structure-syncing-note = Sincronizando o modelo do Gradle. As páginas acompanham quando terminar.

## newproject

newproject-gradle-or-maven = Gradle ou Maven
newproject-from-start-spring-io = De start.spring.io
newproject-end-to-end-tests-in-typescript = Testes E2E em TypeScript
newproject-project-name = Nome do projeto
newproject-folder = Pasta
newproject-search-dependencies = Pesquisar dependências
newproject-waiting-for-start-spring-io = Aguardando start.spring.io…
newproject-start-spring-io-could-not-be-reached = Não foi possível acessar start.spring.io ({ $e }).
newproject-the-project-goes-in = O projeto ficará em { $target }
newproject-asking-start-spring-io-what-it-offers = Perguntando ao start.spring.io o que ele oferece…
newproject-application-url = URL da aplicação
newproject-the-address-the-tests-open-a-base-url = O endereço que os testes abrem. Uma variável de ambiente BASE_URL o substitui quando definida.
newproject-create-a-git-repository = Criar um repositório Git
newproject-build-tool = Ferramenta de build
newproject-coordinates = Coordenadas
newproject-none-chosen-yet = Nenhuma escolhida ainda.
newproject-no-dependency-matches = Nenhuma dependência corresponde.
newproject-dependencies = Dependências
newproject-spring-boot-java-choose-what-the-application-uses = Spring Boot { $version }, Java { $java }. Escolha o que a aplicação usa; dá para adicionar mais ao build depois.
newproject-getting-the-project-from-start-spring-io = Obtendo o projeto de start.spring.io…
newproject-creating-the-project = Criando o projeto…
newproject-new-project = Novo projeto
newproject-a-java-or-spring-boot-project-built-with = Um projeto Java ou Spring Boot, compilado com Gradle ou Maven.
newproject-create-without-the-network = Criar sem rede
newproject-previous = Anterior
newproject-next = Próximo
newproject-already-exists-and-is-not-empty = { $dir } já existe e não está vazio.
newproject-give-the-project-a-name = Dê um nome ao projeto.
newproject-the-name-cannot-start-with-a-dot-or = O nome não pode começar com ponto nem conter / : ou \.
newproject-choose-where-the-project-goes = Escolha onde o projeto fica.
newproject-the-group-is-a-dotted-name-such-as = O grupo é um nome com pontos, como com.example.
newproject-the-artifact-holds-letters-digits-dots-dashes-and = O artefato contém letras, dígitos, pontos, hifens e sublinhados.
newproject-the-package-is-a-dotted-java-name-such = O pacote é um nome Java com pontos, como com.example.demo.
newproject-the-applications-address-starts-with-http-or-https = O endereço da aplicação começa com http:// ou https://.

## runwin

runwin-evaluate-an-expression = Avaliar uma expressão
runwin-clear-all = Limpar tudo
runwin-copy-all = Copiar tudo
runwin-fix-with = Corrigir com { $agent }
runwin-save-output-to-file = Salvar a saída em um arquivo…
runwin-rerun-r = Executar novamente (⌘R)
runwin-rerun-failed-tests-r = Executar novamente os testes com falha (⌥⌘R)
runwin-test-results = Resultados
runwin-passed = Aprovado
runwin-skipped = Ignorado
runwin-nothing-printed = { $what }; nada impresso
runwin-expected = Esperado
runwin-actual = Obtido
runwin-errors-warnings = { $e } erros, { $w } avisos
runwin-up-to-date = atualizado
runwin-from-cache = do cache
runwin-no-source = sem fonte
runwin-debugger = Depurador
runwin-resume-f9 = Retomar (F9)
runwin-pause-program-f9 = Pausar o programa (⌥⌘F9)
runwin-step-over-f8 = Contornar (F8)
runwin-step-into-f7 = Intervir (F7)
runwin-step-out-f8 = Sair da função (⇧F8)
runwin-the-program-ended = O programa terminou.
runwin-waiting-for-the-program-to-start = Aguardando o programa iniciar…
runwin-the-program-is-running-breakpoints-stop-it-here = O programa está em execução. Os pontos de interrupção o param aqui.
runwin-add-to-watches = Adicionar às inspeções (⇧↩)
runwin-nothing-running = Nada em execução
runwin-run-a-configuration-from-the-title-bar-or = Execute uma configuração pela barra de título, ou um teste pela margem.
runwin-no-debug-session = Nenhuma sessão de depuração
runwin-debug-a-configuration-from-the-title-bar-or = Depure uma configuração pela barra de título, ou um teste pela margem.
runwin-no-build-output = Nenhuma saída de build
runwin-build-the-project-or-run-one-of-its = Compile o projeto ou execute uma tarefa Gradle ou um goal Maven.
runwin-run-task = Executar tarefa…

## spring

spring-any-profile = Qualquer perfil
spring-from-the-files = { $config_active } (conforme os arquivos)
spring-text = { $name } ({ $from })
spring-consumes = {"\u000A"}Consome { $join }
spring-produces = {"\u000A"}Produz { $join }
spring-bean-not-active-with-the-active-profiles = Bean '{ $b }' ({ $stereotype_label }): inativo com os perfis ativos
spring-not-injected-anywhere = não injetado em lugar nenhum
spring-injected-in-1-place = injetado em 1 lugar
spring-injected-in-places = injetado em { $n } lugares
spring-bean = Bean '{ $b }' ({ $stereotype_label }), { $used }
spring-text-2 = { $what }: { $value }
spring-bean-2 = { $what }: bean '{ $one }'
spring-no-bean = { $what }: nenhum bean
spring-beans = { $what }: { $count } beans
spring-all-sources = Todas as fontes
spring-default-package = (pacote padrão)
spring-search-beans-by-name-or-type = Pesquisar beans por nome ou tipo
spring-search-paths-and-handlers = Pesquisar caminhos e handlers
spring-all-stereotypes = Todos os estereótipos
spring-no-profile = Sem perfil
spring-profile = Perfil { $p }
spring-inactive = { $inactive } inativos
spring-by-package = Por pacote
spring-by-stereotype = Por estereótipo
spring-no-beans = Nenhum bean
spring-the-project-declares-no-spring-bean-yet = O projeto ainda não declara nenhum bean Spring.
spring-no-bean-matches = Nenhum bean corresponde
spring-try-fewer-letters-or-another-stereotype-or-source = Tente menos letras, ou outro estereótipo ou fonte.
spring-beans-from-libraries = { $count } beans, { $library } de bibliotecas
spring-not-injected-anywhere-2 = Não injetado em lugar nenhum
spring-injected-in-1-place-2 = Injetado em 1 lugar
spring-injected-in-places-2 = Injetado em { $n } lugares
spring-profiles = {"\u000A"}Perfis: { $join }
spring-not-active-note = {"\u000A"}Inativo: os perfis ativos o deixam de fora
spring-primary = Principal
spring-of-beans = { $shown } de { $bean_count } beans
spring-method-all = Todos
spring-method = Método
spring-path = Caminho
spring-handler = Handler
spring-no-endpoints = Nenhum endpoint
spring-no-controller-of-the-project-maps-a-request = Nenhum controlador do projeto mapeia uma requisição ainda.
spring-no-endpoint-matches = Nenhum endpoint corresponde
spring-try-fewer-letters-of-the-path-or-the = Tente menos letras do caminho ou do método.
spring-of-endpoints = { $shown } de { $total } endpoints
spring-as-runs = Como '{ $c }' executa ({ $join })
spring-read-the-spring-model = Ler o modelo Spring
spring-reading-the-spring-model = Lendo o modelo Spring
spring-the-beans-and-endpoints-are-being-read = Os beans e endpoints estão sendo lidos.
spring-no-spring-model-yet = Ainda não há modelo Spring
spring-beans-and-endpoints-appear-once-the-java-engine = Beans e endpoints aparecem depois que o mecanismo Java importar o projeto.
spring-beans-2 = Beans { $bean_count }
spring-endpoints = Endpoints { $count }
spring-reading = Lendo…
spring-data-repository = Repositório de dados

## config

config-arguments-the-program-gets = Argumentos que o programa recebe
config-9229-empty-run-the-file = 9229 (vazio: executar o arquivo)
config-unnamed = Sem nome { $next }
config-no-configuration-add-one-with = Nenhuma configuração. Adicione uma com +.
config-choose-a-module = Escolher um módulo
config-kind = Tipo
config-main-class = Classe principal
config-goals = Goals
config-script = Script
config-file = Arquivo
config-task = Tarefa
config-gradle-arguments = Argumentos do Gradle
config-maven-arguments = Argumentos do Maven
config-playwright-arguments = Argumentos do Playwright
config-script-arguments = Argumentos do script
config-program-arguments = Argumentos do programa
config-node-options = Opções do Node
config-vm-options = Opções da VM
config-environment-variables = Variáveis de ambiente
config-attach-to-port = Anexar à porta
config-from-the-build-with-your-changes-remove-puts = Do build, com suas alterações. Remover a devolve como o build a tem.
config-from-the-build-a-change-you-make-stands = Do build. Uma alteração sua a substitui.
config-your-own-configuration = Sua própria configuração.
config-run-configurations = Configurações de execução
config-what-run-and-debug-start-an-application-a = O que Executar e Depurar iniciam: uma aplicação, uma tarefa Gradle, testes, um script.
config-add-configuration = Adicionar configuração
config-remove-or-put-back-as-the-build-has = Remover ou devolver como o build a tem
config-application = Aplicação
config-package-script = Script do pacote
config-scripts = Scripts

## tests

tests-filter-tests = Filtrar testes
tests-not-run-yet = Ainda não executado
tests-did-not-start = Não iniciou
tests-workers = { $count ->
    [one] { $projects } · { $count } worker
   *[other] { $projects } · { $count } workers
    }
tests-about-left = { $elapsed } · faltam cerca de { $left }
tests-live-run = Execução ao vivo
tests-history = Histórico
tests-run-with-the-browsers-showing-one-test-at = Executar com os navegadores visíveis, um teste por vez
tests-rerun-failed = Repetir falhas
tests-run-all = Executar todos
tests-no-environment = Sem ambiente
tests-environment = Ambiente: { $label }, { $d }
tests-environment-2 = Ambiente: { $label }
tests-no-environment-the-configurations-own = Sem ambiente (o da configuração)
tests-all = Todos { $total }
tests-failed = Falhas { $failed }
tests-flaky = Instáveis { $flaky }
tests-slow-over-4-s = Lentos, mais de 4 s
tests-last-10-runs = Últimas 10
tests-flaky-2 = Instável
tests-finding-the-tests = Procurando os testes…
tests-no-test-matches-the-filter = Nenhum teste corresponde ao filtro.
tests-no-tests-yet-run-them-or-add-a = Ainda não há testes. Execute-os ou adicione um arquivo de spec.
tests-failed-in = Falhou em { $project }
tests-flaky-in = Instável em { $project }
tests-in = { $other } em { $project }
tests-retries = { $count ->
    [one]  · { $count } nova tentativa
   *[other]  · { $count } novas tentativas
    }
tests-screenshot-comparison = Comparação de capturas
tests-difference = Diferença
tests-accept-new-screenshot = Aceitar a nova captura
tests-runs-the-test-again-keeping-its-screenshots-as = Executa o teste de novo, mantendo as capturas como estão agora
tests-steps = Passos
tests-from-the-trace = Do trace
tests-console-error = erro de console
tests-actions-no-failed-requests-or-console-errors = { $count } ações; nenhuma requisição com falha nem erro de console.
tests-the-trace-could-not-be-read = Não foi possível ler o trace: { $why }
tests-reading-the-trace = Lendo o trace…
tests-open-full-trace = Abrir o trace completo
tests-video = Vídeo
tests-run-this-test = Executar este teste
tests-debug-this-test = Depurar este teste
tests-debug-in-inspector = Depurar no inspetor
tests-show-in-editor = Mostrar no editor
tests-copy-error = Copiar o erro
tests-when = Quando
tests-what = O quê
tests-branch = Branch
tests-time = Tempo
tests-new-since-the-run-before = Novo desde a execução anterior
tests-runs-of-this-project = Execuções deste projeto
tests-kept-on-this-machine-outside-the-repository = Guardadas nesta máquina, fora do repositório
tests-no-runs-kept-yet = Nenhuma execução guardada ainda.
tests-min-ago = há { $secs } min
tests-h-ago = há { $secs } h
tests-days-ago = há { $days } dias
tests-the-playwright-test-runner-is-not-installed = O executor de testes do Playwright não está instalado.
tests-no-node-fits-this-project = Nenhum Node serve para este projeto.
tests-trace-viewer = Visualizador de trace
tests-no-tests-found = Nenhum teste encontrado
tests-open-a-playwright-project-or-a-project-whose = Abra um projeto Playwright, ou um cujos pacotes testem com Vitest ou Jest, para executar os testes aqui.
tests-in-2 = { $runner } em { $rel }
tests-is-not-installed-install-the-packages-first = { $runner } não está instalado: instale os pacotes primeiro.
tests-the-snoutcode-reporter-could-not-be-written = Não foi possível gravar o relator do SnoutCode: { $e }
tests-tests = { $count } testes
tests-failed-tests = Testes com falha
tests-the-playwright-test-runner-is-not-installed-install = O executor de testes do Playwright não está instalado: instale os pacotes primeiro.
tests-playwright-listed-no-tests = O Playwright não listou nenhum teste ({ $status })
tests-recording = Gravação
tests-ui-mode = Modo UI
tests-the-snoutcode-locator-picker-could-not-be-written = Não foi possível gravar o seletor de localizadores do SnoutCode: { $e }
tests-new-since-the-last-run = {" "}({ $fresh } novos desde a última execução)
tests-passed-failed = { $title }: { $passed } aprovados, { $failed } com falha{ $new }{ $new_2 }
tests-count-passed = { $count ->
    [one] { $count } aprovado
   *[other] { $count } aprovados
    }
tests-count-failed = { $count ->
    [one] { $count } com falha
   *[other] { $count } com falha
    }
tests-count-flaky = { $count ->
    [one] { $count } instável
   *[other] { $count } instáveis
    }
tests-count-skipped = { $count ->
    [one] { $count } ignorado
   *[other] { $count } ignorados
    }
tests-flaky-3 = , { $flaky ->
    [one] { $flaky } instável
   *[other] { $flaky } instáveis
    }

## env

env-environment = Ambiente { $count }
env-no-environment-add-one-with = Nenhum ambiente. Adicione um com +.
env-variables = Variáveis
env-in-use = em uso
env-where-the-tests-point-the-applications-url-and = Para onde os testes apontam: a URL da aplicação e as variáveis que leem.
env-add-environment = Adicionar ambiente
env-remove-environment = Remover ambiente

## pwsetup

pwsetup-ffmpeg-for-videos = FFmpeg (para os vídeos)
pwsetup-playwright-not-installed = Playwright { $v }, não instalado
pwsetup-no-node-fits = nenhum Node serve
pwsetup-install-packages = Instalar pacotes
pwsetup-install-playwright-browsers = Instalar os navegadores do Playwright
pwsetup-no-node-fits-this-project-download-it-from = Nenhum Node serve para este projeto: baixe primeiro pela barra acima.
pwsetup-the-install-did-not-finish-see-the-build = A instalação não terminou: veja a janela de build.

## pwreport

pwreport-queued = Na fila
pwreport-flaky-passed-on-a-retry = Instável, passou em uma nova tentativa
pwreport-interrupted = Interrompido

## processes

processes-snoutcode-the-window-the-editors-and-git = SnoutCode: a janela, os editores e o Git
processes-idle-for-min-stops-after = Inativo há { $m } min, para após { $limit }
processes-idle-stops-after-min = Inativo, para após { $limit } min
processes-java-engine-for-reads-the-gradle-or-maven = Mecanismo Java de { $project }: lê o build Gradle ou Maven, executa e testa o código
processes-java-language-server-for-completion-errors-navigation = Servidor de linguagem Java de { $project }: autocompletar, erros, navegação
processes-maven-for-runs = Maven de { $project }: executa { $title }
processes-maven-for-runs-goals = Maven de { $project }: executa goals
processes-tests-of-the-test-jvm-maven-started = Testes de { $title }: a JVM de teste iniciada pelo Maven
processes-tests-for-the-test-jvm-maven-started = Testes de { $project }: a JVM de teste iniciada pelo Maven
processes-your-program-started-by-maven = { $title }: seu programa, iniciado pelo Maven
processes-program-maven-started-for = Programa iniciado pelo Maven para { $project }
processes-your-program-being-debugged = { $title }: seu programa, em depuração
processes-your-program-started-with-run = { $title }: seu programa, iniciado com Executar
processes-program-started-by-the-java-engine-for = Programa iniciado pelo mecanismo Java para { $project }
processes-gradle-daemon-for-syncs-the-build-runs-tasks = Daemon Gradle { $version } de { $project }: sincroniza o build, executa tarefas e testes
processes-tests-of-the-test-jvm-gradle-started = Testes de { $title }: a JVM de teste iniciada pelo Gradle
processes-tests-for-the-test-jvm-gradle-started = Testes de { $project }: a JVM de teste iniciada pelo Gradle
processes-gradle-worker-for-compiles-or-processes-in-a = Worker Gradle de { $project }: compila ou processa em uma JVM própria
processes-your-program-started-by-gradle = { $title }: seu programa, iniciado pelo Gradle
processes-program-gradle-started-for = Programa iniciado pelo Gradle para { $project }
processes-in = {" "}em { $p }
processes-java-engine-for-stopping-with-its-language-server = Mecanismo Java de { $folder }: parando, com o servidor de linguagem e os daemons Gradle
processes-terminal-your-shell = Terminal: seu shell { $name }{ $within }
processes-claude-code-the-assistant-in-the-console = Claude Code: o assistente no console
processes-started-by-snoutcode = { $name }: iniciado pelo SnoutCode
processes-java-engine-for-left-over-from-an-earlier = Mecanismo Java de { $folder }, sobra de uma sessão anterior
processes-java-language-server-left-over-from-an-earlier = Servidor de linguagem Java, sobra de uma sessão anterior
processes-gradle-daemon-left-over-from-an-earlier-session = Daemon Gradle { $version }, sobra de uma sessão anterior
processes-nothing-idle-to-stop = Nada inativo para parar.
processes-stopped-1-idle-process = 1 processo inativo parado.
processes-stopped-idle-processes = { $n } processos inativos parados.
processes-state-building = Compilando
processes-state-stopping = Parando
processes-state-left-over = Sobra
processes-looking-at-the-processes = Examinando os processos
processes-snoutcodes-own-processes-show-here-the-java-engine = Os processos do SnoutCode aparecem aqui: mecanismo Java, daemons Gradle, Maven, execuções e terminais.
processes-process = Processo
processes-memory = Memória
processes-uptime = Tempo ativo
processes-project = { $purpose }{"\u000A"}Projeto: { $project }
processes-busy-at-work-now-a-filled-dot = Ocupado: trabalhando agora (ponto cheio)
processes-waiting-alive-doing-nothing-now-a-hollow-dot = Aguardando: ativo, sem fazer nada agora (ponto vazio)
processes-summary = { $count ->
    [one] { $count } processo, { $memory } de memória, { $cpu }% de CPU
   *[other] { $count } processos, { $memory } de memória, { $cpu }% de CPU
    }
processes-idle = { $idle } inativos

## editor

editor-tab = Tab, { $width }
editor-spaces = { $width } espaços
editor-could-not-read = Não foi possível ler { $path }: { $e }
editor-could-not-save = Não foi possível salvar { $path }: { $e }
editor-this-file-changed-on-disk-and-this-tab = Este arquivo mudou no disco, e esta aba tem alterações não salvas.
editor-reload-from-disk = Recarregar do disco
editor-keep-my-changes = Manter minhas alterações
editor-run = Executar '{ $name }'
editor-debug = Depurar '{ $name }'
editor-do-you-want-to-save-the-changes-you = Deseja salvar as alterações feitas em { $name }?
editor-a-librarys-class-file-has-no-git-history = O arquivo de classe de uma biblioteca não tem histórico Git.
editor-not-a-git-project-no-git-repository-holds = Não é um projeto Git: nenhum repositório Git contém este arquivo.
editor-not-committed-yet = Ainda sem commit
editor-uncommitted = Sem commit
editor-reading-history = Lendo o histórico…
editor-close-annotations = Fechar anotações
editor-breakpoint-condition = Condição do ponto de interrupção…
editor-show-diff = Mostrar diff
editor-show-in-git-log = Mostrar no histórico do Git
editor-copy-hash = Copiar hash
editor-copied = { $count } copiado
editor-annotate-previous-revision = Anotar a revisão anterior
editor-the-file-as-it-was-before-this-commit = O arquivo como era antes deste commit
editor-this-commit-brought-the-file-in = Este commit trouxe o arquivo
editor-you-changed-this-line-since-the-last-commit = Você alterou esta linha desde o último commit. Faça o commit e a coluna indicará.
editor-authors-of = Autores de { $decl }
editor-committed-lines = { $total } linhas com commit
editor-last = { $email } · último { $now }
editor-1-line = 1 linha
editor-lines = { $lines } linhas
editor-before = antes de { $short_id }
editor-now = Agora
editor-back = Voltar
editor-back-to-now = Voltar ao presente
editor-you-not-committed-yet = Você, ainda sem commit
editor-1-line-removed = 1 linha removida
editor-lines-removed = { $n } linhas removidas
editor-writer-editing-line = { $writer } edita a linha { $line } { $arrow }
editor-writer-edited-line = { $writer } editou a linha { $line } { $arrow }
editor-claude-code-is-writing-a-change = O Claude Code está escrevendo uma alteração…
editor-claude-code-proposes-a-new-file = O Claude Code propõe um novo arquivo
editor-claude-code-proposes-a-change = O Claude Code propõe uma alteração
editor-more = mais { $waiting }
editor-reject = Rejeitar
editor-accept = Aceitar
editor-claudes-changes-all-reviewed = Alterações do Claude, todas revisadas
editor-claude-changed-this-file-1-change-to-review = O Claude alterou este arquivo: 1 alteração para revisar
editor-claude-changed-this-file-changes-to-review = O Claude alterou este arquivo: { $n } alterações para revisar
editor-next = ⌃⌥⇧↓ próximo
editor-revert-file = Reverter arquivo
editor-accept-file = Aceitar arquivo
editor-hide-the-inline-changes = Ocultar as alterações em linha
editor-changed-by-at = Alterado por { $short } às { $t }
editor-changed-by = Alterado por { $short }
editor-lines-taken-out-here = Linhas retiradas aqui
editor-line = Linha { $start }
editor-lines-2 = Linhas { $start }–{ $end }
editor-reviewed = Revisado
editor-revert = Reverter
editor-ask-to-fix = Pedir ao { $agent } para corrigir
editor-context-actions = Ações de contexto
editor-looking-for-fixes = Procurando correções…
editor-the-symbol = o símbolo
editor-rename-to-enter-renames-everywhere = Renomear para (Enter renomeia em todo lugar)
editor-go-to-declaration-needs-the-java-engine-for = Ir para declaração precisa do mecanismo Java para este arquivo.
editor-no-declaration-found-here = Nenhuma declaração encontrada aqui.
editor-usages-of = Usos de { $word }
editor-no-usages-of-found = Nenhum uso de { $word } encontrado.
editor-implementations-of = Implementações de { $word }
editor-no-implementations-of-found = Nenhuma implementação de { $word } encontrada.
editor-this-needs-the-java-engine-for-this-file = Isto precisa do mecanismo Java para este arquivo.
editor-found = { $count } encontrados
editor-group-in-the-local-repository = grupo no repositório local
editor-artifact-in-the-local-repository = artefato no repositório local
editor-version-in-the-local-repository = versão no repositório local
editor-ask-to-fix-2 = Pedir ao { $short } para corrigir
editor-more-actions = Mais ações…
editor-the-file-changed-meanwhile-try-fix-all-again = O arquivo mudou nesse meio-tempo: tente Corrigir tudo de novo
editor-none-of-these-problems-could-be-fixed-this = Nenhum desses problemas pôde ser corrigido assim
editor-fixed-of-problems-the-rest-need-a-look = { $n } de { $count } problemas corrigidos; o resto precisa de atenção
editor-no-problem-at-the-caret = Nenhum problema no cursor
editor-no-quick-fix-for-this-problem-for-more = Nenhuma correção rápida para este problema: { $more } para mais ações
editor-no-actions-here = Nenhuma ação aqui
editor-recording-but-the-recorded-steps-were-edited-here = Gravando, mas os passos gravados foram editados aqui: o resto fica na janela do gravador.
editor-no-steps-yet = nenhum passo ainda
editor-1-step = 1 passo
editor-steps = { $n } passos
editor-recording-into-this-file-act-in-the-browser = Gravando neste arquivo: { $n }. Aja no navegador e depois pare ou feche-o.
editor-picking-locators-click-an-element-in-the-browser = Escolhendo localizadores: clique em um elemento no navegador para colocar o localizador no cursor.
editor-picking-locators-picked-click-another-or-stop = Escolhendo localizadores: { $picked } escolhidos. Clique em outro ou pare.
editor-cannot-extract-a-method-here-select-whole-statements = Não é possível extrair um método aqui: selecione instruções inteiras ou uma expressão
editor-cannot-extract-a-variable-here-select-an-expression = Não é possível extrair uma variável aqui: selecione uma expressão
editor-cannot-extract-a-constant-here-select-an-expression = Não é possível extrair uma constante aqui: selecione uma expressão de constantes, como um literal
editor-cannot-extract-a-field-here-select-an-expression = Não é possível extrair um campo aqui: selecione uma expressão
editor-nothing-to-inline-here-put-the-caret-on = Nada para embutir aqui: coloque o cursor em uma variável local, um método ou uma constante
editor-a-library-class-is-read-only = Uma classe de biblioteca é somente leitura
editor-replace-all-occurrences = Substituir todas as ocorrências
editor-replace-this-occurrence-only = Substituir somente esta ocorrência
editor-breakpoint-on-line = Ponto de interrupção na linha { $line }
editor-filter-by-class-member-or-file = Filtrar por classe, membro ou arquivo
editor-show-in-endpoints = Mostrar em Endpoints
editor-nothing-matches = Nada encontrado.
editor-of = { $matching } de { $total }
editor-bean = Bean '{ $b }'
editor-bean-is-injected-into = Bean '{ $b }' injetado em
editor-takes = '{ $name }' recebe { $value }
editor-no-bean-for = Nenhum bean para '{ $name }'
editor-press-for-quick-fixes = { $why }. Pressione ⌘. para correções rápidas.
editor-could-take-any-of = '{ $name }' poderia receber qualquer um de
editor-takes-2 = '{ $name }' recebe
editor-takes-the-bean = '{ $name }' recebe o bean
editor-fix-all-in-file = Corrigir tudo “{ $title }” no arquivo
editor-supertypes = Supertipos
editor-subtypes = Subtipos
editor-callers = Chamadores
editor-callees = Chamados
editor-put-the-caret-on-a-type-or-a = Coloque o cursor em um tipo ou método, depois ⌃H ou ⌃⌥H.
editor-put-the-caret-on-a-class-or-an = Coloque o cursor em uma classe ou interface para ver a hierarquia.
editor-put-the-caret-on-a-method-for-its = Coloque o cursor em um método para ver as chamadas.
editor-type-hierarchy-of = Hierarquia de tipos de
editor-calls-of = Chamadas de
editor-reading-the-hierarchy = Lendo a hierarquia
editor-the-java-engine-is-looking-it-up = O mecanismo Java está procurando.
editor-no-hierarchy-yet = Ainda não há hierarquia
editor-no-hierarchy = Sem hierarquia
editor-needs-jdk = Precisa do JDK { $v }
editor-importing-gradle-model = Importando o modelo Gradle
editor-restarting = Reiniciando
editor-syncing-the-build-model = Sincronizando o modelo de build
editor-generating-sources = Gerando fontes
editor-importing = Importando
editor-of-modules-ready = { $ready } de { $n } módulos prontos
editor-java = Java: { $summary }
editor-java-2 = Java: { $what }
editor-java-3 = Java: { $text }
editor-the-java-engine-is-not-built-run-snout = O mecanismo Java não está compilado (execute snout.sh)
editor-the-java-engine-did-not-start = O mecanismo Java não iniciou: { $e }
editor-the-java-engine-keeps-stopping = O mecanismo Java continua parando
editor-references-to = Referências a { $name }
editor-find-references-f12-lists-them-here = Localizar referências (⇧F12) as lista aqui
editor-results = { $count ->
    [one] { $count } resultado
   *[other] { $count } resultados
    }
editor-title-results = { $title }: { $results }
editor-no-usages-found = Nenhum uso encontrado
editor-no-usages-yet = Ainda não há usos
editor-find-usages-on-a-symbol-lists-every-place = Localizar usos em um símbolo lista cada lugar onde ele é usado.
editor-find-usages-at-the-caret = Localizar usos no cursor
editor-filter-problems = Filtrar problemas
editor-show = Mostrar { $what }
editor-expand-all = Expandir tudo
editor-collapse-all = Recolher tudo
editor-no-problems = Nenhum problema
editor-nothing-to-fix-in-the-files-the-java = Nada para corrigir nos arquivos verificados pelo mecanismo Java.
editor-no-problems-match = Nenhum problema corresponde
editor-clear-the-filter-or-turn-on-more-of = Limpe o filtro ou ative mais erros, avisos e informações acima.
editor-filter-by-name = Filtrar por nome
editor-sorted-by-name-click-for-file-order = Ordenado por nome (clique para a ordem do arquivo)
editor-sort-by-name = Ordenar por nome
editor-no-structure = Sem estrutura
editor-reading-the-structure = Lendo a estrutura
editor-try-fewer-letters-of-the-name = Tente menos letras do nome.
editor-nothing-declared = Nada declarado
editor-the-file-declares-no-class-field-or-method = O arquivo não declara nenhuma classe, campo ou método.
editor-package-private = privado do pacote
editor-finding-node = Procurando o Node
editor-needs-node = Precisa do Node { $what }
editor-installing-the-language-server = Instalando o servidor de linguagem
editor-typescript = TypeScript: { $what }
editor-or-newer = { $m } ou mais recente
editor-the-typescript-language-server-did-not-start = O servidor de linguagem TypeScript não iniciou: { $e }
editor-the-typescript-language-server-failed = O servidor de linguagem TypeScript falhou: { $e }
editor-the-typescript-language-server-keeps-stopping = O servidor de linguagem TypeScript continua parando
editor-open-a-java-file = Abra um arquivo Java para ver a estrutura.
editor-java-engine-reading = O mecanismo Java está lendo o arquivo.
editor-problems-title = Problemas
editor-downloading-java-for-the-engine = Baixando Java { $ENGINE_JAVA } para o mecanismo
editor-copying-the-java-engine = Copiando o mecanismo Java
editor-java-engine-install-failed = Não foi possível instalar o mecanismo Java: { $why }
editor-ts-install-failed = Não foi possível instalar o servidor de linguagem TypeScript: { $why }

## blame

blame-month-1 = janeiro
blame-month-2 = fevereiro
blame-month-3 = março
blame-month-4 = abril
blame-month-5 = maio
blame-month-6 = junho
blame-month-7 = julho
blame-month-8 = agosto
blame-month-9 = setembro
blame-month-10 = outubro
blame-month-11 = novembro
blame-month-12 = dezembro
blame-long-date = { $day } de { $month } de { $year } às { $time }

## agents

agents-turn = Turno { $id }
agents-files = { $files } arquivos
agents-1-file = 1 arquivo
agents-revert-what-this-prompt-changed = Reverter o que este prompt alterou
agents-accept-what-this-prompt-changed = Aceitar o que este prompt alterou
agents-editing = Editando
agents-waiting-for-your-review = Aguardando sua revisão
agents-claude-asks-to-make-this-change = O Claude pede para fazer esta alteração
agents-waiting-behind-another-change-to-this-file = Aguardando atrás de outra alteração neste arquivo
agents-1-file-changed = 1 arquivo alterado
agents-files-changed = { $files } arquivos alterados
agents-revert-all = Reverter tudo
agents-put-back-every-file-as-it-was-before = Devolver cada arquivo como estava antes de o Claude alterá-lo
agents-accept-all = Aceitar tudo
agents-keep-every-change-the-files-leave-this-list = Manter todas as alterações; os arquivos saem desta lista
agents-seen-on-disk-while-claude-code-was-connected = Visto no disco enquanto o Claude Code estava conectado; os hooks dele não informaram
agents-revert-the-file = Reverter o arquivo
agents-accept-the-file = Aceitar o arquivo
agents-lines-removed = { $count } linhas removidas
agents-revert-this-change = Reverter esta alteração
agents-accept-this-change = Aceitar esta alteração
agents-no-changes-from-agents-yet = Ainda não há alterações de agentes
agents-the-files-an-agent-changes-show-here-as = Os arquivos que um agente altera aparecem aqui enquanto ele trabalha, cada alteração para aceitar ou reverter.
agents-open-the-agent-console = Abrir o console do agente
agents-a-new-file-the-agent-created-it-reverting = Um novo arquivo: o agente o criou; reverter o exclui
agents-sent-line-to = { $name } linha { $a } enviada ao { $agent }
agents-sent-lines-to = { $name } linhas { $a }–{ $b } enviadas ao { $agent }
agents-sent-to = { $name } enviado ao { $agent }
agents-asked-to-fix-line = Pediu ao { $short } para corrigir { $name } linha { $a }
agents-asked-to-fix-lines = Pediu ao { $short } para corrigir { $name } linhas { $a }–{ $b }
agents-snoutcode-cannot-reach-the-agent-in-this-window = O SnoutCode não consegue falar com o agente nesta janela. Feche o projeto e abra de novo.
agents-asked-for-the-commit-message = Pediu ao { $name } a mensagem de commit
agents-asked-to-fix-the-sign-in = Pediu ao { $name } para corrigir o login do Git
agents-asked-to-fix-the-run = Pediu ao { $name } para corrigir { $title }
agents-claude-is-waiting-for-your-review = O Claude aguarda sua revisão
agents-is-editing = { $short } está editando
agents-waiting = Aguardando
agents-done = Concluído
agents-no-calls-yet = Ainda não há chamadas
agents-what-an-agent-in-the-console-asks-of = O que um agente no console pede ao SnoutCode aparece aqui.
agents-open-a-file-to-insert-into = Abra um arquivo para inserir
agents-insert = Inserir
agents-run-in-terminal = Executar no terminal
agents-nothing-shown-yet = Nada exibido ainda
agents-what-an-agent-in-the-console-chooses-to = O que um agente no console escolhe mostrar aparece aqui, com botões no código.
agents-from = { $title }, de { $agent }
agents-from-2 = De { $agent }
agents-all = Os agentes
agents-two = { $a } e { $b }
agents-n = { $n } agentes
agents-changed-files = { $count ->
    [one] { $who }: 1 arquivo alterado
   *[other] { $who }: { $count } arquivos alterados
    }
agents-seen = visto
agents-new = novo
project-shared = compartilhado

## keys

keys-page-up = Page Up
keys-page-down = Page Down
keys-backspace = Backspace

## json

json-true-or-false = true ou false
json-a-whole-number = um número inteiro
json-a-number = um número
json-a-string = uma string
json-a-list = uma lista
json-an-object = um objeto
json-from-to = { $only } de { $min } a { $max }
json-any-value = qualquer valor
json-or = { $join } ou { $last }
json-default = {"\u000A"}{"\u000A"}Padrão: `{ $shown }`
json-unexpected-text-after-the-settings = Texto inesperado depois das configurações
json-the-settings = As configurações
json-takes = { $what } aceita { $s }
json-is-at-least = { $what } é no mínimo { $min }
json-is-at-most = { $what } é no máximo { $max }
json-unknown-setting-snoutcode-ignores-it = Ajuste desconhecido `{ $key }`: o SnoutCode o ignora
json-needs = { $what } precisa de `{ $req }`
json-a-value-is-missing-here = Falta um valor aqui
json-a-key-in-double-quotes-is-expected-here = Uma chave entre aspas duplas é esperada aqui
json-a-comma-or-a-closing-brace-is-expected = Uma vírgula ou chave de fechamento é esperada aqui
json-a-comma-or-a-closing-bracket-is-expected = Uma vírgula ou colchete de fechamento é esperado aqui
json-this = isto
json-is-not-a-json-value-strings-take-double = { $shown } não é um valor JSON (strings usam aspas duplas)
json-this-string-cannot-be-read = Não é possível ler esta string: { $e }
json-this-string-has-no-closing-quote = Esta string não tem aspas de fechamento
json-colon-expected = São esperados dois-pontos depois de "{ $key }"

## core

core-head-to-worktree = HEAD ↔ Árvore de trabalho
core-index-to-worktree = Índice ↔ Árvore de trabalho
core-head-to-index = HEAD ↔ Índice
core-index = Índice
core-scope-message-only = somente a mensagem
core-scope-nothing = nada para commitar
core-what-staged = as alterações preparadas
core-what-changed-file = o arquivo alterado
core-what-all-changed = todos os { $n } arquivos alterados
core-what-selected-file = o arquivo selecionado
core-what-selected = os { $n } arquivos selecionados
core-what-message = a nova mensagem do último commit
core-what-nothing = nada
core-state-staged = { $state } (preparado)
core-state-partly-staged = { $state } (parcialmente preparado)
core-state-unchanged = Inalterado
core-branch-dash = Um nome de branch não pode começar com hífen.
core-branch-dots = Um nome de branch não pode conter “..” nem “@{"{"}”.
core-branch-chars = Um nome de branch não pode conter ~ ^ : ? * [ nem \.
core-branch-ends = Um nome de branch não pode começar com “/” nem terminar com “/”, “.” ou “.lock”.
core-group-taken = Já existe um grupo com este nome
core-years-ago = { $count ->
    [one] há { $count } ano
   *[other] há { $count } anos
    }
core-engine-built-in = Integrado
core-ghostty-unavailable = Esta versão foi compilada sem o Ghostty. As versões publicadas o incluem.
core-gap-unchanged = { $count ->
    [one] ⋯ { $count } linha inalterada
   *[other] ⋯ { $count } linhas inalteradas
    }
core-update-checking = Procurando atualizações…
core-update-found = Atualização encontrada (v{ $version }), baixando…
core-update-preparing-now = Preparando a atualização…
core-update-downloading-now = Baixando a atualização…
core-update-ready = Atualização pronta: v{ $version }
core-update-from-source = Esta versão foi compilada a partir do código-fonte e não se atualiza sozinha. Compile de novo a partir do código mais recente para atualizar.
core-update-up-to-date = Você está em dia (v{ $version })
core-update-available = O SnoutCode v{ $version } está disponível.
core-update-cannot-replace = O SnoutCode v{ $version } está disponível, mas não consegue se instalar de onde está rodando. Mova-o para seus aplicativos ou baixe-o.
core-update-did-not-land = O SnoutCode v{ $version } não foi instalado. Baixe-o para instalar manualmente.
core-update-preparing = Preparando a atualização
core-update-downloading = Baixando a atualização
core-update-offline = Não foi possível procurar atualizações: este computador está sem conexão. Uma nova tentativa será feita mais tarde.
core-update-feed-down = Não foi possível procurar atualizações: o GitHub não está respondendo. Sua instalação está bem, e uma nova tentativa será feita mais tarde.
core-update-rate-limited = Não foi possível procurar atualizações: o GitHub está limitando as requisições desta rede. Uma nova tentativa será feita mais tarde.
core-update-no-release = Não foi possível procurar atualizações: nenhuma versão publicada foi encontrada.
core-update-signature = Uma atualização foi baixada e recusada: ela não corresponde à assinatura esperada. Nada foi instalado.
core-update-disk = Não foi possível salvar a atualização: não há espaço suficiente em disco.
core-update-unknown = Não foi possível procurar atualizações.
core-scope-staged = { $n ->
    [one] { $n } preparado
   *[other] { $n } preparados
    }
core-scope-all-changed = { $n ->
    [one] { $n } alterado
   *[other] todos os { $n } alterados
    }
core-scope-selected = { $n ->
    [one] { $n } selecionado
   *[other] { $n } selecionados
    }

## icons

icons-plain-text = Texto simples

## menu

menu-menu = Menu

## misc

newproject-location = Local
newproject-offline-fallback = { $why } Estas são as dependências que o SnoutCode conhece; o projeto é criado aqui, e o Gradle baixa o Spring Boot quando houver rede.
structure-modules-row = Módulos
structure-jdk-hint = O Gradle e o mecanismo Java rodam com ele. Fica em .snout/settings.json; o mecanismo passa a usá-lo na próxima vez que iniciar.
structure-locked-maven-hint = Os valores com cadeado são do Maven: Editar no arquivo de build abre a linha do pom.xml que os define. Depois de salvar, a barra sobre o editor oferece recarregar.
structure-locked-gradle-hint = Os valores com cadeado são do build: Editar no arquivo de build abre a linha que os define. Depois de salvar, a barra sobre o editor oferece sincronizar.
console-installer-not-on-path = O instalador terminou, mas o { $agent } não está no PATH que o SnoutCode vê. Indique o caminho em Configurações, Agentes, ou reinicie o SnoutCode.
console-no-project-open = Nenhum projeto aberto: o { $agent } começa na sua pasta pessoal, onde pode criar um. Abra o projeto quando ele existir para acompanhar as alterações.
dialogs-group-example = Trabalho
dialogs-checkout-placeholder = Filtrar branches e tags, ou digitar um commit (SHA, HEAD~2, …)
gutter-run-label = Executar ‘{ $label }’
gutter-debug-label = Depurar ‘{ $label }’
editor-settings-source = Configurações do SnoutCode
terminal-not-installed = { $name } não está instalado
terminal-is-mac-app = { $name } é um aplicativo de Mac
terminal-not-mac-app = { $name } não é um aplicativo de Mac
terminal-not-windows-app = { $name } não é um aplicativo do Windows
terminal-default-linux = Terminal padrão
model-could-not-open-in = Não foi possível abrir o projeto em { $connection }
model-no-repositories-found = Nenhum repositório encontrado
env-environments = Ambientes
env-base-url-hint = As execuções recebem a URL como BASE_URL, lida pelo baseURL da configuração. As variáveis do arquivo vêm primeiro, depois as de cima. Guarde as senhas no arquivo .env, que o repositório deve ignorar.
pwsetup-runner-missing = o executor de testes do Playwright não está instalado
remote-password-title = Senha
run-needs-jdk-note = Este build precisa do JDK { $v }, que não está instalado. Baixe-o pela barra acima do editor ou escolha um JDK em Configurações.
debug-js-download-failed = Não foi possível baixar o depurador JavaScript: { $e }
debug-js-did-not-start = O depurador JavaScript não iniciou: { $e }
run-ts-needs-tsx = Executar um arquivo TypeScript requer o pacote tsx no projeto ({ $manager } add -D tsx) ou o Node 22.6 ou mais recente (este projeto usa o Node { $node }).
settings-open-project-settings = Abrir as opções do projeto
settings-each-project-chooses-its-profiles = Cada projeto escolhe seus perfis ativos, em Opções do projeto.
structure-group-structure = Estrutura
structure-group-settings = Opções deste projeto
structure-download-jdk-dialog = Baixar JDK…
structure-looking-up-jdks = Procurando os JDKs que esta máquina pode baixar…
structure-jdks-could-not-be-looked-up = Não foi possível procurar os JDKs: { $e }
structure-java-lts = { $major } (suporte de longo prazo)
structure-vendor = Fornecedor
structure-location = Local



