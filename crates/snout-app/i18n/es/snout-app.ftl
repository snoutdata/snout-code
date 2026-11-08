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
settings-group-build-tools = Compilación
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

settings-search-settings = Buscar configuración
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
settings-every-build = Cada compilación
settings-jdk-per-project = JDK por proyecto
settings-no-gradle-build-is-open = No hay ninguna compilación de Gradle abierta.
settings-automatic-the-builds-java-level = Automático (el nivel de Java de la compilación)
settings-the-java-server-restarts-on-the-chosen-jdk = El servidor de Java usará el JDK elegido la próxima vez que se inicie.
settings-gradle-daemons = Daemons de Gradle
settings-stop-idle-daemons-after = Detener si inactivos
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
settings-its-colour-caret-and-marks = Color, cursor y marcas
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
settings-scrollback = Historial
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
settings-hover-hint = Para los problemas y sus correcciones, y qué es un símbolo. ⌘ siempre lo muestra al instante, con el enlace a la declaración.
settings-click-hint = Manténgala pulsada para subrayar un nombre como enlace y haga clic para ir a su declaración (en la declaración, a sus usos). ⌥⌘-clic lleva a las implementaciones.
settings-blame-hint = Del blame de Git del archivo. El autor de la línea aparece cuando el cursor se detiene durante el retardo de la información emergente (Editor ▸ Información emergente). Anotar con Git Blame está en el menú contextual del margen y en el menú Git.
settings-daemons-hint = Los daemons de Gradle de SnoutCode se detienen tras este tiempo sin compilar. Un valor nuevo se aplica a los daemons iniciados después de que el motor Java arranque de nuevo.
settings-profiles-hint = La ventana Spring evalúa los beans con estos perfiles y atenúa los que quedan fuera. Su menú de perfiles cambia este mismo ajuste.
settings-agent-colours-hint = Cada agente escribe en su color: un cursor con su nombre mientras escribe, una marca en el margen junto a las líneas que cambió y un punto en las pestañas y archivos que cambió hasta que los revise.
settings-programs-hint = Déjelos vacíos y SnoutCode encontrará cada programa, en su PATH y donde los instaladores lo colocan. Examine para elegir uno usted mismo.
settings-terminal-scope-hint = El aspecto, el motor y las teclas de los terminales, y el terminal externo, son suyos: son iguales en todos los proyectos.
settings-engine-hint = El motor lee lo que se ejecuta en los terminales y en la consola de Claude Code. Un cambio se aplica a los terminales que abra después; los abiertos conservan el motor con el que empezaron.
settings-ghostty-preview-hint = Ghostty también dibuja tachada la llamada obsoleta, su línea de advertencia ondulada y el enlace del informe de pruebas.
settings-custom-terminal-hint = Se ejecuta con /bin/sh. { "{dir}" } es la carpeta; en una máquina remota, { "{ssh}" } es el comando ssh que abre allí un shell.
settings-external-terminal-hint = Abrir en un terminal externo, en el menú Archivo y en el menú de un archivo o carpeta, lo abre en esa carpeta. En un proyecto remoto abre allí una sesión ssh.

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
status-the-editor-works-meanwhile = El editor sigue disponible.
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
find-file-mask-java-test-java = Máscara: *.java, !*Test.java
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
model-sources = Fuentes
model-test-sources = Fuentes de prueba
model-test-resources = Recursos de prueba
model-generated-sources = Fuentes generadas
model-the-build-did-not-sync = La compilación no se sincronizó
model-sync = Sincronización: { $root }
model-the-gradle-sync-failed = La sincronización de Gradle falló

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
git-changed = { $count ->
    [one] { $count } cambiado
   *[other] { $count } cambiados
    }
git-changed-staged = { $count ->
    [one] { $count } cambiado
   *[other] { $count } cambiados
    }, { $staged_count ->
    [one] { $staged_count } preparado
   *[other] { $staged_count } preparados
    }

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
console-open-a-project-to-run-in-it = Abre un proyecto para ejecutar { $agent } en él.

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
background-building-snout-engine = Compilando snout-engine

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
debug-stop-only-when-this-is-true-as-i = Detener solo si esto es verdadero, como `i > 3`
debug-if = si { $c }
debug-remove = Quitar
debug-no-breakpoints = Sin puntos de interrupción
debug-click-left-of-a-line-number-or-press = Haz clic a la izquierda de un número de línea, o pulsa ⌘F8 en un editor, para poner uno.
debug-condition-of = Condición de { $path }:{ $line }
debug-breakpoints-count = { $count ->
    [one] { $count } punto de interrupción
   *[other] { $count } puntos de interrupción
    }
debug-breakpoints-disabled = { $breakpoints }, { $off } deshabilitados
debug-untick-one-to-keep-it-without-stopping-there = Desmarca uno para conservarlo sin detenerte ahí. Un doble clic muestra su línea.
debug-remove-all = Quitar todos
debug-go-to-line = Ir a la línea
debug-an-expression-as-total-2 = Una expresión, como `total * 2`
debug-evaluate-works-while-the-program-is-stopped-at = Evaluar funciona con el programa detenido en un punto de interrupción: depúralo (⌃D) y pon uno (⌘F8).
debug-press-to-evaluate-it-in-the-frame-the = Pulsa ⏎ para evaluarla en el marco donde se detuvo el depurador.
debug-add-it-to-the-watches-to-open-its = Añádela a las inspecciones (⌘⏎) para abrir sus campos en la ventana de depuración.
debug-evaluate-expression = Evaluar expresión
debug-java-in-the-frame-the-debugger-stopped-in = Java, en el marco donde se detuvo el depurador.
debug-add-to-watches = Añadir a inspecciones
debug-evaluate = Evaluar
debug-could-not-connect-the-debugger = No se pudo conectar el depurador: { $e }
debug-debugger-attached = Depurador conectado
debug-the-debugger-could-not-attach = El depurador no pudo conectarse: { $e }
debug-nothing-to-reload = Nada que recargar
debug-reloaded = { $classes } recargada
debug-reloaded-classes = { $n } clases recargadas
debug-could-not-reload-the-changed-classes = No se pudieron recargar las clases cambiadas: { $e }

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
remote-trust-this-host = ¿Confiar en este host?
remote-key-passphrase = Frase de contraseña de la clave
remote-no = No
remote-yes = Sí
remote-password = Contraseña:
remote-user-host-a-host-of-your-ssh-config = usuario@host, un Host de tu configuración ssh, o WSL: Ubuntu
remote-user-host-or-a-host-of-your-ssh = usuario@host, o un Host de tu configuración ssh
remote-recent = Recientes
remote-from-your-ssh-config = De tu configuración ssh
remote-connecting-to = Conectando con { $h }…
remote-connecting-to-2 = Conectando con { $host }…
remote-the-first-time-takes-a-little-longer-snoutcode = La primera vez tarda un poco más: SnoutCode instala allí su servidor.
remote-no-hosts-in-your-ssh-config-type-one = No hay hosts en tu configuración ssh: escribe uno.
remote-connect-to-ssh-host-or-wsl = Conectar a un host SSH o WSL
remote-connect-to-ssh-host = Conectar a un host SSH
remote-snoutcode-installs-its-server-on-the-host-the = SnoutCode instala su servidor en el host la primera vez (Linux, x86_64 o aarch64).
remote-host = Host
remote-connect = Conectar
remote-on = { $path } en { $host }
remote-connected-to = Conectado a { $name }
remote-reconnecting-to = Reconectando con { $name }…
remote-reconnecting-to-attempt = Reconectando con { $name }… (intento { $attempt })
remote-the-connection-to-is-lost = Se perdió la conexión con { $name }
remote-disconnected-from = Desconectado de { $name }
remote-the-linux-server-builds-are-missing-run-scripts = Faltan las compilaciones del servidor Linux: ejecuta scripts/build-server-linux.sh
remote-connecting-to-3 = Conectando con { $name }
remote-connecting = Conectando
remote-installing-snout-server = Instalando snout-server
remote-java-engine-on = Motor de Java en { $name }
remote-looking-for-the-engine = Buscando el motor
remote-could-not-install-it = No se pudo instalar: { $e }
remote-open-folder-on = Abrir una carpeta en { $name }
remote-no-folders-here = No hay carpetas aquí
remote-show-hidden-folders = Mostrar carpetas ocultas

## run

