# SnoutCode's interface in Spanish. English (i18n/en) is the source: same ids, same
# arguments. Say it the way a developer in this language would, not word for word.

## Languages

language-system = Sistema ({ $language })

## Light or dark

theme-system = Sistema
theme-light = Claro
theme-dark = Oscuro

## Settings

settings-nothing-matches = Sin resultados
settings-group-build-tools = Herramientas de compilación
settings-group-frameworks = Frameworks
settings-group-tools = Herramientas
settings-page-appearance = Apariencia
settings-page-keymap = Atajos de teclado
settings-page-editor = Editor
settings-page-version-control = Control de versiones
settings-page-terminal = Terminal
settings-page-agents = Agentes
settings-blurb-appearance = El idioma, los colores, el modo claro u oscuro y las tipografías de la ventana y del editor.
settings-blurb-keymap = Las teclas de cada comando. Añade, quita o vuelve a los valores predeterminados.
settings-blurb-editor = Cómo se sangran, se ajustan y se muestran los archivos mientras escribes.
settings-blurb-version-control = Cómo SnoutCode mantiene al día el Git de tus proyectos.
settings-blurb-gradle = El JDK de cada compilación y cuánto tiempo siguen activos los daemons de Gradle de SnoutCode.
settings-blurb-spring = Los perfiles con los que la ventana Spring evalúa los beans.
settings-blurb-terminal = El aspecto de los terminales, lo que ejecutan y la aplicación de terminal para abrir carpetas.
settings-blurb-agents = Los agentes de código que SnoutCode sigue y el que ejecuta su consola.

settings-section-language = Idioma
settings-language = Idioma de la interfaz
settings-section-theme = Tema
settings-colors = Colores
settings-light-or-dark = Claro u oscuro
settings-dark-only = { $name } no tiene versión clara: sigue oscuro, sea cual sea este ajuste.
settings-section-interface = Interfaz
settings-font = Tipografía
settings-size = Tamaño
settings-section-editor = Editor
settings-section-quitting = Salida
settings-confirm-quit = Confirmar antes de salir
settings-confirm-quit-note = SnoutCode siempre pregunta si salir haría perder cambios sin guardar o detendría algo en ejecución.
settings-section-updates = Actualizaciones
settings-update-check = Buscar actualizaciones automáticamente
settings-update-check-note = Una versión nueva se descarga en segundo plano y se instala al reiniciar o salir. En Ayuda puedes buscar actualizaciones cuando quieras.

## The menu bar

menu-run = Ejecutar
menu-snoutcode = SnoutCode
menu-services = Servicios
menu-file = Archivo
menu-open-recent = Abrir reciente
menu-edit = Edición
menu-view = Ver
menu-git = Git
menu-tool-window-actions = Acciones de las ventanas de herramientas
menu-appearance = Apariencia
menu-code = Código
menu-navigate = Navegar
menu-refactor = Refactorizar
menu-build = Compilar
menu-tools = Herramientas
menu-tool-settings = Ajustes de herramientas
menu-window = Ventana
menu-help = Ayuda

## Commands: the menu items and the command palette

cmd-new-project = Nuevo proyecto…
cmd-open-project = Abrir proyecto…
cmd-close-project = Cerrar proyecto
cmd-connect-to-ssh-host-or-wsl = Conectar a un host SSH o WSL…
cmd-connect-to-ssh-host = Conectar a un host SSH…
cmd-close-remote-connection = Cerrar conexión remota
cmd-clear-recent-projects = Borrar proyectos recientes
cmd-search-everywhere = Buscar en todas partes…
cmd-settings = Configuración…
cmd-open-settings-json = Abrir configuración (JSON)
cmd-open-project-settings-json = Abrir configuración del proyecto (JSON)
cmd-go-to-file = Ir al archivo…
cmd-recent-files = Archivos recientes…
cmd-new-file = Nuevo archivo…
cmd-go-to-class = Ir a la clase…
cmd-go-to-line = Ir a la línea…
cmd-find-in-files = Buscar en archivos…
cmd-replace-in-files = Reemplazar en archivos…
cmd-undo-replace-in-files = Deshacer reemplazo en archivos
cmd-find-in-file = Buscar en el archivo
cmd-replace-in-file = Reemplazar en el archivo
cmd-save = Guardar
cmd-close-tab = Cerrar pestaña
cmd-split-editor-right = Dividir editor a la derecha
cmd-toggle-comment = Alternar comentario
cmd-duplicate-line = Duplicar línea
cmd-move-line-up = Mover línea arriba
cmd-move-line-down = Mover línea abajo
cmd-toggle-soft-wrap = Alternar ajuste de línea
cmd-toggle-parameter-name-hints = Alternar sugerencias de nombres de parámetros
cmd-go-to-definition = Ir a la definición
cmd-go-to-declaration-or-usages = Ir a la declaración o a los usos
cmd-go-to-implementations = Ir a las implementaciones
cmd-find-references = Buscar referencias
cmd-rename-symbol = Cambiar nombre del símbolo…
cmd-extract-method = Extraer método
cmd-extract-variable = Extraer variable
cmd-extract-constant = Extraer constante
cmd-extract-field = Extraer campo
cmd-inline = Integrar
cmd-show-context-actions = Mostrar acciones contextuales…
cmd-apply-top-fix = Aplicar la primera corrección
cmd-format-document = Dar formato al documento
cmd-format-selection = Dar formato a la selección
cmd-organize-imports = Organizar importaciones
cmd-show-parameter-hints = Mostrar ayuda de parámetros
cmd-trigger-completion = Activar autocompletado
cmd-go-to-symbol-in-workspace = Ir al símbolo en el espacio de trabajo…
cmd-go-to-symbol-in-file = Ir al símbolo en el archivo…
cmd-file-structure = Estructura del archivo…
cmd-structure = Estructura
cmd-type-hierarchy = Jerarquía de tipos
cmd-call-hierarchy = Jerarquía de llamadas
cmd-hierarchy = Jerarquía
cmd-show-problems = Mostrar problemas
cmd-run = Ejecutar
cmd-debug = Depurar
cmd-run-context-configuration = Ejecutar la configuración del contexto
cmd-debug-context-configuration = Depurar la configuración del contexto
cmd-stop = Detener
cmd-rerun = Volver a ejecutar
cmd-rerun-failed-tests = Volver a ejecutar las pruebas fallidas
cmd-build-project = Compilar proyecto
cmd-rebuild-project = Recompilar proyecto
cmd-run-gradle-task = Ejecutar tarea de Gradle…
cmd-run-maven-goal = Ejecutar goal de Maven…
cmd-show-the-build-window = Mostrar la ventana de compilación
cmd-show-the-gradle-window = Mostrar la ventana de Gradle
cmd-show-the-maven-window = Mostrar la ventana de Maven
cmd-terminal = Terminal
cmd-output = Salida
cmd-settings-keymap = Configuración: atajos de teclado
cmd-settings-terminal = Configuración: terminal
cmd-settings-coding-agents = Configuración: agentes de código
cmd-settings-gradle-and-maven = Configuración: Gradle y Maven
cmd-edit-configurations = Editar configuraciones…
cmd-edit-environments = Editar entornos…
cmd-show-the-tests-window = Mostrar la ventana de pruebas
cmd-record-at-cursor = Grabar en el cursor
cmd-pick-locator = Elegir localizador
cmd-stop-recording = Detener la grabación
cmd-open-ui-mode = Abrir el modo UI
cmd-toggle-breakpoint = Alternar punto de interrupción
cmd-toggle-breakpoint-enabled = Habilitar o deshabilitar punto de interrupción
cmd-edit-breakpoint-condition = Editar condición del punto de interrupción…
cmd-view-breakpoints = Ver puntos de interrupción…
cmd-evaluate-expression = Evaluar expresión…
cmd-run-to-cursor = Ejecutar hasta el cursor
cmd-step-over = Paso por encima
cmd-step-into = Paso hacia dentro
cmd-step-out = Paso hacia fuera
cmd-pause-program = Pausar el programa
cmd-resume-program = Reanudar el programa
cmd-clear-output = Borrar la salida
cmd-sync-gradle-and-maven-projects = Sincronizar proyectos de Gradle y Maven
cmd-project-structure = Estructura del proyecto…
cmd-refresh-gradle-tasks = Actualizar tareas de Gradle
cmd-collapse-all-gradle-tasks = Contraer todas las tareas de Gradle
cmd-reload-maven-project = Recargar proyecto de Maven
cmd-read-the-maven-goals-again = Volver a leer los goals de Maven
cmd-collapse-all-maven-goals = Contraer todos los goals de Maven
cmd-spring-beans-and-endpoints = Beans y endpoints de Spring
cmd-read-the-spring-model-again = Volver a leer el modelo de Spring
cmd-go-to-spring-bean-or-usages = Ir al bean de Spring o a los usos…
cmd-spring-active-profiles = Perfiles activos de Spring…
cmd-collapse-the-spring-window = Contraer la ventana de Spring
cmd-reload-all-from-disk = Recargar todo desde el disco
cmd-select-opened-file = Seleccionar el archivo abierto
cmd-collapse-the-project-tree = Contraer el árbol del proyecto
cmd-hide-the-side-panel = Ocultar el panel lateral
cmd-sort-structure-by-name = Ordenar la estructura por nombre
cmd-show-processes = Mostrar procesos
cmd-show-the-agent-log = Mostrar el registro del agente
cmd-stop-idle-processes = Detener los procesos inactivos
cmd-restart-the-java-engine = Reiniciar el motor de Java
cmd-send-selection-to-claude = Enviar la selección a Claude
cmd-accept-claudes-change = Aceptar el cambio de Claude
cmd-reject-claudes-change = Rechazar el cambio de Claude
cmd-next-claude-change = Siguiente cambio de Claude
cmd-previous-claude-change = Cambio anterior de Claude
cmd-accept-claude-change-at-caret = Aceptar el cambio de Claude en el cursor
cmd-revert-claude-change-at-caret = Revertir el cambio de Claude en el cursor
cmd-accept-all-claude-changes = Aceptar todos los cambios de Claude
cmd-revert-all-claude-changes = Revertir todos los cambios de Claude
cmd-show-agent-changes = Mostrar los cambios de los agentes
cmd-annotate-with-agents = Anotar con los agentes
cmd-annotate-with-git-blame = Anotar con Git Blame
cmd-undo = Deshacer
cmd-redo = Rehacer
cmd-zoom-in = Acercar
cmd-zoom-out = Alejar
cmd-actual-size = Tamaño real
cmd-show-project = Mostrar el proyecto
cmd-show-git = Mostrar Git
cmd-toggle-agent-console = Alternar la consola del agente
cmd-find-repositories-in-folder = Buscar repositorios en una carpeta…
cmd-clone-from-github = Clonar desde GitHub…
cmd-new-group = Nuevo grupo…
cmd-refresh = Actualizar
cmd-pull = Pull
cmd-fetch = Fetch
cmd-push = Push
cmd-commit = Commit
cmd-draft-commit-message = Redactar el mensaje de commit
cmd-stage = Preparar
cmd-unstage = Quitar de preparados
cmd-discard-changes = Descartar cambios…
cmd-check-out = Cambiar a…
cmd-new-branch = Nueva rama…
cmd-side-by-side-diff = Diff en paralelo
cmd-unified-diff = Diff unificado
cmd-compact-diff = Diff compacto
cmd-theme-match-the-system = Tema: igual que el sistema
cmd-theme-dark = Tema: oscuro
cmd-theme-light = Tema: claro
cmd-open-on-github = Abrir en GitHub
cmd-open-in-external-terminal = Abrir en un terminal externo
cmd-about-snoutcode = Acerca de SnoutCode
cmd-check-for-updates = Buscar actualizaciones…
cmd-release-notes = Notas de la versión
cmd-quit-snoutcode = Salir de SnoutCode
cmd-clear-recent = Borrar recientes
cmd-exit = Salir
cmd-quit = Salir
cmd-zoom = Zoom
cmd-maximize = Maximizar
cmd-command-palette = Paleta de comandos…
cmd-cut = Cortar
cmd-copy = Copiar
cmd-paste = Pegar
cmd-select-all = Seleccionar todo
cmd-find = Buscar
cmd-replace = Reemplazar
cmd-project = Proyecto
cmd-git = Git
cmd-agent-console = Consola del agente
cmd-soft-wrap = Ajuste de línea
cmd-parameter-name-hints = Sugerencias de nombres de parámetros
cmd-processes = Procesos
cmd-agent-log = Registro del agente
cmd-build-window = Ventana de compilación
cmd-gradle = Gradle
cmd-maven = Maven
cmd-move-to-trash = Mover a la papelera…
cmd-agent-changes = Cambios de los agentes
cmd-keymap = Atajos de teclado…
cmd-terminal-more = Terminal…
cmd-coding-agents = Agentes de código…
cmd-gradle-and-maven = Gradle y Maven…
cmd-minimize = Minimizar
cmd-snoutcode-on-github = SnoutCode en GitHub
cmd-report-an-issue = Informar de un problema
cmd-snoutdata-website = Sitio web de SnoutData
cmd-open-recent-project = Abrir reciente: { $name }