run-is-not-among-the-projects-sources = { $class } no está entre las fuentes del proyecto
run-the-file-in-front-has-no-tests-main = El archivo en primer plano no tiene pruebas, clase principal ni script que ejecutar
run-nothing-to-run-here-put-the-caret-in = Nada que ejecutar aquí: coloca el cursor en una clase principal, una prueba o un script
run-root = { $name } (raíz)
run-reading-the-gradle-tasks = Leyendo las tareas de Gradle
run-the-builds-are-being-configured = Se están configurando las compilaciones.
run-the-tasks-could-not-be-read = No se pudieron leer las tareas
run-no-tasks-yet = Aún no hay tareas
run-the-tasks-appear-once-the-java-engine-has = Las tareas aparecen cuando el motor de Java haya importado el proyecto.
run-no-gradle-tasks = Sin tareas de Gradle
run-this-project-has-no-gradle-build = Este proyecto no tiene compilación de Gradle.
run-maven-wrapper = Maven { $version } (wrapper)
run-lifecycle = Ciclo de vida
run-plugins = Plugins
run-delete-the-build-output-target = Eliminar la salida de compilación (target)
run-check-the-project-is-correct-and-complete = Comprobar que el proyecto es correcto y completo
run-compile-the-main-sources = Compilar las fuentes principales
run-run-the-unit-tests = Ejecutar las pruebas unitarias
run-package-the-compiled-code-a-jar-or-war = Empaquetar el código compilado (jar o war)
run-run-the-checks-on-the-package-integration-tests = Ejecutar las comprobaciones sobre el paquete, incluidas las pruebas de integración
run-put-the-package-in-the-local-repository = Poner el paquete en el repositorio local
run-make-the-projects-site = Generar el sitio del proyecto
run-copy-the-package-to-the-remote-repository = Copiar el paquete al repositorio remoto
run-reading-the-maven-goals = Leyendo los goals de Maven
run-the-plugins-goals-are-being-read = Se están leyendo los goals de los plugins.
run-the-goals-could-not-be-read = No se pudieron leer los goals
run-no-goals-yet = Aún no hay goals
run-the-goals-appear-once-the-java-engine-has = Los goals aparecen cuando el motor de Java haya importado el proyecto.
run-no-maven-builds = Sin compilaciones de Maven
run-this-project-has-no-pom-xml = Este proyecto no tiene pom.xml.
run-offline = Sin conexión
run-run-maven-offline-o-only-the-local-repository = Ejecutar Maven sin conexión (-o): solo el repositorio local
run-skip-tests = Omitir pruebas
run-run-goals-without-the-tests-dskiptests = Ejecutar los goals sin las pruebas (-DskipTests)
run-active-in-the-pom = activo en el POM
run-failed = { $label } (fallidas)
run-paused-running = En pausa, en ejecución desde hace { $time }
run-running = En ejecución desde hace { $time }
run-finished-in = Terminado en { $time }
run-failed-after = Falló tras { $time }
run-stopped-after = Detenido tras { $time }
run-tests-count = { $count ->
    [one] { $parts }, { $count } prueba
   *[other] { $parts }, { $count } pruebas
    }
run-the-java-engine-is-not-ready-yet = El motor de Java aún no está listo
run-the-java-engine-stopped-and-this-run-with = El motor de Java se detuvo, y esta ejecución con él.
run-could-not-save-the-output-to = No se pudo guardar la salida en { $path }: { $e }
run-open-a-project-to-run-it = Abre un proyecto para ejecutarlo.
run-has-no-gradle-or-maven-build-runs-tests = { $project } no tiene compilación de Gradle o Maven: las ejecuciones, las pruebas y el depurador la necesitan.
run-java-support-is-off = La compatibilidad con Java está desactivada.
run-the-java-engine-is-not-running = El motor de Java no está en marcha: { $why }
run-starting = Iniciando { $c }
run-running-2 = Ejecutando { $t }
run-running-3 = Ejecutando { $config }
run-testing = Probando { $label }
run-running-4 = Ejecutando { $task }
run-running-mvn-in = Ejecutando mvn { $goals } en { $module }
run-attaching-to-the-node-listening-on-port = Conectando con el Node que escucha en el puerto { $port }
run-waiting-for-the-java-engine-the-run-starts = Esperando al motor de Java ({ $doing }): la ejecución comienza en cuanto esté listo.
run-working-directory = Directorio de trabajo: { $cwd }
run-the-java-engine-is-ready = El motor de Java está listo.
run-working-directory-2 = Directorio de trabajo: { $dir }
run-finished = Terminado
run-failed-3 = Falló
run-stopped = Detenido
run-with-exit-code = {" "}con el código de salida { $code }
run-the-scala-compile-failed-see-problems = La compilación de Scala falló: mira Problemas
run-compiling-the-changed-scala = Compilando el Scala modificado
run-reading-the-tasks = Leyendo las tareas
run-the-engine-does-not-know-this-files-module = El motor aún no conoce el módulo de este archivo
run-downloading-the-javascript-debugger-js-debug = Descargando el depurador de JavaScript (js-debug { $version })
run-the-debugger-could-not-start-the-program = El depurador no pudo iniciar el programa: { $e }
run-detached = Desconectado
run-could-not-connect-the-debugger-to-a-node = No se pudo conectar el depurador a un proceso de Node: { $e }
run-npm-is-missing-beside-node = Falta npm junto a Node
run-is-not-installed-the-projects-lockfile-is-s = { $name } no está instalado (el lockfile del proyecto es de { $name })
run-attach-to-port = Conectar al puerto { $port }
run-the-configuration-names-no-script = La configuración no indica ningún script
run-the-configuration-names-no-file-to-run = La configuración no indica ningún archivo que ejecutar
run-is-not-there = { $file } no está ahí
run-open-a-project-first = Abre primero un proyecto
run-no-node-fits-this-project-download-it-from = Ningún Node sirve para este proyecto: descárgalo primero desde la barra sobre el editor.
run-did-not-start = { $program } no se inició: { $e }
run-process-finished-with-exit-code = Proceso terminado con el código de salida { $c }
run-process-ended-by-a-signal = Proceso terminado por una señal
run-all-tests = Todas las pruebas
run-tests-in = Pruebas en { $p }
run-open-a-playwright-project-first = Abre primero un proyecto de Playwright
run-passed = { $passed ->
    [one] { $passed } superada
   *[other] { $passed } superadas
    }
run-skipped = { $skipped ->
    [one] { $skipped } omitida
   *[other] { $skipped } omitidas
    }
run-failed-2 = { $failed ->
    [one] { $failed } fallida
   *[other] { $failed } fallidas
    }
tests-watch-label = Observar

## structure

structure-included-build = compilación incluida
structure-compile = Compilación
structure-runtime = Ejecución
structure-test = Prueba
structure-annotation-processor = Procesador de anotaciones
structure-what-the-main-code-compiles-against-compile-provided = Con lo que compila el código principal (ámbitos compile, provided y system).
structure-what-the-main-code-runs-with-compile-and = Con lo que se ejecuta el código principal (ámbitos compile y runtime).
structure-what-the-tests-compile-and-run-with-every = Con lo que se compilan y ejecutan las pruebas (todos los ámbitos).
structure-what-the-main-code-compiles-against-compileclasspath = Con lo que compila el código principal (compileClasspath).
structure-what-the-main-code-runs-with-runtimeclasspath = Con lo que se ejecuta el código principal (runtimeClasspath).
structure-what-the-tests-compile-and-run-with-the = Con lo que se compilan y ejecutan las pruebas (el runtimeClasspath del source set de prueba).
structure-the-annotation-processors-javac-runs-on-the-main = Los procesadores de anotaciones que javac ejecuta sobre el código principal.
structure-project = proyecto { $path }
structure-the-gradle-model-could-not-be-read = No se pudo leer el modelo de Gradle: { $e }
structure-the-gradle-model-is-not-json = El modelo de Gradle no es JSON: { $e }
structure-the-gradle-model-is-of-an-older-format = El modelo de Gradle tiene un formato antiguo: vuelve a sincronizar
structure-edit-in-build-file = Editar en el archivo de compilación
structure-sync-gradle = Sincronizar Gradle
structure-the-pages-fill-in-as-soon-as-the = Las páginas se llenan en cuanto se lee el modelo de la última sincronización.
structure-no-gradle-model-yet = Aún no hay modelo de Gradle
structure-this-project-has-not-been-synced-as-a = Este proyecto no se ha sincronizado como compilación de Gradle. Sincronízalo para ver sus módulos y dependencias.
structure-the-gradle-model-could-not-be-read-2 = No se pudo leer el modelo de Gradle
structure-java = Java
structure-looking-for-jdks = Buscando JDK…
structure-not-set-the-jdks-own = Sin definir (el del JDK)
structure-java-to-per-module = Java { $lo } a { $hi }, por módulo
structure-language-level = Nivel de lenguaje
structure-gradle-and-maven = Gradle y Maven
structure-unknown = Desconocido
structure-did-not-sync = No se sincronizó
structure-maven-version = Versión de Maven
structure-gradle-version = Versión de Gradle
structure-maven-jvm = JVM de Maven
structure-gradle-jvm = JVM de Gradle
structure-not-known-until-the-build-syncs = Se desconoce hasta que se sincronice la compilación
structure-the-jdk-maven-runs-on-and-compiles-with = El JDK con el que Maven se ejecuta y compila, elegido según el nivel de Java de la compilación: el JDK de arriba si está definido.
structure-the-jvm-the-last-sync-ran-gradle-on = La JVM con la que la última sincronización ejecutó Gradle: el JDK de arriba si está definido.
structure-included-build-2 = Compilación incluida
structure-spring-boot = Spring Boot
structure-build-file = Archivo de compilación
structure-not-set = Sin definir
structure-toolchain = toolchain { $t }
structure-target-compatibility = Compat. de destino
structure-this-project-compiles-no-java-it-has-no = Este proyecto no compila Java: no tiene source sets.
structure-java-bytecode = Java { $a }, bytecode { $b }
structure-content-roots = Raíces de contenido
structure-not-there-yet = aún no existe
structure-output-folders = Carpetas de salida
structure-resources = Recursos
structure-classes = Clases
structure-no-module-compiles-java = Ningún módulo compila Java
structure-dependencies-show-for-the-projects-that-apply-a = Las dependencias aparecen en los proyectos que aplican un plugin de Java.
structure-conflicts-only = Solo conflictos
structure-no-scope-dependencies = Sin dependencias: { $scope }
structure-declares-nothing-for-this-scope = { $m } no declara nada para este ámbito. { $m_2 }
structure-declared-in-all = { $declared } declaradas, { $count } en total.
structure-version-conflicts = { $count ->
    [one] { $count } conflicto de versiones
   *[other] { $count } conflictos de versiones
    }