## The platform's file manager

machine-mac = este Mac
machine-windows = este PC
machine-linux = este equipo
reveal-mac = Mostrar en Finder
reveal-windows = Mostrar en el Explorador
reveal-linux = Mostrar en Archivos

## settings

settings-search-settings = Buscar en la configuración
settings-found-on-path-and-the-usual-places = Se busca en el PATH y en las ubicaciones habituales
settings-its-programs-name = El nombre de su programa
settings-your-login-shell-shell = Tu shell de inicio de sesión ($SHELL)
settings-no-arguments = Sin argumentos
settings-name-value-then-return = NOMBRE=valor y luego Intro
settings-search-commands-or-shortcuts = Buscar comandos o atajos
settings-overridden-by-this-project = Reemplazado por este proyecto
settings-set-for-this-project = Definido para este proyecto
settings-reset-to-your-setting = Volver a tu ajuste
settings-your-setting = Tu ajuste
settings-user = Usuario
settings-your-settings-json = Tu settings.json
settings-for-only-kept-in-in-the-project-what = Solo para { $root }, guardado en { $file } en el proyecto. Lo que define reemplaza tu configuración.
settings-your-settings-for-every-project-a-project-can = Tu configuración, para todos los proyectos. Un proyecto puede reemplazar parte de ella.
settings-in-every-build = En todas las versiones
settings-reflow-kitty-keys-links = Reajuste, teclas kitty, enlaces
settings-unavailable = No disponible
settings-the-same-in-every-project = Igual en todos los proyectos
settings-settings-are-yours-a-project-cannot-set-them = Los ajustes de { $title } son tuyos: un proyecto no puede definirlos por su cuenta.
settings-show-your-settings = Mostrar tu configuración
settings-indentation = Sangría
settings-tab-size = Tamaño de tabulación
settings-for-a-file-whose-own-indentation-cannot-be = Para un archivo cuya sangría no se puede determinar.
settings-display = Visualización
settings-soft-wrap-long-lines = Ajustar las líneas largas
settings-show-whitespace = Mostrar espacios en blanco
settings-show-parameter-names-of-literal-arguments-java = Mostrar los nombres de parámetro de los argumentos literales (Java)
settings-hover = Información emergente
settings-show-hover = Mostrar
settings-delay = Retraso
settings-navigation = Navegación
settings-click-with = Clic con
settings-preview = Vista previa
settings-fetch-every = Fetch cada
settings-the-projects-are-fetched-in-the-background-to = Se hace fetch de los proyectos en segundo plano, para contar lo que hay por traer.
settings-annotations = Anotaciones
settings-show-who-wrote-each-class-and-method-ada = Mostrar quién escribió cada clase y método («Ada Lovelace +2»)
settings-show-the-current-lines-author-and-commit-after = Mostrar el autor y el commit de la línea actual tras su final
settings-good-to-know = Conviene saber
settings-a-background-fetch-leaves-your-branches-as-they = Un fetch en segundo plano no toca tus ramas: solo cuenta los commits que hay por traer.
settings-click-the-branch-in-the-title-bar-for = Haz clic en la rama de la barra de título para Nueva rama, Fetch, Pull y Push, y para cambiar a cualquier rama.
settings-2-shows-the-git-view-in-the-message = ⌘2 muestra la vista de Git. En el campo del mensaje, ⌘↩ hace el commit.
settings-each-builds-own-your-settings = El de cada compilación (tu configuración)
settings-every-build = Todas las compilaciones
settings-jdk-per-project = JDK por proyecto
settings-no-gradle-build-is-open = No hay ninguna compilación de Gradle abierta.
settings-automatic-the-builds-java-level = Automático (el nivel de Java de la compilación)
settings-the-java-server-restarts-on-the-chosen-jdk = El servidor de Java usará el JDK elegido la próxima vez que se inicie.
settings-gradle-daemons = Daemons de Gradle
settings-stop-idle-daemons-after = Detener los daemons inactivos tras
settings-double-click-a-task-in-the-gradle-tool = Haz doble clic en una tarea de la ventana de Gradle para ejecutarla. Su salida aparece en la ventana de compilación.
settings-the-processes-window-lists-the-gradle-daemons-running = La ventana Procesos enumera los daemons de Gradle en marcha y detiene los inactivos cuando se lo pides.
settings-project-structure-downloads-jdks-and-sets-the-projects = Estructura del proyecto (⌘;) descarga JDK y define el del proyecto.
settings-the-leaf-on-the-right-stripe-opens-the = La hoja de la barra derecha abre la ventana de Spring: los beans y los endpoints que SnoutCode encontró.
settings-spring-active-profiles-p-changes-the-profiles-from = Perfiles activos de Spring (⌃⌥P) cambia los perfiles desde cualquier lugar.
settings-a-bean-the-active-profiles-leave-out-is = Un bean que los perfiles activos dejan fuera aparece atenuado y ninguna inyección lo usa.
settings-active-profiles = Perfiles activos
settings-open-a-project-to-choose-its-profiles = Abre un proyecto para elegir sus perfiles.
settings-any-no-profile-chosen = Cualquiera (ningún perfil elegido)
settings-those-the-application-files-make-active = Los que activan los archivos de la aplicación
settings-these-profiles = Estos perfiles
settings-evaluate-beans-under = Evaluar los beans con
settings-profiles = Perfiles
settings-choose = Elegir
settings-looking-for-it = Buscando…
settings-that-file-is-not-there = Ese archivo no existe.
settings-not-found-install-it-from-the-console-or = No se encontró. Instálalo desde la consola o elige el programa.
settings-browse = Examinar…
settings-sees-the-editor-its-hooks-report-each-edit = Ve el editor; sus hooks informan de cada edición y prompt
settings-its-hooks-report-each-edit-and-prompt = Sus hooks informan de cada edición y prompt
settings-a-plugin-of-snoutcodes-reports-each-edit-and = Un plugin de SnoutCode informa de cada edición y prompt
settings-files-seen-on-disk-while-it-works-in = Archivos vistos en el disco mientras trabaja en la consola
settings-its-colour-caret-and-marks = Su color, cursor y marcas
settings-the-console-runs = La consola ejecuta
settings-programs = Programas
settings-your-agent = Tu agente
settings-its-name = Su nombre
settings-any-command-line-agent-the-console-runs-it = Cualquier agente de línea de comandos: la consola lo ejecuta en el proyecto con /bin/sh -c.
settings-j-shows-or-hides-the-console-the-session = ⌘J muestra u oculta la consola. La sesión sigue en marcha mientras está oculta.
settings-k-sends-the-selection-or-the-whole-file = ⌥⌘K envía la selección, o todo el archivo, al agente.
settings-in-a-proposed-changes-tab-accepts-it-and = En la pestaña de un cambio propuesto, ⌘↩ lo acepta y Esc lo rechaza.
settings-the-agent-changes-window-keeps-every-edit-an = La ventana Cambios de los agentes guarda cada edición de un agente, por prompt, para aceptarla o revertirla.
settings-ghostty-is-unavailable = Ghostty no está disponible
settings-terminal-engine = Motor del terminal
settings-use-the-editors = Usar la del editor
settings-the-editors-size = El tamaño del editor
settings-blink = Parpadeo
settings-look = Aspecto
settings-line-height = Altura de línea
settings-cursor = Cursor
settings-remove-this-variable = Quitar esta variable
settings-shell = Shell
settings-program = Programa
settings-empty-the-login-shell-of-your-account-on = Vacío: el shell de inicio de sesión de tu cuenta en la máquina remota.
settings-empty-your-login-shell-on-a-remote-machine = Vacío: tu shell de inicio de sesión. En una máquina remota, tu shell allí.
settings-arguments = Argumentos
settings-separated-by-spaces-l-starts-a-login-shell = Separados por espacios. -l inicia un shell de inicio de sesión.
settings-environment = Entorno
settings-add = Añadir
settings-added-to-the-terminals-and-the-claude-code = Se añade a los terminales y a la consola de Claude Code que inicies tras un cambio.
settings-behaviour = Comportamiento
settings-scrollback = Historial de desplazamiento
settings-lines-for-terminals-started-after-a-change = líneas, para los terminales iniciados tras un cambio
settings-copy-text-as-soon-as-it-is-selected = Copiar el texto en cuanto se selecciona
settings-use-option-as-meta-f-moves-a-word = Usar Opción como Meta (⌥F avanza una palabra, no escribe ƒ)
settings-external-terminal = Terminal externo
settings-open-in = Abrir en
settings-command = Comando
settings-running-now = En ejecución
settings-a-terminal-running-another-engine-keeps-it-until = Un terminal con otro motor lo conserva hasta que se reinicie (Nuevo, en su cabecera).
settings-press-the-keys-or-esc = Pulsa las teclas, o Esc
settings-none = Ninguno
settings-was-the-shortcut-of-here-it-now-runs = { $keys } era el atajo de { $join }. Aquí ahora ejecuta este comando.
settings-remove-it-there = Quitarlo de allí
settings-no-command-matches = Ningún comando coincide.
settings-reset-all = Restablecer todo
settings-adds-a-shortcut-press-its-keys-changes-apply = + añade un atajo: pulsa sus teclas. Los cambios se aplican con Aplicar u OK.
settings-settings = Configuración
settings-edit-as-json = Editar como JSON
settings-open-these-settings-file-in-a-tab-with = Abrir el archivo de esta configuración en una pestaña, con autocompletado y comprobaciones
settings-cancel = Cancelar
settings-apply = Aplicar
settings-cannot-be-read = No se puede leer { $file }
settings-the-values-it-had-before-stay-in-use = Los valores anteriores siguen en uso hasta que se corrija. { $e }
settings-in-shop = { $shell } en shop
settings-engine = Motor { $shown }
settings-found = Encontrado: { $path }
settings-editors = La del editor ({ $editor_font })
settings-not-installed = { $chosen } (no instalado)
settings-in = { $kind } en { $root }
settings-reset = Restablecer
settings-also-bound-to = También asignado a { $join }
settings-no-jdk = No se encontró ningún JDK en { $machine }.
settings-after-a-delay = Tras un retraso
settings-only-while-is-held = Solo mientras se mantiene ⌘
settings-block = Bloque
settings-bar = Barra
settings-underline = Subrayado

## dialogs

dialogs-discard-body = { $count ->
    [one] Este archivo vuelve a como está en HEAD. Los archivos sin seguimiento se eliminan.
   *[other] Estos { $count } archivos vuelven a como están en HEAD. Los archivos sin seguimiento se eliminan.
    }
dialogs-trash-body = { $count ->
    [one] Este archivo va a la papelera. Los archivos con seguimiento aparecen entonces como eliminados, listos para el commit.
   *[other] Estos { $count } archivos van a la papelera. Los archivos con seguimiento aparecen entonces como eliminados, listos para el commit.
    }
dialogs-delete-body = { $count ->
    [one] Este archivo se elimina del disco. Los archivos con seguimiento aparecen entonces como eliminados, listos para el commit.
   *[other] Estos { $count } archivos se eliminan del disco. Los archivos con seguimiento aparecen entonces como eliminados, listos para el commit.
    }
dialogs-unsaved-files = { $count ->
    [one] Tienes cambios sin guardar en un archivo.
   *[other] Tienes cambios sin guardar en { $count } archivos.
    }
dialogs-version = Versión { $cargo_pkg_version }
dialogs-git-stopped-and-said-why = Git se detuvo y explicó por qué.
dialogs-you-have-uncommitted-changes-what-should-happen-to = Tienes cambios sin commit. ¿Qué hacemos con ellos?
dialogs-a-line-of-work-of-its-own-from = Una línea de trabajo propia, desde donde elijas.
dialogs-its-commits-stay-wherever-else-they-are-reachable = Sus commits siguen donde aún sean accesibles.
dialogs-it-goes-for-everyone-who-uses-the-remote = Desaparece para todos los que usan el remoto. Las ramas locales no cambian.
dialogs-copy-a-remote-repository-to-this-computer-and = Copia un repositorio remoto en este equipo y ábrelo.
dialogs-is-waiting-in-the-console-for-your-answer = { $agent } espera tu respuesta en la consola.
dialogs-the-window-closes-and-the-java-engine-stops = La ventana se cierra y el motor de Java se detiene con ella.
dialogs-what-runs-now-stops-when-snoutcode-quits = Lo que se está ejecutando se detendrá al salir de SnoutCode.
dialogs-clone-repository = Clonar repositorio
dialogs-local-branches = Ramas locales
dialogs-remote-branches = Ramas remotas
dialogs-tags = Etiquetas
dialogs-unsaved = Sin guardar
dialogs-these-stop = Esto se detiene:
dialogs-dont-ask-again-settings-appearance-confirm-before-quitti = No volver a preguntar (Configuración, Apariencia: Confirmar antes de salir)
dialogs-discard-and-quit = Descartar y salir
dialogs-save-all-and-quit = Guardar todo y salir
dialogs-an-ide-for-java-gradle-and-spring-boot = Un IDE para Java, Gradle y Spring Boot, con Git y Claude Code integrados.
dialogs-made-by-snoutdata-mit-license = Creado por SnoutData. Licencia MIT.
dialogs-this-cannot-be-undone = Esto no se puede deshacer.
dialogs-discard = Descartar
dialogs-you-can-still-get-them-back-from-the = Todavía podrás recuperarlos de la papelera.
dialogs-move-to-trash = Mover a la papelera
dialogs-delete = Eliminar
dialogs-stash-check-out-re-apply = Stash, cambiar y volver a aplicar
dialogs-put-the-changes-aside-switch-then-bring-them = Aparta los cambios, cambia de rama y luego recupéralos. Funciona aunque toquen archivos que difieren en el destino.
dialogs-check-out-keep-changes = Cambiar y conservar los cambios
dialogs-carry-the-changes-across-as-they-are-git = Lleva los cambios tal como están. Git se niega si fueran a sobrescribirse.
dialogs-this-leaves-head-detached-create-a-branch-if = Esto deja HEAD separado. Crea una rama si quieres conservar commits nuevos.
dialogs-and-have-each-moved-on-and-this-repositorys = { $branch } y { $upstream } han avanzado por separado, y la configuración de este repositorio no indica cómo combinarlas.
dialogs-merge = Merge
dialogs-join-both-histories-with-a-merge-commit-nothing = Une ambos historiales con un commit de merge. No se reescribe nada.
dialogs-rebase = Rebase
dialogs-replay-your-commits-on-top-of-the-remote = Reaplica tus commits sobre los remotos, para un historial lineal.
dialogs-and-have-no-commit-in-common-so-they = { $branch } y { $upstream } no tienen ningún commit en común, así que no se pueden fusionar. Suele significar que el repositorio remoto se recreó o que su historial se reescribió.
dialogs-resetting-makes-match-exactly-your-current-commits-stay = Al restablecer, { $branch } coincidirá exactamente con { $upstream }. Tus commits actuales quedan en una rama de respaldo, así que no se pierde nada.
dialogs-commit-or-stash-your-uncommitted-changes-first-resetting = Primero haz commit o stash de tus cambios; restablecer los descartaría.
dialogs-reset-to = Restablecer a { $upstream }
dialogs-name = Nombre
dialogs-starting-at = A partir de
dialogs-check-out-the-new-branch = Cambiar a la nueva rama
dialogs-create = Crear
dialogs-renaming = Cambiando el nombre de { $old }
dialogs-goes-into-the-new-group = { $repo } entra en el nuevo grupo.
dialogs-groups-keep-related-repositories-together-drag-repositor = Los grupos reúnen repositorios relacionados. Arrastra repositorios a uno.
dialogs-rename = Cambiar nombre
dialogs-goes-away = desaparecerá.
dialogs-delete-even-if-it-is-not-fully-merged = Eliminar aunque no esté totalmente fusionada
dialogs-also-delete-on-the-remote = Eliminar también { $up } en el remoto
dialogs-removing-it-from-the-remote-affects-everyone-who = Quitarla del remoto afecta a todos los que la usan.
dialogs-delete-branch = Eliminar rama
dialogs-repository-url = URL del repositorio
dialogs-into-folder = En la carpeta
dialogs-lands-in = Destino:
dialogs-clone = Clonar
dialogs-no-matching-branch-tag-or-commit = Ninguna rama, etiqueta o commit coincide
dialogs-uncommitted-changes = Cambios sin commit:
dialogs-keep-them = Conservarlos
dialogs-stash-and-re-apply = Stash y volver a aplicar
dialogs-check-out = Cambiar
dialogs-dont-allow = No permitir
dialogs-allow-for-this-session = Permitir en esta sesión
dialogs-allow = Permitir
dialogs-close = Cerrar
dialogs-a-branch-a-tag-or-any-commit = Una rama, una etiqueta o cualquier commit.
dialogs-something-went-wrong = Algo salió mal
dialogs-discard-changes = ¿Descartar los cambios?
dialogs-check-out-2 = ¿Cambiar a { $t }?
dialogs-check-out-3 = Cambiar de rama
dialogs-new-branch = Nueva rama
dialogs-delete-branch-2 = ¿Eliminar la rama?
dialogs-delete-branch-from = ¿Eliminar la rama de { $remote }?
dialogs-move-to-the-trash = ¿Mover a la papelera?
dialogs-delete-files = ¿Eliminar los archivos?
dialogs-both-sides-have-new-commits = Ambos lados tienen commits nuevos
dialogs-no-shared-history = Sin historial común
dialogs-rename-group = Cambiar nombre del grupo
dialogs-new-group = Nuevo grupo
dialogs-quit-snoutcode = ¿Salir de SnoutCode?
dialogs-allow-to = ¿Permitir que { $agent } { $what }?
dialogs-its-upstream = su rama upstream
dialogs-delete-from = Eliminar de { $remote }
dialogs-allow-for-this-session-lets-it-use-again = Permitir en esta sesión le deja volver a usar { $tool } sin preguntar.