structure-unresolved = { $unresolved_count } sin resolver
structure-no-version-lost-a-conflict-here = Ninguna versión perdió un conflicto aquí.
structure-try-fewer-letters-of-the-group-name-or = Prueba con menos letras del grupo, nombre o versión.
structure-on = en { $h }
structure-on-machine = en { $machine }
structure-download-jdk = Descargar JDK
structure-found-where-jdks-are-installed-and-added-by = Encontrados donde se instalan los JDK { $machine }, o añadidos por ti.
structure-add-from-folder = Añadir desde una carpeta…
structure-downloading-jdk = Descargando JDK { $major }
structure-no-jdk-found = No se encontró ningún JDK
structure-no-jdk-is-installed-where-jdks-usually-are = No hay ningún JDK instalado { $machine } en los lugares habituales. Descarga uno o añade su carpeta.
structure-downloaded = Descargado
structure-detected = Detectado
structure-this-projects = El de este proyecto
structure-remove-from-the-list = Quitar de la lista
structure-no-jdk-there-now = Ya no hay ningún JDK ahí
structure-removing-an-added-jdk-takes-it-off-the = Quitar un JDK añadido lo saca de la lista; sus archivos siguen donde están.
structure-selected-jdk = JDK seleccionado
structure-version = Versión
structure-home = Ubicación
structure-kept-in = Guardado en
structure-your-settings = Tu configuración
structure-found-by-snoutcode = Encontrado por SnoutCode
structure-this-project = Este proyecto
structure-runs-on-it = Se ejecuta con él
structure-use-for-this-project = Usar en este proyecto
structure-not-detected = No detectado
structure-spring-settings = Configuración de Spring
structure-none-with-the-spring-boot-plugin = Ninguno con el plugin de Spring Boot
structure-in-the-spring-model = { $beans } en el modelo de Spring
structure-read-once-the-java-engine-is-ready = Se lee cuando el motor de Java esté listo
structure-profiles-known = Perfiles conocidos
structure-configuration-files = Archivos de configuración
structure-no-application-properties-or-application-yml-in-the = No hay application.properties ni application.yml en los recursos
structure-no-spring-here = Aquí no hay Spring
structure-no-module-applies-the-spring-boot-plugin-and = Ningún módulo aplica el plugin de Spring Boot y el modelo de Spring no tiene beans.
structure-not-found = No encontrado
structure-claude-code-settings = Configuración de Claude Code
structure-claude-was-not-found-on-path-or-the = claude no se encontró en el PATH ni en las ubicaciones habituales
structure-ide-connection = Conexión con el IDE
structure-no-session-connected-a-claude-started-in-the = Ninguna sesión conectada. Un claude iniciado en la consola se conecta solo.
structure-1-session-connected-it-sees-the-open-files = 1 sesión conectada: ve los archivos abiertos, la selección y los problemas
structure-sessions-connected = { $n } sesiones conectadas
structure-the-project-as-a-whole-the-jdk-it = El proyecto en conjunto: tú eliges su JDK, el resto es cosa de Maven.
structure-every-maven-module-of-every-build-with-its = Todos los módulos de Maven de cada compilación, con sus source sets, raíces y salidas.
structure-what-each-module-declares-and-what-that-brings = Lo que declara cada módulo y lo que eso trae, tal como lo resolvió Maven.
structure-the-project-as-a-whole-the-jdk-it-2 = El proyecto en conjunto: tú eliges su JDK, el resto es cosa de Gradle.
structure-every-gradle-project-of-every-build-with-its = Todos los proyectos de Gradle de cada compilación, con sus source sets, raíces y salidas.
structure-what-each-module-declares-and-what-that-brings-2 = Lo que declara cada módulo y lo que eso trae, tal como lo resolvió Gradle.
structure-the-jdks-snoutcode-can-run-gradle-and-the = Los JDK con los que SnoutCode puede ejecutar Gradle y el motor de Java.
structure-frameworks-and-tools-as-they-stand-read-only = Frameworks y herramientas, tal como están. Solo lectura.
structure-unresolved-2 = Sin resolver
structure-show-in-modules = Mostrar { $name } en Módulos
structure-show-in-build-file = Mostrar en el archivo de compilación
structure-show-the-declaration-that-brings-it-in = Mostrar la declaración que lo trae
structure-show-its-dependencies = Mostrar sus dependencias
structure-a-newer-version-asked-for-elsewhere-won = ganó una versión más nueva pedida en otro lugar
structure-dependency-management-chose-it = la gestión de dependencias la eligió
structure-it-is-forced = está forzada
structure-a-constraint-asks-for-it = una restricción la pide
structure-another-request-won = ganó otra petición
structure-was-asked-for-here-is-used = Aquí se pidió { $asked }; se usa { $version }: { $why }
structure-conflict = Conflicto
structure-won-over = ganó a { $join }
structure-show-in-version-catalog = Mostrar en el catálogo de versiones
structure-modules-from-sync = { $count ->
    [one] { $name } · { $count } módulo de la última sincronización de { $tool }
   *[other] { $name } · { $count } módulos de la última sincronización de { $tool }
    }
structure-project-structure = Estructura del proyecto
structure-syncing = Sincronizando…
structure-reload-maven = Recargar Maven
structure-read-the-maven-builds-again = Volver a leer las compilaciones de Maven
structure-read-the-gradle-builds-again = Volver a leer las compilaciones de Gradle
structure-read-the-builds-again = Volver a leer las compilaciones
structure-facets = Facetas
structure-project-settings = Ajustes del proyecto
structure-platform-settings = Plataforma
structure-search = Buscar
structure-filter-dependencies = Filtrar por grupo, nombre o versión
structure-jdk-listed-already = El JDK { $version } ya está en la lista
structure-jdk-added = JDK { $version } añadido. Aplicar lo conserva.
structure-no-jdk-in = No hay ningún JDK en { $dir }: elija la carpeta que contiene la carpeta bin con Java
structure-add-jdk = Añadir JDK
structure-jdk-removed = Quitado de la lista. Aplicar conserva el cambio.
structure-jdk-downloaded = JDK { $version } descargado en { $home }
structure-download-failed = La descarga falló: { $e }
structure-build-file-not-found = No se encontró el archivo de compilación
structure-syncing-note = Sincronizando el modelo de Gradle. Las páginas se actualizarán al terminar.

## newproject