## welcome

welcome-minutes-ago = { $count ->
    [one] hace { $count } minuto
   *[other] hace { $count } minutos
    }
welcome-hours-ago = { $count ->
    [one] hace { $count } hora
   *[other] hace { $count } horas
    }
welcome-days-ago = { $count ->
    [one] hace { $count } día
   *[other] hace { $count } días
    }
welcome-weeks-ago = { $count ->
    [one] hace { $count } semana
   *[other] hace { $count } semanas
    }
welcome-months-ago = { $count ->
    [one] hace { $count } mes
   *[other] hace { $count } meses
    }
welcome-just-now = ahora mismo
welcome-yesterday = ayer
welcome-search-recent-projects = Buscar proyectos recientes
welcome-welcome = Bienvenida
welcome-go-to-file = Ir al archivo
welcome-recent-files = Archivos recientes
welcome-search-everywhere = Buscar en todas partes
welcome-find-in-files = Buscar en archivos
welcome-new-file = Nuevo archivo
welcome-no-files-open = Ningún archivo abierto
welcome-open-one-from-the-project-tree-or = Abre uno desde el árbol del proyecto, o:
welcome-recent-files-2 = Archivos recientes
welcome-clone-repository = Clonar repositorio…
welcome-from-a-git-url = Desde una URL de Git
welcome-java-or-spring-boot-app = Aplicación Java o Spring Boot
welcome-no-recent-project-matches = Ningún proyecto reciente coincide.
welcome-no-recent-projects-yet-open-a-folder-to = Aún no hay proyectos recientes. Abre una carpeta para empezar.
welcome-recent-projects = Proyectos recientes
welcome-a-folder-on = Una carpeta en { $host }
welcome-folder-on-machine = Una carpeta en { $machine }

## tabs

tabs-unpin-tab = Desfijar pestaña
tabs-pin-tab = Fijar pestaña
tabs-close-others = Cerrar las demás
tabs-close-tabs-to-the-left = Cerrar pestañas a la izquierda
tabs-close-tabs-to-the-right = Cerrar pestañas a la derecha
tabs-close-all = Cerrar todas
tabs-close-all-but-pinned = Cerrar todas menos las fijadas
tabs-keep-open = Mantener abierta
tabs-reload-from-disk = Recargar desde el disco
tabs-copy-path = Copiar ruta
tabs-clear = Borrar
tabs-sync-the-gradle-project = Sincronizar el proyecto de Gradle
tabs-read-the-tasks-again = Volver a leer las tareas
tabs-collapse-all = Contraer todo
tabs-reload-the-maven-project = Recargar el proyecto de Maven
tabs-read-the-goals-again = Volver a leer los goals
tabs-stop-idle-processes-idle-gradle-daemons-leftovers-and = Detener los procesos inactivos: daemons de Gradle inactivos, restos y motores de proyectos cerrados
tabs-options = Opciones
tabs-hide = Ocultar

## projects

projects-filter = Filtrar
projects-open-local-repository = Abrir repositorio local…
projects-find-repositories-in-folder = Buscar repositorios en una carpeta…
projects-no-matches = Sin coincidencias
projects-rename = Cambiar nombre…
projects-delete-group = Eliminar grupo
projects-new-group = Nuevo grupo…
projects-drag-repositories-here = Arrastra repositorios aquí
projects-add-a-repository-or-a-group = Añadir un repositorio o un grupo
projects-repositories = Repositorios
projects-no-repositories-yet-use-or-o = Aún no hay repositorios. Usa + o ⌘O.
projects-copy-path = Copiar ruta
projects-open-as-project = Abrir como proyecto
projects-move-to-group = Mover a un grupo
projects-no-group = Sin grupo
projects-remove-from-list = Quitar de la lista

## status

status-snoutcodes-processes-running-of-memory-click-to-show = Procesos de SnoutCode: { $count } en ejecución, { $memory } de memoria. Haz clic para verlos.
status-in-the-background = En segundo plano
status-the-editor-works-meanwhile = El editor sigue funcionando mientras tanto.
status-reconnect = Reconectar
status-changes-the-agents-made-that-wait-for-your = Cambios de los agentes que esperan tu revisión
status-review = Revisar
status-restart = Reiniciar
status-read-only = Solo lectura
status-java-support-unavailable = Compatibilidad con Java no disponible
status-chars-lines = { $chars } caracteres, { $lines } líneas
status-selected = { $chars } seleccionados
status-and-more = y { $count } más

## search

search-go-to-file = Ir al archivo
search-line-or-line-column = Línea, o línea:columna
search-type-a-command = Escribe un comando
search-run-a-gradle-task-or-maven-goal-its = Ejecutar una tarea de Gradle o un goal de Maven: su nombre o ruta
search-spring-profiles-pick-one-to-turn-it-on = Perfiles de Spring: elige uno para activarlo o desactivarlo
search-go-to-symbol = Ir al símbolo
search-filter-the-files-symbols = Filtrar los símbolos del archivo
search-go-to-class-in-the-project-its-libraries = Ir a la clase, en el proyecto, sus bibliotecas y el JDK
search-new-file-its-path-in-the-project-like = Nuevo archivo: su ruta en el proyecto, como src/main/java/App.java
search-no-file-matches = Ningún archivo coincide
search-no-command-matches = Ningún comando coincide
search-no-task-matches = Ninguna tarea coincide
search-no-profile-matches = Ningún perfil coincide
search-type-a-name = Escribe un nombre
search-no-symbol-matches = Ningún símbolo coincide
search-type-a-class-name-or-its-capitals = Escribe un nombre de clase, o sus mayúsculas
search-no-class-matches = Ninguna clase coincide
search-no-recent-file-matches = Ningún archivo reciente coincide
search-return-makes-the-file-and-opens-it = Intro crea el archivo y lo abre
search-type-a-line-number-or-line-column = Escribe un número de línea, o línea:columna
search-go-to-line-column = Ir a la línea { $line }, columna { $column }
search-try-fewer-letters-or-another-word = Prueba con menos letras u otra palabra.

## find

find-matches = { $count ->
    [one] { $count } coincidencia
   *[other] { $count } coincidencias
    }
find-files = { $count ->
    [one] { $count } archivo
   *[other] { $count } archivos
    }
find-matches-in-files = { $matches } en { $files }
find-replaced = Reemplazadas { $matches } en { $files }
find-put-back = Restaurados: { $files }
find-to-replace = , { $ticked } por reemplazar
find-first-only = {" "}(solo las primeras)
find-skipped = ; { $files } cambiados entretanto y sin tocar
find-undo-hint = . Edición ▸ Deshacer reemplazo en archivos lo restaura.
find-not-a-regular-expression = No es una expresión regular: { $error }
find-the-regular-expression-is-too-large = La expresión regular es demasiado grande
find-module = Módulo
find-directory = Directorio
find-open-files = Archivos abiertos
find-text-to-find = Texto que buscar
find-replace-with = Reemplazar por
find-file-mask-java-test-java = Máscara de archivo: *.java, !*Test.java
find-folder-from-the-projects = Carpeta, desde la del proyecto
find-match-case = Coincidir mayúsculas
find-whole-words = Palabras completas
find-regular-expression-1-in-the-replacement-names-a = Expresión regular ($1 en el reemplazo nombra un grupo)
find-hide-replace = Ocultar Reemplazar
find-replace-in-files-r = Reemplazar en archivos (⇧⌘R)
find-choose = Elegir…
find-find-in-the-projects-files = Buscar en los archivos del proyecto
find-type-a-word-a-phrase-or-a-regular = Escribe arriba una palabra, una frase o una expresión regular.
find-nothing-found = No se encontró nada
find-try-fewer-letters-another-scope-or-a-wider = Prueba con menos letras, otro ámbito o una máscara más amplia.
find-searching = Buscando…
find-replace-selected = Reemplazar lo seleccionado
find-replace-all = Reemplazar todo
find-open = Abrir
find-replace-in-files = Reemplazar en archivos
find-no-replacement-to-undo = No hay ningún reemplazo que deshacer
find-not-a-file-mask = No es una máscara de archivo: { $kind }
find-module-2 = Módulo: { $name }
find-of = { $ticked } de { $count }

## files

files-no-project-open = Ningún proyecto abierto
files-open-a-project-to-see-its-files = Abre un proyecto para ver sus archivos.
files-the-connection-is-lost = Se perdió la conexión
files-node-package = Paquete de Node: { $path }
files-the-files-on-come-back-when-it-reconnects = Los archivos de { $host } volverán cuando se reconecte.

## project

project-no-classes-to-show = No hay clases que mostrar
project-reading-the-build-model = Leyendo el modelo de compilación
project-external-libraries = Bibliotecas externas
project-the-build-model-could-not-be-read = No se pudo leer el modelo de compilación: { $e }
project-uses = usa { $join }
project-showing-the-last-model-that-synced = { $reason }{"\u000A"}Se muestra el último modelo sincronizado.
project-compiled-by = Compilado por { $join }
project-did-not-sync = No se sincronizó: { $reason }{"\u000A"}{"\u000A"}{ $note }
project-did-not-sync-2 = No se sincronizó: { $reason }
project-excluded = Excluido: { $path }
project-and-more = { $builds } y { $n } más

## model

model-build-files = Archivos de compilación
model-maven-files = Archivos de Maven
model-gradle-scripts = Scripts de Gradle
model-detached-at = separado en { $count }
model-detached-head = HEAD separado
model-does-not-exist = { $path } no existe
model-ready = Listo
model-no-upstream = {"  "}(sin upstream)
model-opened = { $asked } abierto
model-snoutcode-could-not-connect-to = SnoutCode no pudo conectarse a { $connection }. { $why }
model-showing-in-git = Mostrando { $root } en Git
model-closed = { $closed } cerrado
model-could-not-save-settings = No se pudo guardar la configuración: { $e }
model-your-settings-json-has-an-error-so-the = Tu settings.json tiene un error, así que sigue la configuración en uso: { $e }
model-your-settings-are-read-again = Tu configuración se ha vuelto a leer
model-could-not-save-the-projects-settings = No se pudo guardar la configuración del proyecto: { $e }
model-finished = { $label } terminado
model-cloned-into = Clonado en { $root }
model-failed = { $label } falló
model-failed-2 = { $label } falló:{"\u000A"}{ $e }
model-no-git-repositories-within-levels-of = Ningún repositorio Git a menos de { $SCAN_DEPTH } niveles de{"\u000A"}{ $root }
model-found-repositories-in-new = { $count } repositorios encontrados en { $root } ({ $added } nuevos)
model-asking-the-agent-to-draft-the-commit-message = Pidiendo al agente que redacte el mensaje de commit…
model-the-agent-has-not-sent-a-commit-message = El agente aún no ha enviado un mensaje de commit. Su respuesta está en la consola.
model-asks-you-something-in-the-console-answer-it = { $name } te pregunta algo en la consola. Responde allí y la redacción continuará.
model-is-writing-the-commit-message = { $name } está escribiendo el mensaje de commit…
model-the-agent-finished-without-writing-a-commit-message = El agente terminó sin escribir un mensaje de commit. Su respuesta está en la consola.
model-no-commit-message-yet = Aún no hay mensaje de commit
model-drafted-the-commit-message = { $name } redactó el mensaje de commit
model-refreshed = Actualizado
model-open-project = Abrir proyecto
model-initialized-repository-in = Repositorio inicializado en { $root }
model-find-repositories-in-folder = Buscar repositorios en una carpeta
model-searching-for-repositories = Buscando repositorios
model-staged = { $count ->
    [one] 1 archivo preparado
   *[other] { $count } archivos preparados
    }