newproject-gradle-or-maven = Gradle o Maven
newproject-from-start-spring-io = Desde start.spring.io
newproject-end-to-end-tests-in-typescript = Pruebas E2E en TypeScript
newproject-project-name = Nombre del proyecto
newproject-folder = Carpeta
newproject-search-dependencies = Buscar dependencias
newproject-waiting-for-start-spring-io = Esperando a start.spring.io…
newproject-start-spring-io-could-not-be-reached = No se pudo acceder a start.spring.io ({ $e }).
newproject-the-project-goes-in = El proyecto se creará en { $target }
newproject-asking-start-spring-io-what-it-offers = Preguntando a start.spring.io qué ofrece…
newproject-application-url = URL de la aplicación
newproject-the-address-the-tests-open-a-base-url = La dirección que abren las pruebas. Una variable de entorno BASE_URL la reemplaza si está definida.
newproject-create-a-git-repository = Crear un repositorio Git
newproject-build-tool = Herramienta
newproject-coordinates = Coordenadas
newproject-none-chosen-yet = Aún no se ha elegido ninguna.
newproject-no-dependency-matches = Ninguna dependencia coincide.
newproject-dependencies = Dependencias
newproject-spring-boot-java-choose-what-the-application-uses = Spring Boot { $version }, Java { $java }. Elige lo que usa la aplicación; se pueden añadir más a la compilación después.
newproject-getting-the-project-from-start-spring-io = Obteniendo el proyecto de start.spring.io…
newproject-creating-the-project = Creando el proyecto…
newproject-new-project = Nuevo proyecto
newproject-a-java-or-spring-boot-project-built-with = Un proyecto Java o Spring Boot, compilado con Gradle o Maven.
newproject-create-without-the-network = Crear sin red
newproject-previous = Anterior
newproject-next = Siguiente
newproject-already-exists-and-is-not-empty = { $dir } ya existe y no está vacío.
newproject-give-the-project-a-name = Dale un nombre al proyecto.
newproject-the-name-cannot-start-with-a-dot-or = El nombre no puede empezar por un punto ni contener / : o \.
newproject-choose-where-the-project-goes = Elige dónde va el proyecto.
newproject-the-group-is-a-dotted-name-such-as = El grupo es un nombre con puntos, como com.example.
newproject-the-artifact-holds-letters-digits-dots-dashes-and = El artefacto contiene letras, dígitos, puntos, guiones y guiones bajos.
newproject-the-package-is-a-dotted-java-name-such = El paquete es un nombre Java con puntos, como com.example.demo.
newproject-the-applications-address-starts-with-http-or-https = La dirección de la aplicación empieza por http:// o https://.

## runwin

runwin-evaluate-an-expression = Evaluar una expresión
runwin-clear-all = Borrar todo
runwin-copy-all = Copiar todo
runwin-save-output-to-file = Guardar la salida en un archivo…
runwin-rerun-r = Volver a ejecutar (⌘R)
runwin-rerun-failed-tests-r = Volver a ejecutar las pruebas fallidas (⌥⌘R)
runwin-test-results = Resultados
runwin-passed = Superada
runwin-skipped = Omitida
runwin-nothing-printed = { $what }; no imprimió nada
runwin-expected = Esperado
runwin-actual = Obtenido
runwin-errors-warnings = { $e } errores, { $w } advertencias
runwin-up-to-date = actualizado
runwin-from-cache = desde la caché
runwin-no-source = sin fuente
runwin-debugger = Depurador
runwin-resume-f9 = Reanudar (F9)
runwin-pause-program-f9 = Pausar el programa (⌥⌘F9)
runwin-step-over-f8 = Paso por encima (F8)
runwin-step-into-f7 = Paso hacia dentro (F7)
runwin-step-out-f8 = Paso hacia fuera (⇧F8)
runwin-the-program-ended = El programa terminó.
runwin-waiting-for-the-program-to-start = Esperando a que se inicie el programa…
runwin-the-program-is-running-breakpoints-stop-it-here = El programa está en marcha. Los puntos de interrupción lo detienen aquí.
runwin-add-to-watches = Añadir a inspecciones (⇧↩)
runwin-nothing-running = Nada en ejecución
runwin-run-a-configuration-from-the-title-bar-or = Ejecuta una configuración desde la barra de título, o una prueba desde el margen.
runwin-no-debug-session = Sin sesión de depuración
runwin-debug-a-configuration-from-the-title-bar-or = Depura una configuración desde la barra de título, o una prueba desde el margen.
runwin-no-build-output = Sin salida de compilación
runwin-build-the-project-or-run-one-of-its = Compila el proyecto o ejecuta una de sus tareas de Gradle o goals de Maven.
runwin-run-task = Ejecutar tarea…

## spring

spring-any-profile = Cualquier perfil
spring-from-the-files = { $config_active } (según los archivos)
spring-text = { $name } ({ $from })
spring-consumes = {"\u000A"}Consume { $join }
spring-produces = {"\u000A"}Produce { $join }
spring-bean-not-active-with-the-active-profiles = Bean '{ $b }' ({ $stereotype_label }): inactivo con los perfiles activos
spring-not-injected-anywhere = no se inyecta en ningún sitio
spring-injected-in-1-place = inyectado en 1 lugar
spring-injected-in-places = inyectado en { $n } lugares
spring-bean = Bean '{ $b }' ({ $stereotype_label }), { $used }
spring-text-2 = { $what }: { $value }
spring-bean-2 = { $what }: bean '{ $one }'
spring-no-bean = { $what }: ningún bean
spring-beans = { $what }: { $count } beans
spring-all-sources = Todas las fuentes
spring-default-package = (paquete predeterminado)
spring-search-beans-by-name-or-type = Buscar beans por nombre o tipo
spring-search-paths-and-handlers = Buscar rutas y handlers
spring-all-stereotypes = Todos los estereotipos
spring-no-profile = Sin perfil
spring-profile = Perfil { $p }
spring-inactive = { $inactive } inactivos
spring-by-package = Por paquete
spring-by-stereotype = Por estereotipo
spring-no-beans = Sin beans
spring-the-project-declares-no-spring-bean-yet = El proyecto aún no declara ningún bean de Spring.
spring-no-bean-matches = Ningún bean coincide
spring-try-fewer-letters-or-another-stereotype-or-source = Prueba con menos letras u otro estereotipo o fuente.
spring-beans-from-libraries = { $count } beans, { $library } de bibliotecas
spring-not-injected-anywhere-2 = No se inyecta en ningún sitio
spring-injected-in-1-place-2 = Inyectado en 1 lugar
spring-injected-in-places-2 = Inyectado en { $n } lugares
spring-profiles = {"\u000A"}Perfiles: { $join }
spring-not-active-note = {"\u000A"}Inactivo: los perfiles activos lo dejan fuera
spring-primary = Principal
spring-of-beans = { $shown } de { $bean_count } beans
spring-method-all = Todos
spring-method = Método
spring-path = Ruta
spring-handler = Handler
spring-no-endpoints = Sin endpoints
spring-no-controller-of-the-project-maps-a-request = Ningún controlador del proyecto mapea aún una petición.
spring-no-endpoint-matches = Ningún endpoint coincide
spring-try-fewer-letters-of-the-path-or-the = Prueba con menos letras de la ruta o el método.
spring-of-endpoints = { $shown } de { $total } endpoints
spring-as-runs = Como se ejecuta '{ $c }' ({ $join })
spring-read-the-spring-model = Leer el modelo de Spring
spring-reading-the-spring-model = Leyendo el modelo de Spring
spring-the-beans-and-endpoints-are-being-read = Se están leyendo los beans y endpoints.
spring-no-spring-model-yet = Aún no hay modelo de Spring
spring-beans-and-endpoints-appear-once-the-java-engine = Los beans y endpoints aparecen cuando el motor de Java haya importado el proyecto.
spring-beans-2 = Beans { $bean_count }
spring-endpoints = Endpoints { $count }
spring-reading = Leyendo…
spring-data-repository = Repositorio de datos

## config

config-arguments-the-program-gets = Argumentos que recibe el programa
config-9229-empty-run-the-file = 9229 (vacío: ejecutar el archivo)
config-unnamed = Sin nombre { $next }
config-no-configuration-add-one-with = Ninguna configuración. Añade una con +.
config-choose-a-module = Elegir un módulo
config-kind = Tipo
config-main-class = Clase principal
config-goals = Goals
config-script = Script
config-file = Archivo
config-task = Tarea
config-gradle-arguments = Argumentos de Gradle
config-maven-arguments = Argumentos de Maven
config-playwright-arguments = Argumentos de Playwright
config-script-arguments = Argumentos del script
config-program-arguments = Argumentos del programa
config-node-options = Opciones de Node
config-vm-options = Opciones de la VM
config-environment-variables = Variables de entorno
config-attach-to-port = Conectar al puerto
config-from-the-build-with-your-changes-remove-puts = Del build, con tus cambios. Quitar la devuelve a como la tiene el build.
config-from-the-build-a-change-you-make-stands = Del build. Un cambio tuyo la reemplaza.
config-your-own-configuration = Tu propia configuración.
config-run-configurations = Configuraciones de ejecución
config-what-run-and-debug-start-an-application-a = Lo que inician Ejecutar y Depurar: una aplicación, una tarea de Gradle, pruebas, un script.
config-add-configuration = Añadir configuración
config-remove-or-put-back-as-the-build-has = Quitar o devolver a como la tiene el build
config-application = Aplicación
config-package-script = Script del paquete
config-scripts = Scripts