model-discarded = { $count ->
    [one] 1 archivo descartado
   *[other] { $count } archivos descartados
    }
model-trashed = { $count ->
    [one] 1 archivo movido a la papelera
   *[other] { $count } archivos movidos a la papelera
    }
model-deleted = { $count ->
    [one] 1 archivo eliminado
   *[other] { $count } archivos eliminados
    }
model-unstaged = Quitado de preparados
model-added-to-gitignore = { $pattern } añadido a .gitignore
model-added-rules-to-gitignore = { $n } reglas añadidas a .gitignore
model-committed = Commit hecho: { $summary }
model-stopped-drafting = Redacción detenida
model-checked-out = Cambiado a { $target }
model-local-changes-conflicted = Los cambios locales entraron en conflicto
model-created-branch = Rama { $name } creada
model-deleted-branch = Rama { $name } eliminada
model-delete = Eliminar { $r }
model-pull-merge = Pull (merge)
model-pull-rebase = Pull (rebase)
model-reset-to = { $head } restablecida a { $upstream }
model-your-previous-commits-are-kept-on-the-branch = Tus commits anteriores se conservan en la rama { $backup }. Elimínala cuando ya no los necesites.
model-now-matches = { $head } ahora coincide con { $upstream }
model-commit = Commit { $short_id } { $summary }

## branch

branch-search-branches = Buscar ramas
branch-local = Local
branch-remote = Remoto

## panels

panels-files = Archivos
panels-no-output-yet = Aún no hay salida
panels-what-git-and-the-tools-say-shows-here = Lo que dicen Git y las herramientas aparece aquí mientras trabajan.
panels-open-git = Abrir Git

## title

title-no-project = Sin proyecto
title-current-file = Archivo actual
title-yours = Las tuyas
title-reading-the-configurations = Leyendo las configuraciones…
title-configurations-appear-once-the-tests-are-listed = Las configuraciones aparecerán cuando se enumeren las pruebas
title-configurations-appear-once-the-java-engine-is-ready = Las configuraciones aparecerán cuando el motor de Java esté listo
title-stop-f2 = Detener (⌘F2)
title-search-everywhere-a = Buscar en todas partes (⇧⌘A)
title-settings = Configuración (⌘,)
title-the-current-file = el archivo actual
title-rerun-r = Volver a ejecutar { $what } (⌃R)
title-run-r = Ejecutar { $what } (⌃R)
title-debug-d = Depurar { $what } (⌃D)

## stripes

stripes-left = Izquierda
stripes-right = Derecha
stripes-bottom = Abajo
stripes-move-to = Mover a
stripes-build = Compilación
stripes-problems-m = Problemas (⇧⌘M)
stripes-agent-console-j = Consola del agente (⌘J)
stripes-spring = Spring
stripes-notifications = Notificaciones
stripes-structure-7 = Estructura (⌘7)
stripes-hierarchy-h-h = Jerarquía (⌃H, ⌃⌥H)
stripes-agent-view = Vista del agente
stripes-tests = Pruebas
stripes-project-1 = Proyecto (⌘1)
stripes-git-2 = Git (⌘2)

## git

git-state = Estado
git-relative-directory = Directorio relativo
git-file-filter = Filtrar archivos
git-message-author-sha-or-ref = Mensaje, autor, SHA o ref.
git-commit-message = Mensaje de commit
git-message = Mensaje
git-author = Autor
git-date = Fecha
git-push = Push ({ $n })
git-hide = Ocultar { $short }
git-console-in-this-repository-j = Consola de { $agent } en este repositorio (⌘J)
git-no-github-remote = Sin remoto de GitHub
git-open-local-repository-o = Abrir repositorio local (⌘O)
git-git-push-sets-upstream-on-first-push = git push (define el upstream en el primer push)
git-move-to-the-trash = Mover a la papelera (⌘⌫)
git-branch-tag-or-commit-o = Rama, etiqueta o commit (⇧⌘O)
git-new-branch = Nueva rama
git-no-repository-open = Ningún repositorio abierto
git-local-branches = Ramas locales
git-tags = Etiquetas ({ $count })
git-branches = Ramas
git-check-out-o = Cambiar… (⇧⌘O)
git-double-click-to-check-out = { $b }{ $u }{"\u000A"}{ $target } { $subject }{"\u000A"}{"\u000A"}Doble clic para cambiar
git-double-click-to-check-out-detached = { $target } { $subject }{"\u000A"}{ $date }{"\u000A"}{"\u000A"}Doble clic para cambiar (HEAD separado)
git-delete = Eliminar…
git-delete-from = Eliminar de { $remote }…
git-check-out-as-local = Cambiar a la rama local «{ $local_name }»
git-delete-branch = Eliminar rama…
git-delete-from-the-remote = Eliminar del remoto…
git-create-branch-from-here = Crear rama desde aquí…
git-copy-name = Copiar nombre
git-check-out-detached = Cambiar (HEAD separado)
git-create-branch-from-tag = Crear rama desde la etiqueta…
git-tag = etiqueta { $name }
git-changed-staged = { $count } cambiados, { $staged_count } preparados
git-nothing-to-commit-on = Nada para hacer commit en { $head }
git-nothing-to-commit = Nada para hacer commit
git-show-all-files = Mostrar todos los archivos
git-in = en { $dir }/
git-the-working-tree-is-clean = El árbol de trabajo está limpio
git-discard = Descartar…
git-add-to-gitignore = Añadir a .gitignore
git-see-console = Ver la consola
git-writing = Escribiendo…
git-writes-the-message-from-building-on-anything-you = { $agent } escribe el mensaje a partir de { $what }, aprovechando lo que hayas escrito. Si no está en marcha, se inicia en la consola.
git-draft = Redactar
git-amend = Enmendar
git-amend-last-commit = Enmendar el último commit
git-commits = Hace commit de { $what }  ⌘↩
git-files-selected = { $count } archivos seleccionados
git-select-a-file-to-see-its-changes = Selecciona un archivo para ver sus cambios
git-changes = Cambios
git-nothing-staged = Nada preparado
git-stage-files-to-see-what-the-next-commit = Prepara archivos para ver qué incluirá el próximo commit.
git-journal = Historial
git-click-a-commit-to-see-what-it-changed = Haz clic en un commit para ver qué cambió
git-no-commits-yet = Aún no hay commits
git-stage-some-files-and-make-the-first-one = Prepara algunos archivos y haz el primero
git-show-changes = Mostrar cambios
git-check-out-this-commit-detached = Cambiar a este commit (HEAD separado)
git-create-branch-here = Crear rama aquí…
git-copy-sha = Copiar SHA
git-copy-message = Copiar mensaje
git-working-tree = Árbol de trabajo
git-back-esc = Atrás (Esc)
git-the-commit-could-not-be-read = No se pudo leer el commit
git-from = { $directory }  (desde { $orig })
git-this-commit-changes-no-files = Este commit no cambia ningún archivo
git-changed = { $count } cambiados
git-committer = Committer
git-committed = Fecha del commit
git-parents = Padres
git-parent = Padre
git-none-this-is-the-first-commit = ninguno, es el primer commit
git-select-a-file = Selecciona un archivo
git-nothing = (nada)
git-this-commit = Este commit
git-git-could-not-read-this-folder = Git no pudo leer esta carpeta
git-the-project-works-without-it-but-its-changes = El proyecto funciona sin él, pero sus cambios e historial no se pueden mostrar.
git-this-folder-has-no-git-repository = Esta carpeta no tiene ningún repositorio Git
git-the-project-works-without-git-start-a-repository = El proyecto funciona sin Git. Crea un repositorio para seguir sus cambios e historial.
git-try-again = Reintentar
git-initialize-a-repository-here = Inicializar un repositorio aquí
git-find-repositories-inside = Buscar repositorios dentro
git-open-a-project-to-see-its-changes-branches = Abre un proyecto para ver sus cambios, ramas e historial.
git-open-project-o = Abrir proyecto…   ⌘O
git-unified = Unificado
git-side-by-side = En paralelo
git-compact-hide-unchanged-lines-away-from-changes = Compacto: ocultar las líneas sin cambios alejadas de los cambios
git-compact = Compacto
git-hide-unchanged-lines-away-from-changes = Ocultar las líneas sin cambios alejadas de los cambios
git-no-diff = Sin diff
git-binary-file = Archivo binario
git-its-versions-differ-as-bytes = Sus versiones difieren en bytes.
git-too-large-to-show = Demasiado grande para mostrarlo
git-the-file-is-too-large-to-diff-here = El archivo es demasiado grande para hacer diff aquí.
git-no-differences = Sin diferencias
git-and-are-the-same = { $left } y { $right } son iguales.