## tests

tests-filter-tests = Filtrar pruebas
tests-not-run-yet = Aún sin ejecutar
tests-did-not-start = No se inició
tests-workers = { $count ->
    [one] { $projects } · { $count } worker
   *[other] { $projects } · { $count } workers
    }
tests-about-left = { $elapsed } · quedan unos { $left }
tests-live-run = Ejecución en directo
tests-history = Historial
tests-run-with-the-browsers-showing-one-test-at = Ejecutar con los navegadores visibles, una prueba cada vez
tests-rerun-failed = Repetir fallidas
tests-run-all = Ejecutar todas
tests-no-environment = Sin entorno
tests-environment = Entorno: { $label }, { $d }
tests-environment-2 = Entorno: { $label }
tests-no-environment-the-configurations-own = Sin entorno (el de la configuración)
tests-all = Todas { $total }
tests-failed = Fallidas { $failed }
tests-flaky = Inestables { $flaky }
tests-slow-over-4-s = Lentas, más de 4 s
tests-last-10-runs = Últimas 10
tests-flaky-2 = Inestable
tests-finding-the-tests = Buscando las pruebas…
tests-no-test-matches-the-filter = Ninguna prueba coincide con el filtro.
tests-no-tests-yet-run-them-or-add-a = Aún no hay pruebas. Ejecútalas o añade un archivo de spec.
tests-failed-in = Falló en { $project }
tests-flaky-in = Inestable en { $project }
tests-in = { $other } en { $project }
tests-retries = { $count ->
    [one]  · { $count } reintento
   *[other]  · { $count } reintentos
    }
tests-screenshot-comparison = Comparación de capturas
tests-difference = Diferencia
tests-accept-new-screenshot = Aceptar la captura nueva
tests-runs-the-test-again-keeping-its-screenshots-as = Vuelve a ejecutar la prueba conservando sus capturas como están ahora
tests-steps = Pasos
tests-from-the-trace = Desde la traza
tests-console-error = error de consola
tests-actions-no-failed-requests-or-console-errors = { $count } acciones; sin peticiones fallidas ni errores de consola.
tests-the-trace-could-not-be-read = No se pudo leer la traza: { $why }
tests-reading-the-trace = Leyendo la traza…
tests-open-full-trace = Abrir la traza completa
tests-video = Vídeo
tests-run-this-test = Ejecutar esta prueba
tests-debug-this-test = Depurar esta prueba
tests-debug-in-inspector = Depurar en el inspector
tests-show-in-editor = Mostrar en el editor
tests-copy-error = Copiar el error
tests-when = Cuándo
tests-what = Qué
tests-branch = Rama
tests-time = Tiempo
tests-new-since-the-run-before = Nuevo desde la ejecución anterior
tests-runs-of-this-project = Ejecuciones de este proyecto
tests-kept-on-this-machine-outside-the-repository = Guardadas en esta máquina, fuera del repositorio
tests-no-runs-kept-yet = Aún no se ha guardado ninguna ejecución.
tests-min-ago = hace { $secs } min
tests-h-ago = hace { $secs } h
tests-days-ago = hace { $days } días
tests-the-playwright-test-runner-is-not-installed = El ejecutor de pruebas de Playwright no está instalado.
tests-no-node-fits-this-project = Ningún Node sirve para este proyecto.
tests-trace-viewer = Visor de trazas
tests-no-tests-found = No se encontraron pruebas
tests-open-a-playwright-project-or-a-project-whose = Abre un proyecto de Playwright, o uno cuyos paquetes prueben con Vitest o Jest, para ejecutar sus pruebas aquí.
tests-in-2 = { $runner } en { $rel }
tests-is-not-installed-install-the-packages-first = { $runner } no está instalado: instala primero los paquetes.
tests-the-snoutcode-reporter-could-not-be-written = No se pudo escribir el reportero de SnoutCode: { $e }
tests-tests = { $count } pruebas
tests-failed-tests = Pruebas fallidas
tests-the-playwright-test-runner-is-not-installed-install = El ejecutor de pruebas de Playwright no está instalado: instala primero los paquetes.
tests-playwright-listed-no-tests = Playwright no enumeró ninguna prueba ({ $status })
tests-recording = Grabación
tests-ui-mode = Modo UI
tests-the-snoutcode-locator-picker-could-not-be-written = No se pudo escribir el selector de localizadores de SnoutCode: { $e }
tests-new-since-the-last-run = {" "}({ $fresh } nuevos desde la última ejecución)
tests-passed-failed = { $title }: { $passed } superadas, { $failed } fallidas{ $new }{ $new_2 }
tests-count-passed = { $count ->
    [one] { $count } superada
   *[other] { $count } superadas
    }
tests-count-failed = { $count ->
    [one] { $count } fallida
   *[other] { $count } fallidas
    }
tests-count-flaky = { $count ->
    [one] { $count } inestable
   *[other] { $count } inestables
    }
tests-count-skipped = { $count ->
    [one] { $count } omitida
   *[other] { $count } omitidas
    }
tests-flaky-3 = , { $flaky ->
    [one] { $flaky } inestable
   *[other] { $flaky } inestables
    }

## env

env-environment = Entorno { $count }
env-no-environment-add-one-with = Ningún entorno. Añade uno con +.
env-variables = Variables
env-in-use = en uso
env-where-the-tests-point-the-applications-url-and = A dónde apuntan las pruebas: la URL de la aplicación y las variables que leen.
env-add-environment = Añadir entorno
env-remove-environment = Quitar entorno

## pwsetup

pwsetup-ffmpeg-for-videos = FFmpeg (para los vídeos)
pwsetup-playwright-not-installed = Playwright { $v }, no instalado
pwsetup-no-node-fits = ningún Node sirve
pwsetup-install-packages = Instalar paquetes
pwsetup-install-playwright-browsers = Instalar los navegadores de Playwright
pwsetup-no-node-fits-this-project-download-it-from = Ningún Node sirve para este proyecto: descárgalo primero desde la barra de arriba.
pwsetup-the-install-did-not-finish-see-the-build = La instalación no terminó: mira la ventana de compilación.

## pwreport

pwreport-queued = En cola
pwreport-flaky-passed-on-a-retry = Inestable, superada en un reintento
pwreport-interrupted = Interrumpida

## processes

processes-snoutcode-the-window-the-editors-and-git = SnoutCode: la ventana, los editores y Git
processes-idle-for-min-stops-after = Inactivo desde hace { $m } min, se detiene tras { $limit }
processes-idle-stops-after-min = Inactivo, se detiene tras { $limit } min
processes-java-engine-for-reads-the-gradle-or-maven = Motor de Java de { $project }: lee la compilación de Gradle o Maven, ejecuta y prueba el código
processes-java-language-server-for-completion-errors-navigation = Servidor de lenguaje Java de { $project }: autocompletado, errores, navegación
processes-maven-for-runs = Maven de { $project }: ejecuta { $title }
processes-maven-for-runs-goals = Maven de { $project }: ejecuta goals
processes-tests-of-the-test-jvm-maven-started = Pruebas de { $title }: la JVM de pruebas iniciada por Maven
processes-tests-for-the-test-jvm-maven-started = Pruebas de { $project }: la JVM de pruebas iniciada por Maven
processes-your-program-started-by-maven = { $title }: tu programa, iniciado por Maven
processes-program-maven-started-for = Programa iniciado por Maven para { $project }
processes-your-program-being-debugged = { $title }: tu programa, en depuración
processes-your-program-started-with-run = { $title }: tu programa, iniciado con Ejecutar
processes-program-started-by-the-java-engine-for = Programa iniciado por el motor de Java para { $project }
processes-gradle-daemon-for-syncs-the-build-runs-tasks = Daemon de Gradle { $version } de { $project }: sincroniza la compilación, ejecuta tareas y pruebas
processes-tests-of-the-test-jvm-gradle-started = Pruebas de { $title }: la JVM de pruebas iniciada por Gradle
processes-tests-for-the-test-jvm-gradle-started = Pruebas de { $project }: la JVM de pruebas iniciada por Gradle
processes-gradle-worker-for-compiles-or-processes-in-a = Worker de Gradle de { $project }: compila o procesa en su propia JVM
processes-your-program-started-by-gradle = { $title }: tu programa, iniciado por Gradle
processes-program-gradle-started-for = Programa iniciado por Gradle para { $project }
processes-in = {" "}en { $p }
processes-java-engine-for-stopping-with-its-language-server = Motor de Java de { $folder }: deteniéndose, con su servidor de lenguaje y daemons de Gradle
processes-terminal-your-shell = Terminal: tu shell { $name }{ $within }
processes-claude-code-the-assistant-in-the-console = Claude Code: el asistente de la consola
processes-started-by-snoutcode = { $name }: iniciado por SnoutCode
processes-java-engine-for-left-over-from-an-earlier = Motor de Java de { $folder }, sobrante de una sesión anterior
processes-java-language-server-left-over-from-an-earlier = Servidor de lenguaje Java, sobrante de una sesión anterior
processes-gradle-daemon-left-over-from-an-earlier-session = Daemon de Gradle { $version }, sobrante de una sesión anterior
processes-nothing-idle-to-stop = Nada inactivo que detener.
processes-stopped-1-idle-process = 1 proceso inactivo detenido.
processes-stopped-idle-processes = { $n } procesos inactivos detenidos.
processes-state-building = Compilando
processes-state-stopping = Deteniéndose
processes-state-left-over = Sobrante
processes-looking-at-the-processes = Examinando los procesos
processes-snoutcodes-own-processes-show-here-the-java-engine = Aquí aparecen los procesos de SnoutCode: el motor de Java, los daemons de Gradle, Maven, las ejecuciones y los terminales.
processes-process = Proceso
processes-memory = Memoria
processes-uptime = Tiempo activo
processes-project = { $purpose }{"\u000A"}Proyecto: { $project }
processes-busy-at-work-now-a-filled-dot = Ocupado: trabajando ahora (punto relleno)
processes-waiting-alive-doing-nothing-now-a-hollow-dot = En espera: activo, sin hacer nada ahora (punto hueco)
processes-summary = { $count ->
    [one] { $count } proceso, { $memory } de memoria, { $cpu } % de CPU
   *[other] { $count } procesos, { $memory } de memoria, { $cpu } % de CPU
    }
processes-idle = { $idle } inactivos

## editor

editor-tab = Tab, { $width }
editor-spaces = { $width } espacios
editor-could-not-read = No se pudo leer { $path }: { $e }
editor-could-not-save = No se pudo guardar { $path }: { $e }
editor-this-file-changed-on-disk-and-this-tab = Este archivo cambió en el disco y esta pestaña tiene cambios sin guardar.
editor-reload-from-disk = Recargar desde el disco
editor-keep-my-changes = Conservar mis cambios
editor-run = Ejecutar '{ $name }'
editor-debug = Depurar '{ $name }'
editor-do-you-want-to-save-the-changes-you = ¿Quieres guardar los cambios hechos en { $name }?
editor-a-librarys-class-file-has-no-git-history = El archivo de clase de una biblioteca no tiene historial de Git.
editor-not-a-git-project-no-git-repository-holds = No es un proyecto Git: ningún repositorio Git contiene este archivo.
editor-not-committed-yet = Aún sin commit
editor-uncommitted = Sin commit
editor-reading-history = Leyendo el historial…
editor-close-annotations = Cerrar anotaciones
editor-breakpoint-condition = Condición del punto de interrupción…
editor-show-diff = Mostrar diff
editor-show-in-git-log = Mostrar en el historial de Git
editor-copy-hash = Copiar hash
editor-copied = { $count } copiado
editor-annotate-previous-revision = Anotar la revisión anterior
editor-the-file-as-it-was-before-this-commit = El archivo como estaba antes de este commit
editor-this-commit-brought-the-file-in = Este commit trajo el archivo
editor-you-changed-this-line-since-the-last-commit = Cambiaste esta línea desde el último commit. Haz commit y la columna lo indicará.
editor-authors-of = Autores de { $decl }
editor-committed-lines = { $total } líneas con commit
editor-last = { $email } · último { $now }
editor-1-line = 1 línea
editor-lines = { $lines } líneas
editor-before = antes de { $short_id }
editor-now = Ahora
editor-back = Atrás
editor-back-to-now = Volver al presente
editor-you-not-committed-yet = Tú, aún sin commit
editor-1-line-removed = 1 línea eliminada
editor-lines-removed = { $n } líneas eliminadas
editor-writer-editing-line = { $writer } edita la línea { $line } { $arrow }
editor-writer-edited-line = { $writer } editó la línea { $line } { $arrow }
editor-claude-code-is-writing-a-change = Claude Code está escribiendo un cambio…
editor-claude-code-proposes-a-new-file = Claude Code propone un archivo nuevo
editor-claude-code-proposes-a-change = Claude Code propone un cambio
editor-more = { $waiting } más
editor-reject = Rechazar
editor-accept = Aceptar
editor-claudes-changes-all-reviewed = Cambios de Claude, todos revisados
editor-claude-changed-this-file-1-change-to-review = Claude cambió este archivo: 1 cambio por revisar
editor-claude-changed-this-file-changes-to-review = Claude cambió este archivo: { $n } cambios por revisar
editor-next = ⌃⌥⇧↓ siguiente
editor-revert-file = Revertir archivo
editor-accept-file = Aceptar archivo
editor-hide-the-inline-changes = Ocultar los cambios en línea
editor-changed-by-at = Cambiado por { $short } a las { $t }
editor-changed-by = Cambiado por { $short }
editor-lines-taken-out-here = Líneas quitadas aquí
editor-line = Línea { $start }
editor-lines-2 = Líneas { $start }–{ $end }
editor-reviewed = Revisado
editor-revert = Revertir
editor-ask-to-fix = Pedir a { $agent } que lo corrija
editor-context-actions = Acciones contextuales
editor-looking-for-fixes = Buscando correcciones…
editor-the-symbol = el símbolo
editor-rename-to-enter-renames-everywhere = Cambiar nombre a (Intro cambia en todas partes)
editor-go-to-declaration-needs-the-java-engine-for = Ir a la declaración necesita el motor de Java para este archivo.
editor-no-declaration-found-here = No se encontró ninguna declaración aquí.
editor-usages-of = Usos de { $word }
editor-no-usages-of-found = No se encontraron usos de { $word }.
editor-implementations-of = Implementaciones de { $word }
editor-no-implementations-of-found = No se encontraron implementaciones de { $word }.
editor-this-needs-the-java-engine-for-this-file = Esto necesita el motor de Java para este archivo.
editor-found = { $count } encontrados
editor-group-in-the-local-repository = grupo en el repositorio local
editor-artifact-in-the-local-repository = artefacto en el repositorio local
editor-version-in-the-local-repository = versión en el repositorio local
editor-ask-to-fix-2 = Pedir a { $short } que lo corrija
editor-more-actions = Más acciones…
editor-the-file-changed-meanwhile-try-fix-all-again = El archivo cambió entretanto: vuelve a probar Corregir todo
editor-none-of-these-problems-could-be-fixed-this = Ninguno de estos problemas se pudo corregir así
editor-fixed-of-problems-the-rest-need-a-look = Se corrigieron { $n } de { $count } problemas; el resto necesita revisión
editor-no-problem-at-the-caret = Ningún problema en el cursor
editor-no-quick-fix-for-this-problem-for-more = Sin corrección rápida para este problema: { $more } para más acciones
editor-no-actions-here = No hay acciones aquí
editor-recording-but-the-recorded-steps-were-edited-here = Grabando, pero los pasos grabados se editaron aquí: el resto se queda en la ventana del grabador.
editor-no-steps-yet = aún sin pasos
editor-1-step = 1 paso
editor-steps = { $n } pasos
editor-recording-into-this-file-act-in-the-browser = Grabando en este archivo: { $n }. Actúa en el navegador y luego detén o ciérralo.
editor-picking-locators-click-an-element-in-the-browser = Eligiendo localizadores: haz clic en un elemento del navegador para poner su localizador en el cursor.
editor-picking-locators-picked-click-another-or-stop = Eligiendo localizadores: { $picked } elegidos. Haz clic en otro o detén.
editor-cannot-extract-a-method-here-select-whole-statements = No se puede extraer un método aquí: selecciona sentencias completas o una expresión
editor-cannot-extract-a-variable-here-select-an-expression = No se puede extraer una variable aquí: selecciona una expresión
editor-cannot-extract-a-constant-here-select-an-expression = No se puede extraer una constante aquí: selecciona una expresión de constantes, como un literal
editor-cannot-extract-a-field-here-select-an-expression = No se puede extraer un campo aquí: selecciona una expresión
editor-nothing-to-inline-here-put-the-caret-on = Nada que integrar aquí: coloca el cursor en una variable local, un método o una constante
editor-a-library-class-is-read-only = Una clase de biblioteca es de solo lectura
editor-replace-all-occurrences = Reemplazar todas las apariciones
editor-replace-this-occurrence-only = Reemplazar solo esta aparición
editor-breakpoint-on-line = Punto de interrupción en la línea { $line }
editor-filter-by-class-member-or-file = Filtrar por clase, miembro o archivo
editor-show-in-endpoints = Mostrar en Endpoints
editor-nothing-matches = Sin resultados.
editor-of = { $matching } de { $total }
editor-bean = Bean '{ $b }'
editor-bean-is-injected-into = Bean '{ $b }' inyectado en
editor-takes = '{ $name }' toma { $value }
editor-no-bean-for = Ningún bean para '{ $name }'
editor-press-for-quick-fixes = { $why }. Pulsa ⌘. para correcciones rápidas.
editor-could-take-any-of = '{ $name }' podría tomar cualquiera de
editor-takes-2 = '{ $name }' toma
editor-takes-the-bean = '{ $name }' toma el bean
editor-fix-all-in-file = Corregir todo «{ $title }» en el archivo
editor-supertypes = Supertipos
editor-subtypes = Subtipos
editor-callers = Llamadores
editor-callees = Llamados
editor-put-the-caret-on-a-type-or-a = Coloca el cursor en un tipo o un método y luego ⌃H o ⌃⌥H.
editor-put-the-caret-on-a-class-or-an = Coloca el cursor en una clase o interfaz para ver su jerarquía.
editor-put-the-caret-on-a-method-for-its = Coloca el cursor en un método para ver sus llamadas.
editor-type-hierarchy-of = Jerarquía de tipos de
editor-calls-of = Llamadas de
editor-reading-the-hierarchy = Leyendo la jerarquía
editor-the-java-engine-is-looking-it-up = El motor de Java lo está buscando.
editor-no-hierarchy-yet = Aún no hay jerarquía
editor-no-hierarchy = Sin jerarquía
editor-needs-jdk = Necesita JDK { $v }
editor-importing-gradle-model = Importando el modelo de Gradle
editor-restarting = Reiniciando
editor-syncing-the-build-model = Sincronizando el modelo de compilación
editor-generating-sources = Generando fuentes
editor-importing = Importando
editor-of-modules-ready = { $ready } de { $n } módulos listos
editor-java = Java: { $summary }
editor-java-2 = Java: { $what }
editor-java-3 = Java: { $text }
editor-the-java-engine-is-not-built-run-snout = El motor de Java no está compilado (ejecuta snout.sh)
editor-the-java-engine-did-not-start = El motor de Java no se inició: { $e }
editor-the-java-engine-keeps-stopping = El motor de Java se detiene una y otra vez
editor-references-to = Referencias a { $name }
editor-find-references-f12-lists-them-here = Buscar referencias (⇧F12) las muestra aquí
editor-results = { $count ->
    [one] { $count } resultado
   *[other] { $count } resultados
    }