## console

console-console = Consola
console-the-installer-is-still-at-work-end-stops = El instalador sigue trabajando; Finalizar lo detiene
console-ended = { $name } terminó
console-ended-resume = { $name } terminó; Continuar lo reanuda
console-your-own-agent-has-no-command-yet-give = Tu agente aún no tiene comando. Dale uno en Configuración, Agentes, y vuelve a hacer clic en Redactar.
console-find-in-terminal = Buscar en el terminal
console-of = { $i } de { $n }
console-previous-match-return = Coincidencia anterior (Intro)
console-next-match-shift-return = Coincidencia siguiente (Mayús+Intro)
console-close-esc = Cerrar (Esc)
console-open-a-project-to-run-a-shell-in = Abre un proyecto para ejecutar un shell en él.
console-not-running = Detenido
console-starting = Iniciando
console-connected = Conectado
console-not-connected = No conectado
console-running = En ejecución
console-working = Trabajando
console-installing = Instalando
console-ended-2 = Terminado
console-is-not-running = { $agent } no está en marcha
console-is-starting = { $agent } se está iniciando
console-sees-the-editor-its-selection-open-files-and = { $agent } ve el editor: su selección, archivos abiertos y problemas; propone allí cambios para que los aceptes o rechaces, y sus ediciones aparecen a medida que las hace
console-runs-but-has-not-connected-to-the-editor = { $agent } está en marcha, pero aún no se ha conectado al editor
console-runs-the-files-it-changes-show-as-it = { $agent } está en marcha; los archivos que cambia aparecen mientras trabaja
console-is-at-work-on-your-prompt = { $agent } está trabajando en tu prompt
console-is-being-installed = { $agent } se está instalando
console-has-ended = { $agent } ha terminado
console-the-shell = El shell
console-follow = Seguir
console-following-the-editor-goes-to-the-files-the = Siguiendo: el editor va a los archivos que cambia el agente, mientras escribe
console-not-following-the-editor-stays-where-you-are = Sin seguir: el editor se queda donde estás; unos puntos marcan lo que cambió el agente
console-start-again-with-a-new-conversation = Empezar de nuevo con una conversación nueva
console-start-a-new-shell = Iniciar un shell nuevo
console-new = Nuevo
console-continue = Continuar
console-start-again-on-the-most-recent-conversation-in = Reanudar la conversación más reciente de esta carpeta
console-end = Finalizar
console-end-this-session = Finalizar esta sesión
console-hide-j-the-session-keeps-running = Ocultar (⌘J); la sesión sigue en marcha
console-starting-the-shell = Iniciando el shell…
console-starting-2 = Iniciando { $agent }…
console-the-shell-is-not-running = El shell no está en marcha
console-start-a-shell-in-the-project = Inicia un shell en el proyecto.
console-start-shell = Iniciar shell
console-start-claude = Iniciar Claude
console-start-codex = Iniciar Codex
console-start-opencode = Iniciar opencode
console-start-amp = Iniciar Amp
console-start-agent = Iniciar agente
console-it-sees-the-editor-and-its-edits-type = Ve el editor, y sus ediciones se escriben solas, en su color, para que las revises.
console-the-files-it-changes-type-themselves-in-in = Los archivos que cambia se escriben solos, en su color, para que los revises.
console-continue-last-session = Continuar la última sesión
console-install-here = Instalar ejecuta su instalador aquí, en la consola. Si está instalado en otro lugar, indica su ruta en Configuración, Agentes.
console-install-here-node = Instalar ejecuta su instalador aquí, en la consola. Necesita Node.js. Si está instalado en otro lugar, indica su ruta en Configuración, Agentes.
console-is-not-installed = { $agent } no está instalado
console-set-its-path-in-settings-agents = Indica su ruta en Configuración, Agentes.
console-your-own-agent-has-no-command-yet = Tu agente aún no tiene comando
console-name-the-command-line-agent-to-run-here = Indica el agente de línea de comandos que ejecutar aquí y el nombre que mostrar, en Configuración, Agentes.
console-agent-settings = Configuración del agente
console-set-its-path = Indicar su ruta
console-s-developers-recommend-running-it-in-wsl = Los desarrolladores de { $name } recomiendan ejecutarlo en WSL.
console-looking-for-in-wsl = Buscando { $name } en WSL…
console-open-a-folder-in = Abrir una carpeta en { $d }
console-is-installed-in = { $name } está instalado en { $and_value }
console-open-a-folder-in-and-the-console-runs = { $makers } Abre una carpeta en { $has } y la consola ejecutará { $name } allí.
console-this-pc-has-no-wsl-distribution-yet-wsl = { $makers } Este PC aún no tiene ninguna distribución de WSL: `wsl --install` en PowerShell instala una. Para ejecutar { $name } en Windows, instálalo con la línea de abajo o indica su ruta.
console-on-windows-itself = En Windows
console-install-runs-its-installer-in-then-open-a = { $makers } Instalar ejecuta su instalador en { $first }; luego abre una carpeta allí y la consola ejecutará { $name } en ella. En Windows, se instala con la segunda línea.
console-install-in = Instalar en { $d }
console-in-wsl = En WSL
console-the-installer = El instalador
console-try-again = Reintentar
console-start-a-new-session = Iniciar una sesión nueva
console-open-link = Abrir enlace
console-copy-link = Copiar enlace
console-install-claude-code = Instalar Claude Code
console-install-codex = Instalar Codex
console-install-opencode = Instalar opencode
console-install-amp = Instalar Amp
console-install = Instalar
console-starts-with-the-next-session = { $agent } se iniciará en la próxima sesión
console-draft-stopped-the-console-started-a-new-session = Redacción detenida: la consola inició una sesión nueva. Haz clic en Redactar para pedírselo a { $name }.
console-could-not-start = No se pudo iniciar { $program }: { $e }
console-is-not-installed-yet-install-it-in-the = { $agent } aún no está instalado. Instálalo en la consola y vuelve a hacer clic en Redactar.
console-is-not-running-2 = { $agent } no está en marcha.
console-set-up = { $a } (configurar)
console-install-2 = { $a } (instalar)

## workspace

workspace-could-not-make = No se pudo crear { $path }: { $e }
workspace-do-you-want-to-save-the-changes-you = ¿Quieres guardar los cambios hechos en { $one }?
workspace-do-you-want-to-save-the-changes-you-2 = ¿Quieres guardar los cambios hechos en { $count } archivos?
workspace-your-changes-are-lost-if-you-dont-save = Tus cambios se perderán si no los guardas.
workspace-dont-save = No guardar
workspace-open-a-project-to-see-its-structure = Abre un proyecto para ver su estructura
workspace-settings-on-machine = Tu settings.json está en { $machine }: ábrelo desde una ventana en { $machine }
workspace-these-settings-are-not-kept-in-a-file = Esta configuración no se guarda en un archivo
workspace-open-a-project-to-edit-its-settings = Abre un proyecto para editar su configuración
workspace-the-projects-settings-are-read-again = La configuración del proyecto se ha vuelto a leer
workspace-has-an-error-so-its-last-good-values = { $file } tiene un error, así que siguen sus últimos valores válidos: { $e }
workspace-reading-the-projects-folders = Leyendo las carpetas del proyecto
workspace-reading-the-open-file-and-the-projects-folders = Leyendo el archivo abierto y las carpetas del proyecto
workspace-reading-of-open-files-and-the-projects-folders = Leyendo { $n } de { $total } archivos abiertos y las carpetas del proyecto
workspace-nothing-changed-on-disk = No cambió nada en el disco
workspace-1-file-changed-on-disk = 1 archivo cambió en el disco
workspace-files-changed-on-disk = { $n } archivos cambiaron en el disco
workspace-drop-a-folder-to-open-it = Suelta una carpeta para abrirla

## tools

tools-problems = Problemas
tools-references = Referencias
tools-show-output = Mostrar la salida
tools-no-notifications = Sin notificaciones
tools-errors-from-git-and-the-tools-show-here = Los errores de Git y las herramientas aparecen aquí.

## state