editor-title-results = { $title }: { $results }
editor-no-usages-found = No se encontraron usos
editor-no-usages-yet = Aún no hay usos
editor-find-usages-on-a-symbol-lists-every-place = Buscar usos en un símbolo muestra cada lugar donde se usa.
editor-find-usages-at-the-caret = Buscar usos en el cursor
editor-filter-problems = Filtrar problemas
editor-show = Mostrar { $what }
editor-expand-all = Expandir todo
editor-collapse-all = Contraer todo
editor-no-problems = Sin problemas
editor-nothing-to-fix-in-the-files-the-java = Nada que corregir en los archivos que comprobó el motor de Java.
editor-no-problems-match = Ningún problema coincide
editor-clear-the-filter-or-turn-on-more-of = Borra el filtro o activa más errores, advertencias e información arriba.
editor-filter-by-name = Filtrar por nombre
editor-sorted-by-name-click-for-file-order = Ordenado por nombre (clic para el orden del archivo)
editor-sort-by-name = Ordenar por nombre
editor-no-structure = Sin estructura
editor-reading-the-structure = Leyendo la estructura
editor-try-fewer-letters-of-the-name = Prueba con menos letras del nombre.
editor-nothing-declared = Nada declarado
editor-the-file-declares-no-class-field-or-method = El archivo no declara ninguna clase, campo o método.
editor-package-private = privado del paquete
editor-finding-node = Buscando Node
editor-needs-node = Necesita Node { $what }
editor-installing-the-language-server = Instalando el servidor de lenguaje
editor-typescript = TypeScript: { $what }
editor-or-newer = { $m } o más reciente
editor-the-typescript-language-server-did-not-start = El servidor de lenguaje de TypeScript no se inició: { $e }
editor-the-typescript-language-server-failed = El servidor de lenguaje de TypeScript falló: { $e }
editor-the-typescript-language-server-keeps-stopping = El servidor de lenguaje de TypeScript se detiene una y otra vez
editor-open-a-java-file = Abre un archivo Java para ver su estructura.
editor-java-engine-reading = El motor de Java está leyendo el archivo.
editor-problems-title = Problemas
editor-downloading-java-for-the-engine = Descargando Java { $ENGINE_JAVA } para el motor
editor-copying-the-java-engine = Copiando el motor de Java
editor-java-engine-install-failed = No se pudo instalar el motor de Java: { $why }
editor-ts-install-failed = No se pudo instalar el servidor de lenguaje de TypeScript: { $why }

## blame

blame-month-1 = enero
blame-month-2 = febrero
blame-month-3 = marzo
blame-month-4 = abril
blame-month-5 = mayo
blame-month-6 = junio
blame-month-7 = julio
blame-month-8 = agosto
blame-month-9 = septiembre
blame-month-10 = octubre
blame-month-11 = noviembre
blame-month-12 = diciembre
blame-long-date = { $day } de { $month } de { $year } a las { $time }

## agents

agents-turn = Turno { $id }
agents-files = { $files } archivos
agents-1-file = 1 archivo
agents-revert-what-this-prompt-changed = Revertir lo que cambió este prompt
agents-accept-what-this-prompt-changed = Aceptar lo que cambió este prompt
agents-editing = Editando
agents-waiting-for-your-review = Esperando tu revisión
agents-claude-asks-to-make-this-change = Claude pide hacer este cambio
agents-waiting-behind-another-change-to-this-file = Esperando tras otro cambio en este archivo
agents-1-file-changed = 1 archivo cambiado
agents-files-changed = { $files } archivos cambiados
agents-revert-all = Revertir todo
agents-put-back-every-file-as-it-was-before = Devolver cada archivo a como estaba antes de que Claude lo cambiara
agents-accept-all = Aceptar todo
agents-keep-every-change-the-files-leave-this-list = Conservar todos los cambios; los archivos salen de esta lista
agents-seen-on-disk-while-claude-code-was-connected = Visto en el disco mientras Claude Code estaba conectado; sus hooks no lo informaron
agents-revert-the-file = Revertir el archivo
agents-accept-the-file = Aceptar el archivo
agents-lines-removed = { $count } líneas eliminadas
agents-revert-this-change = Revertir este cambio
agents-accept-this-change = Aceptar este cambio
agents-no-changes-from-agents-yet = Aún no hay cambios de agentes
agents-the-files-an-agent-changes-show-here-as = Los archivos que cambia un agente aparecen aquí mientras trabaja, cada cambio para aceptar o revertir.
agents-open-the-agent-console = Abrir la consola del agente
agents-a-new-file-the-agent-created-it-reverting = Un archivo nuevo: lo creó el agente; revertir lo elimina
agents-sent-line-to = { $name } línea { $a } enviada a { $agent }
agents-sent-lines-to = { $name } líneas { $a }–{ $b } enviadas a { $agent }
agents-sent-to = { $name } enviado a { $agent }
agents-asked-to-fix-line = Se pidió a { $short } corregir { $name } línea { $a }
agents-asked-to-fix-lines = Se pidió a { $short } corregir { $name } líneas { $a }–{ $b }
agents-snoutcode-cannot-reach-the-agent-in-this-window = SnoutCode no puede comunicarse con el agente en esta ventana. Cierra el proyecto y vuelve a abrirlo.
agents-asked-for-the-commit-message = Se pidió a { $name } el mensaje de commit
agents-claude-is-waiting-for-your-review = Claude espera tu revisión
agents-is-editing = { $short } está editando
agents-waiting = Esperando
agents-done = Hecho
agents-no-calls-yet = Aún no hay llamadas
agents-what-an-agent-in-the-console-asks-of = Lo que un agente de la consola le pide a SnoutCode aparece aquí.
agents-open-a-file-to-insert-into = Abre un archivo en el que insertar
agents-insert = Insertar
agents-run-in-terminal = Ejecutar en el terminal
agents-nothing-shown-yet = Aún no se muestra nada
agents-what-an-agent-in-the-console-chooses-to = Lo que un agente de la consola decide mostrarte aparece aquí, con botones en su código.
agents-from = { $title }, de { $agent }
agents-from-2 = De { $agent }
agents-all = Los agentes
agents-two = { $a } y { $b }
agents-n = { $n } agentes
agents-changed-files = { $count ->
    [one] { $who }: 1 archivo modificado
   *[other] { $who }: { $count } archivos modificados
    }