state-modified = Modificado
state-added = Añadido
state-deleted = Eliminado
state-renamed = Renombrado
state-copied = Copiado
state-type-changed = Tipo cambiado
state-untracked = Sin seguimiento
state-conflicted = En conflicto
state-staged = Preparado
state-partly-staged = Preparado en parte

## splash

splash-starting-snoutcode = Iniciando SnoutCode

## updates

updates-snoutcode-update = Actualización de SnoutCode
updates-updated-to-v = Actualizado a v{ $version }
updates-dismiss = Descartar
updates-later = Más tarde
updates-download = Descargar
updates-restart-to-install-the-update = Reiniciar para instalar la actualización
updates-restart-to-update = Reiniciar para actualizar

## background

background-reading-the-gradle-model = Leyendo el modelo de Gradle
background-compiling-scala = Compilando Scala
background-starting-java = Iniciando Java
background-indexing = Indexando
background-restarting-java = Reiniciando Java
background-waiting-for-snout-engine = Esperando a snout-engine
background-attempt-of-5 = {" "}(intento { $n } de 5)
background-the-java-engine-stopped-unexpectedly-and-is-starting = El motor de Java se detuvo inesperadamente y se está reiniciando{ $again }: { $doing }
background-java-stopped-starting-again = Java se detuvo, reiniciando

## quit

quit-debugging = Depuración
quit-gradle-build = Compilación de Gradle
quit-compiling-the-scala-classes = Compilando las clases de Scala
quit-claude-code-editing = Claude Code, editando { $name }
quit-the-connection-to-and-what-the-project-runs = La conexión con { $host } y lo que el proyecto ejecuta allí
quit-a-file-could-not-be-saved-so-snoutcode = No se pudo guardar un archivo, así que SnoutCode no se cerró

## terminal

terminal-custom-command = Comando personalizado
terminal-command-prompt = Símbolo del sistema
terminal-the-custom-external-terminal-has-no-command-set = El terminal externo personalizado no tiene comando: define uno en Configuración ▸ Herramientas ▸ Terminal.
terminal-could-not-run-the-external-terminal-command = No se pudo ejecutar el comando del terminal externo: { $e }
terminal-could-not-open-the-external-terminal = No se pudo abrir el terminal externo: { $e }
terminal-could-not-open-the-external-terminal-2 = No se pudo abrir el terminal externo: { $trim }

## debug

debug-breakpoints-go-in-a-java-kotlin-scala-typescript = Los puntos de interrupción van en un archivo Java, Kotlin, Scala, TypeScript o JavaScript
debug-there-is-no-breakpoint-on-this-line-f8 = No hay ningún punto de interrupción en esta línea: ⌘F8 pone uno
debug-no-spring-bean-injection-or-endpoint-on-this = Ningún bean, inyección o endpoint de Spring en esta línea
debug-run-to-cursor-works-while-the-debugger-is = Ejecutar hasta el cursor funciona con el depurador detenido
debug-put-the-caret-on-a-line-of-code = Coloca el cursor en una línea de código
debug-nothing-to-pause-no-program-is-running-in = Nada que pausar: ningún programa se ejecuta en el depurador
debug-nothing-ran-yet-to-run-again = Aún no se ha ejecutado nada que repetir
debug-no-failed-tests-to-run-again = No hay pruebas fallidas que repetir
debug-the-maven-goals-are-being-read-try-again = Se están leyendo los goals de Maven; inténtalo de nuevo en un momento
debug-no-maven-goals-the-project-has-no-maven = Sin goals de Maven: el proyecto no tiene compilación de Maven o se está leyendo
debug-the-gradle-tasks-are-being-read-try-again = Se están leyendo las tareas de Gradle; inténtalo de nuevo en un momento
debug-no-gradle-tasks-yet-the-projects-build-is = Aún no hay tareas de Gradle: se está leyendo la compilación del proyecto
debug-on = Activado
debug-any-profile-every-bean-may-be-there = Cualquier perfil: cualquier bean puede estar
debug-as-the-application-files-say = Según los archivos de la aplicación ({ $join })
debug-the-maven-build-is-being-read-try-again = Se está leyendo la compilación de Maven; inténtalo de nuevo en un momento

## lang

lang-symbols-come-from-the-java-server-which-starts = Los símbolos vienen del servidor de Java, que se inicia con un archivo Java
lang-file-structure-shows-the-symbols-of-a-java = Estructura del archivo muestra los símbolos de un archivo Java
lang-classes-come-from-the-java-engine-which-starts = Las clases vienen del motor de Java, que se inicia con un proyecto de Gradle
lang-hierarchies-come-from-the-java-engine-in-a = Las jerarquías vienen del motor de Java, en un archivo Java
lang-downloading-jdk = Descargando JDK { $major }… { $pct } %
lang-downloading-jdk-2 = Descargando JDK { $major }…
lang-jdk-could-not-be-downloaded = No se pudo descargar JDK { $major }: { $error }
lang-this-project-needs-jdk-download-it = Este proyecto necesita JDK { $major }. ¿Descargarlo?
lang-not-now = Ahora no
lang-downloading-node = Descargando Node { $prompt }… { $pct } %
lang-downloading-node-2 = Descargando Node { $prompt }…
lang-node-could-not-be-downloaded = No se pudo descargar Node { $prompt }: { $error }
lang-this-project-needs-node-which-is-not-on = Este proyecto necesita Node { $prompt }, que no está en esta máquina. ¿Descargarlo para SnoutCode?
lang-installing-the-packages-of-the-build-window-shows = Instalando los paquetes de { $dir }… La ventana de compilación muestra el progreso.
lang-installing-the-projects-packages-the-build-window-shows = Instalando los paquetes del proyecto… La ventana de compilación muestra el progreso.
lang-installing-playwrights-browsers-the-build-window-shows-h = Instalando los navegadores de Playwright… La ventana de compilación muestra el progreso.
lang-the-packages-of-are-not-installed-so-it = Los paquetes de { $dir } no están instalados, así que aún no se puede ejecutar.
lang-this-playwright-projects-packages-are-not-installed-so = Los paquetes de este proyecto de Playwright no están instalados, así que sus pruebas aún no se pueden ejecutar.
lang-this-projects-packages-are-not-installed-so-it = Los paquetes de este proyecto no están instalados, así que aún no se puede ejecutar.
lang-install-packages = Instalar paquetes ({ $command })
lang-playwright-needs-browsers-that-are-not-downloaded = Playwright { $version } necesita navegadores que no se han descargado: { $join }.
lang-install-browsers = Instalar navegadores
lang-pom-changed = Cambió un pom.xml. ¿Recargar el proyecto de Maven?
lang-build-files-changed = Cambiaron archivos de compilación. ¿Sincronizar?
lang-gradle-files-changed = Cambiaron archivos de Gradle. ¿Sincronizar?
lang-the-sync-failed = La sincronización falló: { $e }
lang-reload = Recargar
lang-sync = Sincronizar
lang-library-classes-open-once-the-java-engine-is = Las clases de biblioteca se abren cuando el motor de Java está listo
lang-the-java-engine-does-not-know = El motor de Java no conoce { $qualified }

## record

record-works-in-a-playwright-project = { $what } funciona en un proyecto de Playwright
record-open-a-spec-file-and-put-the-caret = { $what }: abre un archivo de spec y coloca el cursor donde deba ir
record-a-recording-is-going-stop-it-first = Hay una grabación en curso: detenla primero
record-it-did-not-start = No se inició
record-the-recorder-stopped-the-build-window-says-why = El grabador se detuvo: la ventana de compilación dice por qué
record-the-recording-ended-with-no-steps = La grabación terminó sin pasos
record-recorded-1-step-into = 1 paso grabado en { $unwrap_or_default }
record-recorded-steps-into = { $n } pasos grabados en { $unwrap_or_default }
record-picking-is-on-click-an-element-in-the = Selección activa: haz clic en un elemento del navegador
record-pick-locator-stopped = Elegir localizador se detuvo
record-picked = { $locator } elegido
record-ui-mode-works-in-a-playwright-project = El modo UI funciona en un proyecto de Playwright
record-ui-mode-opens-in-your-browser = El modo UI se abre en tu navegador

## remote

remote-could-not-connect-to = No se pudo conectar con { $name }: { $why }

## run

run-is-not-among-the-projects-sources = { $class } no está entre las fuentes del proyecto
run-the-file-in-front-has-no-tests-main = El archivo en primer plano no tiene pruebas, clase principal ni script que ejecutar
run-nothing-to-run-here-put-the-caret-in = Nada que ejecutar aquí: coloca el cursor en una clase principal, una prueba o un script