agents-seen = visto
agents-new = nuevo
project-shared = compartido

## keys

keys-page-up = Re Pág
keys-page-down = Av Pág
keys-backspace = Retroceso

## json

json-true-or-false = true o false
json-a-whole-number = un número entero
json-a-number = un número
json-a-string = una cadena
json-a-list = una lista
json-an-object = un objeto
json-from-to = { $only } de { $min } a { $max }
json-any-value = cualquier valor
json-or = { $join } o { $last }
json-default = {"\u000A"}{"\u000A"}Predeterminado: `{ $shown }`
json-unexpected-text-after-the-settings = Texto inesperado tras la configuración
json-the-settings = La configuración
json-takes = { $what } admite { $s }
json-is-at-least = { $what } es como mínimo { $min }
json-is-at-most = { $what } es como máximo { $max }
json-unknown-setting-snoutcode-ignores-it = Ajuste desconocido `{ $key }`: SnoutCode lo ignora
json-needs = { $what } necesita `{ $req }`
json-a-value-is-missing-here = Falta un valor aquí
json-a-key-in-double-quotes-is-expected-here = Aquí se espera una clave entre comillas dobles
json-a-comma-or-a-closing-brace-is-expected = Aquí se espera una coma o una llave de cierre
json-a-comma-or-a-closing-bracket-is-expected = Aquí se espera una coma o un corchete de cierre
json-this = esto
json-is-not-a-json-value-strings-take-double = { $shown } no es un valor JSON (las cadenas llevan comillas dobles)
json-this-string-cannot-be-read = No se puede leer esta cadena: { $e }
json-this-string-has-no-closing-quote = Esta cadena no tiene comilla de cierre
json-colon-expected = Se esperan dos puntos tras «{ $key }»

## core

core-head-to-worktree = HEAD ↔ Árbol de trabajo
core-index-to-worktree = Índice ↔ Árbol de trabajo
core-head-to-index = HEAD ↔ Índice
core-index = Índice
core-scope-message-only = solo el mensaje
core-scope-nothing = nada para hacer commit
core-what-staged = los cambios preparados
core-what-changed-file = el archivo cambiado
core-what-all-changed = los { $n } archivos cambiados
core-what-selected-file = el archivo seleccionado
core-what-selected = los { $n } archivos seleccionados
core-what-message = el nuevo mensaje del último commit
core-what-nothing = nada
core-state-staged = { $state } (preparado)
core-state-partly-staged = { $state } (preparado en parte)
core-state-unchanged = Sin cambios
core-branch-dash = Un nombre de rama no puede empezar por un guion.
core-branch-dots = Un nombre de rama no puede contener «..» ni «@{"{"}».
core-branch-chars = Un nombre de rama no puede contener ~ ^ : ? * [ ni \.
core-branch-ends = Un nombre de rama no puede empezar por «/» ni terminar en «/», «.» o «.lock».
core-group-taken = Ya hay un grupo con este nombre
core-years-ago = { $count ->
    [one] hace { $count } año
   *[other] hace { $count } años
    }
core-engine-built-in = Integrado
core-ghostty-unavailable = Esta versión se compiló sin Ghostty. Las versiones publicadas lo incluyen.
core-gap-unchanged = { $count ->
    [one] ⋯ { $count } línea sin cambios
   *[other] ⋯ { $count } líneas sin cambios
    }
core-update-checking = Buscando actualizaciones…
core-update-found = Actualización encontrada (v{ $version }), descargando…
core-update-preparing-now = Preparando la actualización…
core-update-downloading-now = Descargando la actualización…
core-update-ready = Actualización lista: v{ $version }
core-update-from-source = Esta versión se compiló desde el código fuente y no se actualiza sola. Compílela de nuevo desde el código más reciente para actualizarla.
core-update-up-to-date = Está al día (v{ $version })
core-update-available = SnoutCode v{ $version } está disponible.
core-update-cannot-replace = SnoutCode v{ $version } está disponible, pero no puede instalarse desde donde se ejecuta. Muévalo a sus aplicaciones o descárguelo.
core-update-did-not-land = SnoutCode v{ $version } no se instaló. Descárguelo para instalarlo a mano.
core-update-preparing = Preparando la actualización
core-update-downloading = Descargando la actualización
core-update-offline = No se pudieron buscar actualizaciones: este equipo está sin conexión. Lo intentará más tarde.
core-update-feed-down = No se pudieron buscar actualizaciones: GitHub no responde. Su instalación está bien y lo intentará más tarde.
core-update-rate-limited = No se pudieron buscar actualizaciones: GitHub está limitando las solicitudes de esta red. Lo intentará más tarde.
core-update-no-release = No se pudieron buscar actualizaciones: no se encontró ninguna versión publicada.
core-update-signature = Se descargó una actualización y se rechazó: no coincide con la firma que debería tener. No se instaló nada.
core-update-disk = No se pudo guardar la actualización: no hay suficiente espacio en disco.
core-update-unknown = No se pudieron buscar actualizaciones.
core-scope-staged = { $n ->
    [one] { $n } preparado
   *[other] { $n } preparados
    }
core-scope-all-changed = { $n ->
    [one] { $n } cambiado
   *[other] los { $n } cambiados
    }
core-scope-selected = { $n ->
    [one] { $n } seleccionado
   *[other] { $n } seleccionados
    }

## icons

icons-plain-text = Texto sin formato

## menu

menu-menu = Menú

## misc

newproject-location = Ubicación
newproject-offline-fallback = { $why } Estas son las dependencias que SnoutCode conoce; el proyecto se crea aquí y Gradle descargará Spring Boot cuando haya red.
structure-modules-row = Módulos
structure-jdk-hint = Gradle y el motor Java se ejecutan con él. Se guarda en .snout/settings.json; el motor lo usará la próxima vez que arranque.
structure-locked-maven-hint = Los valores con candado son de Maven: Editar en el archivo de compilación abre la línea del pom.xml que los define. Al guardar, la barra sobre el editor ofrece recargar.
structure-locked-gradle-hint = Los valores con candado son de la compilación: Editar en el archivo de compilación abre la línea que los define. Al guardar, la barra sobre el editor ofrece sincronizar.
console-installer-not-on-path = El instalador terminó, pero { $agent } no está en el PATH que ve SnoutCode. Indique su ruta en Configuración, Agentes, o reinicie SnoutCode.
console-no-project-open = No hay ningún proyecto abierto: { $agent } empieza en su carpeta personal, donde puede crear uno. Abra el proyecto cuando exista para seguir sus cambios.
dialogs-group-example = Trabajo
dialogs-checkout-placeholder = Filtrar ramas y etiquetas, o escribir un commit (SHA, HEAD~2, …)
gutter-run-label = Ejecutar ‘{ $label }’
gutter-debug-label = Depurar ‘{ $label }’
editor-settings-source = Configuración de SnoutCode
terminal-not-installed = { $name } no está instalado
terminal-is-mac-app = { $name } es una aplicación de Mac
terminal-not-mac-app = { $name } no es una aplicación de Mac
terminal-not-windows-app = { $name } no es una aplicación de Windows
terminal-default-linux = Terminal predeterminado
model-could-not-open-in = No se pudo abrir el proyecto en { $connection }
model-no-repositories-found = No se encontraron repositorios
env-environments = Entornos
env-base-url-hint = Las ejecuciones reciben la URL como BASE_URL, que lee el baseURL de la configuración. Primero van las variables del archivo y luego las de arriba. Guarde las contraseñas en el archivo .env, que el repositorio debe ignorar.
pwsetup-runner-missing = el ejecutor de pruebas de Playwright no está instalado
remote-password-title = Contraseña
run-needs-jdk-note = Esta compilación necesita el JDK { $v }, que no está instalado. Descárguelo desde la barra sobre el editor o elija un JDK en Configuración.
debug-js-download-failed = No se pudo descargar el depurador de JavaScript: { $e }
debug-js-did-not-start = El depurador de JavaScript no arrancó: { $e }
run-ts-needs-tsx = Ejecutar un archivo TypeScript requiere el paquete tsx en el proyecto ({ $manager } add -D tsx) o Node 22.6 o posterior (este proyecto usa Node { $node }).
