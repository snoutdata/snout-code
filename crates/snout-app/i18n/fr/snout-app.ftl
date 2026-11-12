# SnoutCode's interface in French. English (i18n/en) is the source: same ids, same
# arguments. Say it the way a developer in this language would, not word for word.

## Languages

language-system = Système ({ $language })

## Light or dark

theme-system = Système
theme-light = Clair
theme-dark = Sombre

## Settings

settings-nothing-matches = Aucun résultat
settings-group-build-tools = Outils de build
settings-group-frameworks = Frameworks
settings-group-tools = Outils
settings-page-appearance = Apparence
settings-page-keymap = Raccourcis clavier
settings-page-editor = Éditeur
settings-page-version-control = Gestion de versions
settings-page-terminal = Terminal
settings-page-agents = Agents
settings-blurb-appearance = La langue, les couleurs, le mode clair ou sombre et les polices de la fenêtre et de l’éditeur.
settings-blurb-keymap = Les touches de chaque commande. Ajoutez-en, retirez-en ou revenez aux valeurs par défaut.
settings-blurb-editor = Comment les fichiers s’indentent, se replient et s’affichent pendant la frappe.
settings-blurb-version-control = Comment SnoutCode tient le Git de vos projets à jour.
settings-blurb-gradle = Combien de temps les daemons Gradle de SnoutCode restent sans build.
settings-blurb-spring = Les profils sous lesquels la fenêtre Spring évalue les beans.
settings-blurb-terminal = L’apparence des terminaux, ce qu’ils exécutent et l’application de terminal pour ouvrir les dossiers.
settings-blurb-agents = Les agents de code que SnoutCode suit et celui que lance sa console.

settings-section-language = Langue
settings-language = Langue de l’interface
settings-section-theme = Thème
settings-colors = Couleurs
settings-light-or-dark = Clair ou sombre
settings-dark-only = { $name } n’a pas de version claire : il reste sombre, quel que soit ce réglage.
settings-section-interface = Interface
settings-font = Police
settings-size = Taille
settings-section-editor = Éditeur
settings-section-quitting = Fermeture
settings-confirm-quit = Confirmer avant de quitter
settings-confirm-quit-note = SnoutCode demande toujours confirmation si quitter ferait perdre des modifications ou arrêterait quelque chose en cours.
settings-section-updates = Mises à jour
settings-update-check = Rechercher les mises à jour automatiquement
settings-update-check-note = Une nouvelle version se télécharge en arrière-plan et s’installe au redémarrage ou à la fermeture. Le menu Aide permet de rechercher les mises à jour à tout moment.

## The menu bar

menu-run = Exécuter
menu-snoutcode = SnoutCode
menu-services = Services
menu-file = Fichier
menu-open-recent = Ouvrir un projet récent
menu-edit = Édition
menu-view = Affichage
menu-git = Git
menu-tool-window-actions = Actions des fenêtres d’outils
menu-appearance = Apparence
menu-code = Code
menu-navigate = Naviguer
menu-refactor = Refactoriser
menu-build = Compiler
menu-tools = Outils
menu-tool-settings = Paramètres des outils
menu-window = Fenêtre
menu-help = Aide

## Commands: the menu items and the command palette

cmd-new-project = Nouveau projet…
cmd-open-project = Ouvrir un projet…
cmd-close-project = Fermer le projet
cmd-connect-to-ssh-host-or-wsl = Se connecter à un hôte SSH ou WSL…
cmd-connect-to-ssh-host = Se connecter à un hôte SSH…
cmd-close-remote-connection = Fermer la connexion distante
cmd-clear-recent-projects = Effacer les projets récents
cmd-search-everywhere = Rechercher partout…
cmd-settings = Paramètres…
cmd-open-settings-json = Ouvrir les paramètres (JSON)
cmd-open-project-settings-json = Ouvrir les paramètres du projet (JSON)
cmd-go-to-file = Aller au fichier…
cmd-recent-files = Fichiers récents…
cmd-new-file = Nouveau fichier…
cmd-go-to-class = Aller à la classe…
cmd-go-to-line = Aller à la ligne…
cmd-find-in-files = Rechercher dans les fichiers…
cmd-replace-in-files = Remplacer dans les fichiers…
cmd-undo-replace-in-files = Annuler le remplacement dans les fichiers
cmd-find-in-file = Rechercher dans le fichier
cmd-replace-in-file = Remplacer dans le fichier
cmd-save = Enregistrer
cmd-close-tab = Fermer l’onglet
cmd-split-editor-right = Scinder l’éditeur à droite
cmd-toggle-comment = Commenter/décommenter
cmd-duplicate-line = Dupliquer la ligne
cmd-move-line-up = Déplacer la ligne vers le haut
cmd-move-line-down = Déplacer la ligne vers le bas
cmd-toggle-soft-wrap = Activer/désactiver le retour à la ligne
cmd-toggle-parameter-name-hints = Activer/désactiver les noms de paramètres
cmd-go-to-definition = Aller à la définition
cmd-go-to-declaration-or-usages = Aller à la déclaration ou aux utilisations
cmd-go-to-implementations = Aller aux implémentations
cmd-find-references = Rechercher les références
cmd-rename-symbol = Renommer le symbole…
cmd-extract-method = Extraire la méthode
cmd-extract-variable = Extraire la variable
cmd-extract-constant = Extraire la constante
cmd-extract-field = Extraire le champ
cmd-inline = Incorporer
cmd-show-context-actions = Afficher les actions contextuelles…
cmd-apply-top-fix = Appliquer le premier correctif
cmd-format-document = Mettre en forme le document
cmd-format-selection = Mettre en forme la sélection
cmd-organize-imports = Organiser les imports
cmd-show-parameter-hints = Afficher l’aide des paramètres
cmd-trigger-completion = Déclencher la complétion
cmd-go-to-symbol-in-workspace = Aller au symbole dans l’espace de travail…
cmd-go-to-symbol-in-file = Aller au symbole dans le fichier…
cmd-file-structure = Structure du fichier…
cmd-structure = Structure
cmd-type-hierarchy = Hiérarchie des types
cmd-call-hierarchy = Hiérarchie des appels
cmd-hierarchy = Hiérarchie
cmd-show-problems = Afficher les problèmes
cmd-run = Exécuter
cmd-debug = Déboguer
cmd-run-context-configuration = Exécuter la configuration du contexte
cmd-debug-context-configuration = Déboguer la configuration du contexte
cmd-stop = Arrêter
cmd-rerun = Réexécuter
cmd-rerun-failed-tests = Réexécuter les tests en échec
cmd-build-project = Compiler le projet
cmd-rebuild-project = Recompiler le projet
cmd-run-gradle-task = Exécuter une tâche Gradle…
cmd-run-maven-goal = Exécuter un goal Maven…
cmd-show-the-build-window = Afficher la fenêtre de compilation
cmd-show-the-gradle-window = Afficher la fenêtre Gradle
cmd-show-the-maven-window = Afficher la fenêtre Maven
cmd-terminal = Terminal
cmd-output = Sortie
cmd-settings-keymap = Paramètres : raccourcis clavier
cmd-settings-terminal = Paramètres : terminal
cmd-settings-coding-agents = Paramètres : agents de code
cmd-settings-gradle-and-maven = Paramètres : Gradle et Maven
cmd-edit-configurations = Modifier les configurations…
cmd-edit-environments = Modifier les environnements…
cmd-show-the-tests-window = Afficher la fenêtre des tests
cmd-record-at-cursor = Enregistrer au curseur
cmd-pick-locator = Choisir un localisateur
cmd-stop-recording = Arrêter l’enregistrement
cmd-open-ui-mode = Ouvrir le mode UI
cmd-toggle-breakpoint = Poser/retirer un point d’arrêt
cmd-toggle-breakpoint-enabled = Activer/désactiver le point d’arrêt
cmd-edit-breakpoint-condition = Modifier la condition du point d’arrêt…
cmd-view-breakpoints = Afficher les points d’arrêt…
cmd-evaluate-expression = Évaluer l’expression…
cmd-run-to-cursor = Exécuter jusqu’au curseur
cmd-step-over = Pas à pas principal
cmd-step-into = Pas à pas détaillé
cmd-step-out = Pas à pas sortant
cmd-pause-program = Suspendre le programme
cmd-resume-program = Reprendre le programme
cmd-clear-output = Effacer la sortie
cmd-sync-gradle-and-maven-projects = Synchroniser les projets Gradle et Maven
cmd-project-structure = Paramètres du projet…
cmd-refresh-gradle-tasks = Actualiser les tâches Gradle
cmd-collapse-all-gradle-tasks = Replier toutes les tâches Gradle
cmd-reload-maven-project = Recharger le projet Maven
cmd-read-the-maven-goals-again = Relire les goals Maven
cmd-collapse-all-maven-goals = Replier tous les goals Maven
cmd-spring-beans-and-endpoints = Beans et endpoints Spring
cmd-read-the-spring-model-again = Relire le modèle Spring
cmd-go-to-spring-bean-or-usages = Aller au bean Spring ou aux utilisations…
cmd-spring-active-profiles = Profils Spring actifs…
cmd-collapse-the-spring-window = Replier la fenêtre Spring
cmd-reload-all-from-disk = Tout recharger depuis le disque
cmd-select-opened-file = Sélectionner le fichier ouvert
cmd-collapse-the-project-tree = Replier l’arborescence du projet
cmd-hide-the-side-panel = Masquer le panneau latéral
cmd-sort-structure-by-name = Trier la structure par nom
cmd-show-processes = Afficher les processus
cmd-show-the-agent-log = Afficher le journal de l’agent
cmd-stop-idle-processes = Arrêter les processus inactifs
cmd-restart-the-java-engine = Redémarrer le moteur Java
cmd-send-selection-to-claude = Envoyer la sélection à Claude
cmd-accept-claudes-change = Accepter la modification de Claude
cmd-reject-claudes-change = Rejeter la modification de Claude
cmd-next-claude-change = Modification suivante de Claude
cmd-previous-claude-change = Modification précédente de Claude
cmd-accept-claude-change-at-caret = Accepter la modification de Claude au curseur
cmd-revert-claude-change-at-caret = Annuler la modification de Claude au curseur
cmd-accept-all-claude-changes = Accepter toutes les modifications de Claude
cmd-revert-all-claude-changes = Annuler toutes les modifications de Claude
cmd-show-agent-changes = Afficher les modifications des agents
cmd-annotate-with-agents = Annoter avec les agents
cmd-annotate-with-git-blame = Annoter avec Git Blame
cmd-show-history = Afficher l’historique
cmd-show-history-of-file = Afficher l’historique du fichier
cmd-undo = Annuler
cmd-redo = Rétablir
cmd-zoom-in = Zoom avant
cmd-zoom-out = Zoom arrière
cmd-actual-size = Taille réelle
cmd-show-project = Afficher le projet
cmd-show-git = Afficher Git
cmd-toggle-agent-console = Afficher/masquer la console de l’agent
cmd-find-repositories-in-folder = Rechercher des dépôts dans un dossier…
cmd-clone-from-github = Cloner depuis GitHub…
cmd-new-group = Nouveau groupe…
cmd-refresh = Actualiser
cmd-pull = Pull
cmd-fetch = Fetch
cmd-push = Push
cmd-commit = Commit
cmd-draft-commit-message = Rédiger le message de commit
cmd-stage = Indexer
cmd-unstage = Désindexer
cmd-discard-changes = Annuler les modifications…
cmd-check-out = Basculer vers…
cmd-new-branch = Nouvelle branche…
cmd-side-by-side-diff = Diff côte à côte
cmd-unified-diff = Diff unifié
cmd-compact-diff = Diff compact
cmd-theme-match-the-system = Thème : suivre le système
cmd-theme-dark = Thème : sombre
cmd-theme-light = Thème : clair
cmd-open-on-github = Ouvrir sur GitHub
cmd-open-in-external-terminal = Ouvrir dans un terminal externe
cmd-about-snoutcode = À propos de SnoutCode
cmd-check-for-updates = Rechercher les mises à jour…
cmd-release-notes = Notes de version
cmd-quit-snoutcode = Quitter SnoutCode
cmd-clear-recent = Effacer les récents
cmd-exit = Quitter
cmd-quit = Quitter
cmd-zoom = Zoom
cmd-maximize = Agrandir
cmd-command-palette = Palette de commandes…
cmd-cut = Couper
cmd-copy = Copier
cmd-paste = Coller
cmd-select-all = Tout sélectionner
cmd-find = Rechercher
cmd-replace = Remplacer
cmd-project = Projet
cmd-git = Git
cmd-agent-console = Console de l’agent
cmd-soft-wrap = Retour à la ligne automatique
cmd-parameter-name-hints = Noms de paramètres
cmd-processes = Processus
cmd-agent-log = Journal de l’agent
cmd-build-window = Fenêtre de compilation
cmd-gradle = Gradle
cmd-maven = Maven
cmd-move-to-trash = Placer dans la corbeille…
cmd-agent-changes = Modifications des agents
cmd-keymap = Raccourcis clavier…
cmd-terminal-more = Terminal…
cmd-coding-agents = Agents de code…
cmd-gradle-and-maven = Gradle et Maven…
cmd-minimize = Réduire
cmd-snoutcode-on-github = SnoutCode sur GitHub
cmd-report-an-issue = Signaler un problème
cmd-snoutdata-website = Site web de SnoutData
cmd-open-recent-project = Ouvrir un projet récent : { $name }

## The platform's file manager

machine-mac = ce Mac
machine-windows = ce PC
machine-linux = cet ordinateur
reveal-mac = Afficher dans le Finder
reveal-windows = Afficher dans l’Explorateur
reveal-linux = Afficher dans Fichiers

## settings

settings-search-settings = Rechercher un paramètre
settings-found-on-path-and-the-usual-places = Détecté dans le PATH et aux emplacements habituels
settings-its-programs-name = Le nom de son programme
settings-your-login-shell-shell = Votre shell de connexion ($SHELL)
settings-no-arguments = Aucun argument
settings-name-value-then-return = NOM=valeur, puis Entrée
settings-search-commands-or-shortcuts = Rechercher des commandes ou des raccourcis
settings-overridden-by-this-project = Remplacé par ce projet
settings-set-for-this-project = Défini pour ce projet
settings-reset-to-your-setting = Revenir à votre réglage
settings-your-setting = Votre réglage
settings-user = Utilisateur
settings-your-settings-json = Votre settings.json
settings-for-only-kept-in-in-the-project-what = Pour { $root } uniquement, enregistré dans { $file } dans le projet. Ce qui y est défini remplace vos paramètres.
settings-your-settings-for-every-project-a-project-can = Vos paramètres, pour tous les projets. Un projet peut en définir certains pour lui-même, dans Paramètres du projet (⌘;).
settings-in-every-build = Dans toutes les versions
settings-reflow-kitty-keys-links = Redistribution, touches kitty, liens
settings-unavailable = Indisponible
settings-the-same-in-every-project = Identique dans tous les projets
settings-settings-are-yours-a-project-cannot-set-them = Les paramètres « { $title } » sont les vôtres : un projet ne peut pas les définir lui-même.
settings-show-your-settings = Afficher vos paramètres
settings-indentation = Indentation
settings-tab-size = Taille de tabulation
settings-for-a-file-whose-own-indentation-cannot-be = Pour un fichier dont l’indentation ne peut pas être déterminée.
settings-display = Affichage
settings-soft-wrap-long-lines = Replier les lignes longues à l’affichage
settings-show-whitespace = Afficher les espaces
settings-show-parameter-names-of-literal-arguments-java = Afficher le nom des paramètres des arguments littéraux (Java)
settings-hover = Survol
settings-show-hover = Afficher au survol
settings-delay = Délai
settings-navigation = Navigation
settings-click-with = Cliquer avec
settings-preview = Aperçu
settings-fetch-every = Fetch toutes les
settings-the-projects-are-fetched-in-the-background-to = Un fetch des projets se fait en arrière-plan, pour compter ce qu’il y a à récupérer.
settings-annotations = Annotations
settings-show-who-wrote-each-class-and-method-ada = Afficher l’auteur de chaque classe et méthode (« Ada Lovelace +2 »)
settings-show-the-current-lines-author-and-commit-after = Afficher l’auteur et le commit de la ligne courante à sa suite
settings-good-to-know = Bon à savoir
settings-a-background-fetch-leaves-your-branches-as-they = Un fetch en arrière-plan ne touche pas à vos branches : il compte seulement les commits à récupérer.
settings-click-the-branch-in-the-title-bar-for = Cliquez sur la branche dans la barre de titre pour Nouvelle branche, Fetch, Pull et Push, et pour basculer vers n’importe quelle branche.
settings-2-shows-the-git-view-in-the-message = ⌘2 affiche la vue Git. Dans le champ du message, ⌘↩ crée le commit.
settings-each-builds-own-your-settings = Celui de chaque build (selon son niveau Java)
settings-every-build = Tous les builds
settings-jdk-per-project = JDK par projet
settings-no-gradle-build-is-open = Aucun build Gradle n’est ouvert.
settings-automatic-the-builds-java-level = Automatique (le niveau Java du build)
settings-the-java-server-restarts-on-the-chosen-jdk = Le serveur Java utilisera le JDK choisi à son prochain démarrage.
settings-gradle-daemons = Daemons Gradle
settings-stop-idle-daemons-after = Arrêter si inactifs
settings-double-click-a-task-in-the-gradle-tool = Double-cliquez sur une tâche dans la fenêtre Gradle pour l’exécuter. Sa sortie s’affiche dans la fenêtre de compilation.
settings-the-processes-window-lists-the-gradle-daemons-running = La fenêtre Processus liste les daemons Gradle en cours et arrête les inactifs à la demande.
settings-project-structure-downloads-jdks-and-sets-the-projects = Paramètres du projet (⌘;) télécharge des JDK et définit celui du projet.
settings-the-leaf-on-the-right-stripe-opens-the = La feuille de la barre de droite ouvre la fenêtre Spring : les beans et les endpoints trouvés par SnoutCode.
settings-spring-active-profiles-p-changes-the-profiles-from = Profils Spring actifs (⌃⌥P) change les profils depuis n’importe où.
settings-a-bean-the-active-profiles-leave-out-is = Un bean exclu par les profils actifs est grisé, et aucune injection ne le prend.
settings-active-profiles = Profils actifs
settings-open-a-project-to-choose-its-profiles = Ouvrez un projet pour choisir ses profils.
settings-any-no-profile-chosen = Tous (aucun profil choisi)
settings-those-the-application-files-make-active = Ceux activés par les fichiers de l’application
settings-these-profiles = Ces profils
settings-evaluate-beans-under = Évaluer les beans avec
settings-profiles = Profils
settings-choose = Choisir
settings-looking-for-it = Recherche en cours…
settings-that-file-is-not-there = Ce fichier n’existe pas.
settings-not-found-install-it-from-the-console-or = Introuvable. Installez-le depuis la console ou choisissez le programme.
settings-browse = Parcourir…
settings-sees-the-editor-its-hooks-report-each-edit = Voit l’éditeur ; ses hooks signalent chaque modification et prompt
settings-its-hooks-report-each-edit-and-prompt = Ses hooks signalent chaque modification et prompt
settings-a-plugin-of-snoutcodes-reports-each-edit-and = Un plugin de SnoutCode signale chaque modification et prompt
settings-files-seen-on-disk-while-it-works-in = Fichiers vus sur le disque pendant qu’il travaille dans la console
settings-its-colour-caret-and-marks = Couleur, curseur et marques
settings-the-console-runs = La console exécute
settings-programs = Programmes
settings-your-agent = Votre agent
settings-its-name = Son nom
settings-any-command-line-agent-the-console-runs-it = N’importe quel agent en ligne de commande : la console le lance dans le projet avec /bin/sh -c.
settings-j-shows-or-hides-the-console-the-session = ⌘J affiche ou masque la console. La session continue pendant qu’elle est masquée.
settings-k-sends-the-selection-or-the-whole-file = ⌥⌘K envoie la sélection, ou tout le fichier, à l’agent.
settings-in-a-proposed-changes-tab-accepts-it-and = Dans l’onglet d’une modification proposée, ⌘↩ l’accepte et Échap la rejette.
settings-the-agent-changes-window-keeps-every-edit-an = La fenêtre Modifications des agents conserve chaque modification d’un agent, par prompt, pour l’accepter ou l’annuler.
settings-ghostty-is-unavailable = Ghostty est indisponible
settings-terminal-engine = Moteur du terminal
settings-use-the-editors = Utiliser celle de l’éditeur
settings-the-editors-size = La taille de l’éditeur
settings-blink = Clignotant
settings-look = Apparence
settings-line-height = Hauteur de ligne
settings-cursor = Curseur
settings-remove-this-variable = Supprimer cette variable
settings-shell = Shell
settings-program = Programme
settings-empty-the-login-shell-of-your-account-on = Vide : le shell de connexion de votre compte sur la machine distante.
settings-empty-your-login-shell-on-a-remote-machine = Vide : votre shell de connexion. Sur une machine distante, votre shell là-bas.
settings-arguments = Arguments
settings-separated-by-spaces-l-starts-a-login-shell = Séparés par des espaces. -l lance un shell de connexion.
settings-environment = Environnement
settings-add = Ajouter
settings-added-to-the-terminals-and-the-claude-code = Ajouté aux terminaux et à la console Claude Code que vous lancez après une modification.
settings-behaviour = Comportement
settings-scrollback = Historique
settings-lines-for-terminals-started-after-a-change = lignes, pour les terminaux lancés après une modification
settings-copy-text-as-soon-as-it-is-selected = Copier le texte dès sa sélection
settings-use-option-as-meta-f-moves-a-word = Utiliser Option comme Meta (⌥F avance d’un mot, sans taper ƒ)
settings-external-terminal = Terminal externe
settings-open-in = Ouvrir dans
settings-command = Commande
settings-running-now = En cours
settings-a-terminal-running-another-engine-keeps-it-until = Un terminal utilisant un autre moteur le garde jusqu’à son redémarrage (Nouveau, dans son en-tête).
settings-press-the-keys-or-esc = Appuyez sur les touches, ou Échap
settings-none = Aucun
settings-was-the-shortcut-of-here-it-now-runs = { $keys } était le raccourci de { $join }. Ici, il lance désormais cette commande.
settings-remove-it-there = Le retirer là-bas
settings-no-command-matches = Aucune commande ne correspond.
settings-reset-all = Tout réinitialiser
settings-adds-a-shortcut-press-its-keys-changes-apply = + ajoute un raccourci : appuyez sur ses touches. Les changements s’appliquent avec Appliquer ou OK.
settings-settings = Paramètres
settings-edit-as-json = Modifier en JSON
settings-open-these-settings-file-in-a-tab-with = Ouvrir le fichier de ces paramètres dans un onglet, avec complétion et vérifications
settings-cancel = Annuler
settings-apply = Appliquer
settings-cannot-be-read = { $file } ne peut pas être lu
settings-the-values-it-had-before-stay-in-use = Les valeurs précédentes restent en vigueur jusqu’à sa correction. { $e }
settings-in-shop = { $shell } dans shop
settings-engine = Moteur { $shown }
settings-found = Trouvé : { $path }
settings-editors = Celle de l’éditeur ({ $editor_font })
settings-not-installed = { $chosen } (non installé)
settings-in = { $kind } dans { $root }
settings-reset = Réinitialiser
settings-also-bound-to = Également associé à { $join }
settings-no-jdk = Aucun JDK trouvé sur { $machine }.
settings-after-a-delay = Après un délai
settings-only-while-is-held = Seulement avec ⌘ enfoncée
settings-block = Bloc
settings-bar = Barre
settings-underline = Souligné
settings-hover-hint = Pour les problèmes et leurs correctifs, et la description d’un symbole. ⌘ l’affiche toujours aussitôt, avec le lien vers la déclaration.
settings-click-hint = Maintenez-la pour souligner un nom comme un lien, et cliquez pour aller à sa déclaration (sur la déclaration, à ses usages). ⌥⌘-clic mène aux implémentations.
settings-blame-hint = D’après le blame Git du fichier. L’auteur de la ligne s’affiche quand le curseur reste immobile le temps du délai de survol (Éditeur ▸ Survol). Annoter avec Git Blame se trouve dans le menu contextuel de la gouttière et dans le menu Git.
settings-daemons-hint = Les daemons Gradle de SnoutCode s’arrêtent après cette durée sans build. Une nouvelle valeur vaut pour les daemons lancés après le redémarrage du moteur Java.
settings-profiles-hint = La fenêtre Spring évalue les beans avec ces profils et grise ceux qu’ils excluent. Son menu de profils change ce même réglage.
settings-agent-colours-hint = Chaque agent écrit dans sa couleur : un curseur à son nom pendant qu’il tape, une marque dans la gouttière près des lignes modifiées, et un point sur les onglets et fichiers modifiés jusqu’à votre relecture.
settings-programs-hint = Laissez vide et SnoutCode trouve chaque programme, dans votre PATH et là où les installeurs le placent. Parcourez pour en choisir un vous-même.
settings-terminal-scope-hint = L’apparence, le moteur et les touches des terminaux, ainsi que le terminal externe, sont les vôtres : ils sont identiques dans tous les projets.
settings-engine-hint = Le moteur lit ce qui s’exécute dans les terminaux et la console Claude Code. Un changement vaut pour les terminaux lancés ensuite ; ceux déjà ouverts gardent leur moteur.
settings-ghostty-preview-hint = Ghostty affiche aussi l’appel obsolète barré, sa ligne d’avertissement ondulée et le lien du rapport de tests.
settings-custom-terminal-hint = Exécuté avec /bin/sh. { "{dir}" } est le dossier ; sur une machine distante, { "{ssh}" } est la commande ssh qui y ouvre un shell.
settings-external-terminal-hint = Ouvrir dans un terminal externe, dans le menu Fichier et dans le menu d’un fichier ou dossier, l’ouvre à ce dossier. Sur un projet distant, il y ouvre une session ssh.
settings-unit-minutes = minutes

## dialogs

dialogs-discard-body = { $count ->
    [one] Ce fichier revient à son état dans HEAD. Les fichiers non suivis sont supprimés.
   *[other] Ces { $count } fichiers reviennent à leur état dans HEAD. Les fichiers non suivis sont supprimés.
    }
dialogs-trash-body = { $count ->
    [one] Ce fichier part à la corbeille. Les fichiers suivis apparaissent alors comme supprimés, prêts à être commités.
   *[other] Ces { $count } fichiers partent à la corbeille. Les fichiers suivis apparaissent alors comme supprimés, prêts à être commités.
    }
dialogs-delete-body = { $count ->
    [one] Ce fichier est supprimé du disque. Les fichiers suivis apparaissent alors comme supprimés, prêts à être commités.
   *[other] Ces { $count } fichiers sont supprimés du disque. Les fichiers suivis apparaissent alors comme supprimés, prêts à être commités.
    }
dialogs-unsaved-files = { $count ->
    [one] Un fichier contient des modifications non enregistrées.
   *[other] { $count } fichiers contiennent des modifications non enregistrées.
    }
dialogs-version = Version { $cargo_pkg_version }
dialogs-git-stopped-and-said-why = Git s’est arrêté et a dit pourquoi.
dialogs-you-have-uncommitted-changes-what-should-happen-to = Vous avez des modifications non commitées. Que doit-on en faire ?
dialogs-a-line-of-work-of-its-own-from = Une ligne de travail à part, à partir du point que vous choisissez.
dialogs-its-commits-stay-wherever-else-they-are-reachable = Ses commits restent partout où ils sont encore accessibles.
dialogs-it-goes-for-everyone-who-uses-the-remote = Elle disparaît pour tous les utilisateurs du dépôt distant. Les branches locales ne changent pas.
dialogs-copy-a-remote-repository-to-this-computer-and = Copier un dépôt distant sur cet ordinateur et l’ouvrir.
dialogs-is-waiting-in-the-console-for-your-answer = { $agent } attend votre réponse dans la console.
dialogs-the-window-closes-and-the-java-engine-stops = La fenêtre se ferme, et le moteur Java s’arrête avec elle.
dialogs-what-runs-now-stops-when-snoutcode-quits = Ce qui tourne maintenant s’arrêtera à la fermeture de SnoutCode.
dialogs-clone-repository = Cloner le dépôt
dialogs-local-branches = Branches locales
dialogs-remote-branches = Branches distantes
dialogs-tags = Tags
dialogs-unsaved = Non enregistré
dialogs-these-stop = Ces éléments s’arrêtent :
dialogs-dont-ask-again-settings-appearance-confirm-before-quitti = Ne plus demander (Paramètres, Apparence : Confirmer avant de quitter)
dialogs-discard-and-quit = Abandonner et quitter
dialogs-save-all-and-quit = Tout enregistrer et quitter
dialogs-an-ide-for-java-gradle-and-spring-boot = Un IDE pour Java, Gradle et Spring Boot, avec Git et Claude Code intégrés.
dialogs-made-by-snoutdata-mit-license = Créé par SnoutData. Licence MIT.
dialogs-this-cannot-be-undone = Cette action est irréversible.
dialogs-discard = Abandonner
dialogs-you-can-still-get-them-back-from-the = Vous pourrez encore les récupérer dans la corbeille.
dialogs-move-to-trash = Placer dans la corbeille
dialogs-delete = Supprimer
dialogs-stash-check-out-re-apply = Stash, basculer, réappliquer
dialogs-put-the-changes-aside-switch-then-bring-them = Mettre les modifications de côté, basculer, puis les ramener. Fonctionne même si elles touchent des fichiers différents sur la cible.
dialogs-check-out-keep-changes = Basculer, garder les modifications
dialogs-carry-the-changes-across-as-they-are-git = Emporter les modifications telles quelles. Git refuse si elles devaient être écrasées.
dialogs-this-leaves-head-detached-create-a-branch-if = HEAD sera détachée. Créez une branche pour garder de nouveaux commits.
dialogs-and-have-each-moved-on-and-this-repositorys = { $branch } et { $upstream } ont chacune avancé, et les réglages de ce dépôt n’indiquent pas comment les combiner.
dialogs-merge = Merge
dialogs-join-both-histories-with-a-merge-commit-nothing = Réunir les deux historiques par un commit de merge. Rien n’est réécrit.
dialogs-rebase = Rebase
dialogs-replay-your-commits-on-top-of-the-remote = Rejouer vos commits par-dessus ceux du dépôt distant, pour un historique linéaire.
dialogs-and-have-no-commit-in-common-so-they = { $branch } et { $upstream } n’ont aucun commit en commun : impossible de les fusionner. En général, le dépôt distant a été recréé ou son historique réécrit.
dialogs-resetting-makes-match-exactly-your-current-commits-stay = La réinitialisation aligne { $branch } exactement sur { $upstream }. Vos commits actuels restent sur une branche de sauvegarde : rien n’est perdu.
dialogs-commit-or-stash-your-uncommitted-changes-first-resetting = Commitez ou mettez de côté vos modifications d’abord : la réinitialisation les supprimerait.
dialogs-reset-to = Réinitialiser sur { $upstream }
dialogs-name = Nom
dialogs-starting-at = À partir de
dialogs-check-out-the-new-branch = Basculer vers la nouvelle branche
dialogs-create = Créer
dialogs-renaming = Renommage de { $old }
dialogs-goes-into-the-new-group = { $repo } rejoint le nouveau groupe.
dialogs-groups-keep-related-repositories-together-drag-repositor = Les groupes rassemblent les dépôts liés. Faites-y glisser des dépôts.
dialogs-rename = Renommer
dialogs-goes-away = sera supprimée.
dialogs-delete-even-if-it-is-not-fully-merged = Supprimer même si elle n’est pas entièrement fusionnée
dialogs-also-delete-on-the-remote = Supprimer aussi { $up } sur le dépôt distant
dialogs-removing-it-from-the-remote-affects-everyone-who = La supprimer du dépôt distant affecte tous ceux qui l’utilisent.
dialogs-delete-branch = Supprimer la branche
dialogs-repository-url = URL du dépôt
dialogs-into-folder = Dans le dossier
dialogs-lands-in = Destination :
dialogs-clone = Cloner
dialogs-no-matching-branch-tag-or-commit = Aucune branche, aucun tag ni commit correspondant
dialogs-uncommitted-changes = Modifications non commitées :
dialogs-keep-them = Les garder
dialogs-stash-and-re-apply = Stash et réappliquer
dialogs-check-out = Basculer
dialogs-dont-allow = Refuser
dialogs-allow-for-this-session = Autoriser pour cette session
dialogs-allow = Autoriser
dialogs-close = Fermer
dialogs-a-branch-a-tag-or-any-commit = Une branche, un tag ou n’importe quel commit.
dialogs-something-went-wrong = Un problème est survenu
dialogs-discard-changes = Annuler les modifications ?
dialogs-check-out-2 = Basculer vers { $t } ?
dialogs-check-out-3 = Basculer
dialogs-new-branch = Nouvelle branche
dialogs-delete-branch-2 = Supprimer la branche ?
dialogs-delete-branch-from = Supprimer la branche de { $remote } ?
dialogs-move-to-the-trash = Placer dans la corbeille ?
dialogs-delete-files = Supprimer les fichiers ?
dialogs-both-sides-have-new-commits = Les deux côtés ont de nouveaux commits
dialogs-no-shared-history = Aucun historique commun
dialogs-git-could-not-sign-in-to = Git n’a pas pu se connecter à { $host }
dialogs-git-could-not-sign-in = Git n’a pas pu se connecter
dialogs-sign-in-helper-missing = Le gestionnaire d’identifiants de ce dépôt a besoin de { $program }, qui n’est pas installé ici.
dialogs-sign-in-helper-broken = Le gestionnaire d’identifiants de ce dépôt ne peut pas s’exécuter sur cet ordinateur.
dialogs-sign-in-rejected = Le serveur a refusé le nom d’utilisateur et le jeton envoyés par Git.
dialogs-sign-in-none = Git n’a ni nom d’utilisateur ni jeton pour ce serveur.
dialogs-sign-in-no-access = Le serveur dit que le dépôt n’existe pas, ou ce compte ne peut pas le voir.
dialogs-sign-in-ssh-key = Le serveur n’a accepté aucune des clés SSH de cet ordinateur.
dialogs-sign-in-host-key = SSH ne connaît pas encore la clé d’hôte de ce serveur, ou elle a changé.
dialogs-sign-in-fix-helper = Cela arrive quand un dépôt a été copié depuis un autre ordinateur avec ses réglages, ou quand la connexion enregistrée ici a été perdue. Installez le programme, ou retirez le gestionnaire et reconnectez-vous avec vos propres identifiants.
dialogs-sign-in-fix-rejected = Le jeton enregistré a peut-être expiré, été révoqué, ou appartient à un autre compte. Reconnectez-vous, ou créez un nouveau jeton et enregistrez-le.
dialogs-sign-in-fix-none = Connectez-vous une fois avec un gestionnaire d’identifiants qui l’enregistre, ou passez le remote en SSH.
dialogs-sign-in-fix-no-access = Vérifiez l’adresse du remote, et que le compte utilisé par Git voit le dépôt. Avec plusieurs comptes, celui enregistré n’est peut-être pas le bon.
dialogs-sign-in-fix-ssh-key = Ajoutez la clé publique de cet ordinateur à votre compte sur le serveur, ou chargez votre clé dans l’agent SSH.
dialogs-sign-in-fix-host-key = Connectez-vous une fois dans un terminal, vérifiez l’empreinte affichée par le serveur, puis acceptez-la. Une clé qui a changé peut signifier que le serveur a été remplacé, ou que quelqu’un s’est interposé.
dialogs-sign-in-agent = { $agent } reçoit ce que Git a dit, trouve la cause avec vous et vous guide jusqu’à la solution.
dialogs-fix-with = Corriger avec { $agent }
dialogs-open-terminal = Ouvrir un terminal
dialogs-rename-group = Renommer le groupe
dialogs-new-group = Nouveau groupe
dialogs-quit-snoutcode = Quitter SnoutCode ?
dialogs-allow-to = Autoriser { $agent } à { $what } ?
dialogs-its-upstream = sa branche amont
dialogs-delete-from = Supprimer de { $remote }
dialogs-allow-for-this-session-lets-it-use-again = Autoriser pour cette session lui permet de réutiliser { $tool } sans demander.

## welcome

welcome-minutes-ago = { $count ->
    [one] il y a { $count } minute
   *[other] il y a { $count } minutes
    }
welcome-hours-ago = { $count ->
    [one] il y a { $count } heure
   *[other] il y a { $count } heures
    }
welcome-days-ago = { $count ->
    [one] il y a { $count } jour
   *[other] il y a { $count } jours
    }
welcome-weeks-ago = { $count ->
    [one] il y a { $count } semaine
   *[other] il y a { $count } semaines
    }
welcome-months-ago = { $count ->
    [one] il y a { $count } mois
   *[other] il y a { $count } mois
    }
welcome-just-now = à l’instant
welcome-yesterday = hier
welcome-search-recent-projects = Rechercher dans les projets récents
welcome-welcome = Bienvenue
welcome-go-to-file = Aller au fichier
welcome-recent-files = Fichiers récents
welcome-search-everywhere = Rechercher partout
welcome-find-in-files = Rechercher dans les fichiers
welcome-new-file = Nouveau fichier
welcome-no-files-open = Aucun fichier ouvert
welcome-open-one-from-the-project-tree-or = Ouvrez-en un depuis l’arborescence du projet, ou :
welcome-recent-files-2 = Fichiers récents
welcome-clone-repository = Cloner un dépôt…
welcome-from-a-git-url = Depuis une URL Git
welcome-java-or-spring-boot-app = Projet Java ou Spring Boot
welcome-no-recent-project-matches = Aucun projet récent ne correspond.
welcome-no-recent-projects-yet-open-a-folder-to = Pas encore de projet récent. Ouvrez un dossier pour commencer.
welcome-recent-projects = Projets récents
welcome-a-folder-on = Un dossier sur { $host }
welcome-folder-on-machine = Un dossier sur { $machine }

## tabs

tabs-unpin-tab = Détacher l’onglet
tabs-pin-tab = Épingler l’onglet
tabs-close-others = Fermer les autres
tabs-close-tabs-to-the-left = Fermer les onglets à gauche
tabs-close-tabs-to-the-right = Fermer les onglets à droite
tabs-close-all = Tout fermer
tabs-close-all-but-pinned = Tout fermer sauf les épinglés
tabs-keep-open = Garder ouvert
tabs-reload-from-disk = Recharger depuis le disque
tabs-copy-path = Copier le chemin
tabs-clear = Effacer
tabs-sync-the-gradle-project = Synchroniser le projet Gradle
tabs-read-the-tasks-again = Relire les tâches
tabs-collapse-all = Tout replier
tabs-reload-the-maven-project = Recharger le projet Maven
tabs-read-the-goals-again = Relire les goals
tabs-stop-idle-processes-idle-gradle-daemons-leftovers-and = Arrêter les processus inactifs : daemons Gradle inactifs, restes et moteurs des projets fermés
tabs-options = Options
tabs-hide = Masquer

## projects

projects-filter = Filtrer
projects-open-local-repository = Ouvrir un dépôt local…
projects-find-repositories-in-folder = Rechercher des dépôts dans un dossier…
projects-no-matches = Aucun résultat
projects-rename = Renommer…
projects-delete-group = Supprimer le groupe
projects-new-group = Nouveau groupe…
projects-drag-repositories-here = Faites glisser des dépôts ici
projects-add-a-repository-or-a-group = Ajouter un dépôt ou un groupe
projects-repositories = Dépôts
projects-no-repositories-yet-use-or-o = Pas encore de dépôt. Utilisez + ou ⌘O.
projects-copy-path = Copier le chemin
projects-open-as-project = Ouvrir comme projet
projects-move-to-group = Déplacer vers un groupe
projects-no-group = Aucun groupe
projects-remove-from-list = Retirer de la liste

## status

status-snoutcodes-processes-running-of-memory-click-to-show = Processus de SnoutCode : { $count } en cours, { $memory } de mémoire. Cliquez pour les afficher.
status-in-the-background = En arrière-plan
status-the-editor-works-meanwhile = L’éditeur reste utilisable.
status-reconnect = Se reconnecter
status-changes-the-agents-made-that-wait-for-your = Modifications des agents en attente de votre relecture
status-review = Relire
status-restart = Redémarrer
status-read-only = Lecture seule
status-java-support-unavailable = Prise en charge de Java indisponible
status-chars-lines = { $chars } caractères, { $lines } lignes
status-selected = { $chars } sélectionnés
status-and-more = et { $count } de plus

## search

search-go-to-file = Aller au fichier
search-line-or-line-column = Ligne, ou ligne:colonne
search-type-a-command = Saisissez une commande
search-run-a-gradle-task-or-maven-goal-its = Exécuter une tâche Gradle ou un goal Maven : son nom ou son chemin
search-spring-profiles-pick-one-to-turn-it-on = Profils Spring : choisissez-en un pour l’activer ou le désactiver
search-go-to-symbol = Aller au symbole
search-filter-the-files-symbols = Filtrer les symboles du fichier
search-go-to-class-in-the-project-its-libraries = Aller à la classe, dans le projet, ses bibliothèques et le JDK
search-new-file-its-path-in-the-project-like = Nouveau fichier : son chemin dans le projet, comme src/main/java/App.java
search-no-file-matches = Aucun fichier ne correspond
search-no-command-matches = Aucune commande ne correspond
search-no-task-matches = Aucune tâche ne correspond
search-no-profile-matches = Aucun profil ne correspond
search-type-a-name = Saisissez un nom
search-no-symbol-matches = Aucun symbole ne correspond
search-type-a-class-name-or-its-capitals = Saisissez un nom de classe, ou ses majuscules
search-no-class-matches = Aucune classe ne correspond
search-no-recent-file-matches = Aucun fichier récent ne correspond
search-return-makes-the-file-and-opens-it = Entrée crée le fichier et l’ouvre
search-type-a-line-number-or-line-column = Saisissez un numéro de ligne, ou ligne:colonne
search-go-to-line-column = Aller à la ligne { $line }, colonne { $column }
search-try-fewer-letters-or-another-word = Essayez moins de lettres ou un autre mot.

## find

find-matches = { $count ->
    [one] { $count } occurrence
   *[other] { $count } occurrences
    }
find-files = { $count ->
    [one] { $count } fichier
   *[other] { $count } fichiers
    }
find-matches-in-files = { $matches } dans { $files }
find-replaced = { $matches } remplacées dans { $files }
find-put-back = Restauration : { $files }
find-to-replace = , { $ticked } à remplacer
find-first-only = {" "}(les premières seulement)
find-skipped =  ; laissés tels quels car modifiés entre-temps : { $files }
find-undo-hint = . Édition ▸ Annuler le remplacement dans les fichiers les restaure.
find-not-a-regular-expression = Expression régulière non valide : { $error }
find-the-regular-expression-is-too-large = L’expression régulière est trop grande
find-module = Module
find-directory = Répertoire
find-open-files = Fichiers ouverts
find-text-to-find = Texte à rechercher
find-replace-with = Remplacer par
find-file-mask-java-test-java = Masque : *.java, !*Test.java
find-folder-from-the-projects = Dossier, depuis celui du projet
find-match-case = Respecter la casse
find-whole-words = Mots entiers
find-regular-expression-1-in-the-replacement-names-a = Expression régulière ($1 dans le remplacement désigne un groupe)
find-hide-replace = Masquer Remplacer
find-replace-in-files-r = Remplacer dans les fichiers (⇧⌘R)
find-choose = Choisir…
find-find-in-the-projects-files = Rechercher dans les fichiers du projet
find-type-a-word-a-phrase-or-a-regular = Saisissez ci-dessus un mot, une phrase ou une expression régulière.
find-nothing-found = Aucun résultat
find-try-fewer-letters-another-scope-or-a-wider = Essayez moins de lettres, une autre portée ou un masque plus large.
find-searching = Recherche…
find-replace-selected = Remplacer la sélection
find-replace-all = Tout remplacer
find-open = Ouvrir
find-replace-in-files = Remplacer dans les fichiers
find-no-replacement-to-undo = Aucun remplacement à annuler
find-not-a-file-mask = Masque de fichier non valide : { $kind }
find-module-2 = Module : { $name }
find-of = { $ticked } sur { $count }

## files

files-no-project-open = Aucun projet ouvert
files-open-a-project-to-see-its-files = Ouvrez un projet pour voir ses fichiers.
files-the-connection-is-lost = La connexion est perdue
files-node-package = Paquet Node : { $path }
files-the-files-on-come-back-when-it-reconnects = Les fichiers de { $host } reviendront à sa reconnexion.

## project

project-no-classes-to-show = Aucune classe à afficher
project-reading-the-build-model = Lecture du modèle de build
project-external-libraries = Bibliothèques externes
project-the-build-model-could-not-be-read = Impossible de lire le modèle de build : { $e }
project-uses = utilise { $join }
project-showing-the-last-model-that-synced = { $reason }{"\u000A"}Affichage du dernier modèle synchronisé.
project-compiled-by = Compilé par { $join }
project-did-not-sync = Échec de la synchronisation : { $reason }{"\u000A"}{"\u000A"}{ $note }
project-did-not-sync-2 = Échec de la synchronisation : { $reason }
project-excluded = Exclu : { $path }
project-and-more = { $builds } et { $n } de plus

## model

model-build-files = Fichiers de build
model-maven-files = Fichiers Maven
model-gradle-scripts = Scripts Gradle
model-detached-at = détachée à { $count }
model-detached-head = HEAD détachée
model-does-not-exist = { $path } n’existe pas
model-ready = Prêt
model-no-upstream = {"  "}(pas d’upstream)
model-opened = { $asked } ouvert
model-snoutcode-could-not-connect-to = SnoutCode n’a pas pu se connecter à { $connection }. { $why }
model-showing-in-git = Affichage de { $root } dans Git
model-closed = { $closed } fermé
model-could-not-save-settings = Impossible d’enregistrer les paramètres : { $e }
model-your-settings-json-has-an-error-so-the = Votre settings.json contient une erreur, les paramètres en vigueur restent donc inchangés : { $e }
model-your-settings-are-read-again = Vos paramètres ont été relus
model-could-not-save-the-projects-settings = Impossible d’enregistrer les paramètres du projet : { $e }
model-finished = { $label } terminé
model-cloned-into = Cloné dans { $root }
model-failed = Échec de { $label }
model-failed-2 = Échec de { $label } :{"\u000A"}{ $e }
model-no-git-repositories-within-levels-of = Aucun dépôt Git à moins de { $SCAN_DEPTH } niveaux de{"\u000A"}{ $root }
model-found-repositories-in-new = { $count } dépôts trouvés dans { $root } ({ $added } nouveaux)
model-asking-the-agent-to-draft-the-commit-message = Demande à l’agent de rédiger le message de commit…
model-the-agent-has-not-sent-a-commit-message = L’agent n’a pas encore envoyé de message de commit. Sa réponse est dans la console.
model-asks-you-something-in-the-console-answer-it = { $name } vous pose une question dans la console. Répondez-y et la rédaction continuera.
model-is-writing-the-commit-message = { $name } rédige le message de commit…
model-the-agent-finished-without-writing-a-commit-message = L’agent a terminé sans écrire de message de commit. Sa réponse est dans la console.
model-no-commit-message-yet = Pas encore de message de commit
model-drafted-the-commit-message = { $name } a rédigé le message de commit
model-refreshed = Actualisé
model-open-project = Ouvrir un projet
model-initialized-repository-in = Dépôt initialisé dans { $root }
model-find-repositories-in-folder = Rechercher des dépôts dans un dossier
model-searching-for-repositories = Recherche de dépôts
model-staged = { $count ->
    [one] 1 fichier indexé
   *[other] { $count } fichiers indexés
    }
model-discarded = { $count ->
    [one] 1 fichier abandonné
   *[other] { $count } fichiers abandonnés
    }
model-trashed = { $count ->
    [one] 1 fichier placé dans la corbeille
   *[other] { $count } fichiers placés dans la corbeille
    }
model-deleted = { $count ->
    [one] 1 fichier supprimé
   *[other] { $count } fichiers supprimés
    }
model-unstaged = Désindexé
model-added-to-gitignore = { $pattern } ajouté à .gitignore
model-added-rules-to-gitignore = { $n } règles ajoutées à .gitignore
model-committed = Commit créé : { $summary }
model-stopped-drafting = Rédaction arrêtée
model-checked-out = Basculé vers { $target }
model-local-changes-conflicted = Conflit avec les modifications locales
model-created-branch = Branche { $name } créée
model-deleted-branch = Branche { $name } supprimée
model-delete = Supprimer { $r }
model-pull-merge = Pull (merge)
model-pull-rebase = Pull (rebase)
model-reset-to = { $head } réinitialisée sur { $upstream }
model-your-previous-commits-are-kept-on-the-branch = Vos commits précédents sont conservés sur la branche { $backup }. Supprimez-la quand vous n’en aurez plus besoin.
model-now-matches = { $head } correspond maintenant à { $upstream }
model-commit = Commit { $short_id } { $summary }
model-not-in-the-repository = { $path } n’est pas dans le dépôt affiché par la vue Git
model-sources = Sources
model-test-sources = Sources de test
model-test-resources = Ressources de test
model-generated-sources = Sources générées
model-the-build-did-not-sync = Le build ne s’est pas synchronisé
model-sync = Synchronisation : { $root }
model-the-gradle-sync-failed = La synchronisation Gradle a échoué

## branch

branch-search-branches = Rechercher des branches
branch-local = Local
branch-remote = Distant

## panels

panels-files = Fichiers
panels-no-output-yet = Pas encore de sortie
panels-what-git-and-the-tools-say-shows-here = Ce que disent Git et les outils s’affiche ici pendant qu’ils travaillent.
panels-open-git = Ouvrir Git

## title

title-no-project = Aucun projet
title-current-file = Fichier courant
title-yours = Les vôtres
title-reading-the-configurations = Lecture des configurations…
title-configurations-appear-once-the-tests-are-listed = Les configurations apparaîtront une fois les tests listés
title-configurations-appear-once-the-java-engine-is-ready = Les configurations apparaîtront quand le moteur Java sera prêt
title-stop-f2 = Arrêter (⌘F2)
title-search-everywhere-a = Rechercher partout (⇧⌘A)
title-settings = Paramètres (⌘,)
title-the-current-file = le fichier courant
title-rerun-r = Réexécuter { $what } (⌃R)
title-run-r = Exécuter { $what } (⌃R)
title-debug-d = Déboguer { $what } (⌃D)

## stripes

stripes-left = Gauche
stripes-right = Droite
stripes-bottom = Bas
stripes-move-to = Déplacer vers
stripes-build = Compilation
stripes-problems-m = Problèmes (⇧⌘M)
stripes-agent-console-j = Console de l’agent (⌘J)
stripes-spring = Spring
stripes-notifications = Notifications
stripes-structure-7 = Structure (⌘7)
stripes-hierarchy-h-h = Hiérarchie (⌃H, ⌃⌥H)
stripes-agent-view = Vue de l’agent
stripes-tests = Tests
stripes-project-1 = Projet (⌘1)
stripes-git-2 = Git (⌘2)

## git

git-state = État
git-relative-directory = Répertoire relatif
git-file-filter = Filtrer les fichiers
git-message-author-sha-or-ref = Message, auteur, SHA ou réf.
git-commit-message = Message de commit
git-message = Message
git-author = Auteur
git-date = Date
git-push = Push ({ $n })
git-hide = Masquer { $short }
git-console-in-this-repository-j = Console { $agent } dans ce dépôt (⌘J)
git-no-github-remote = Aucun dépôt distant GitHub
git-open-local-repository-o = Ouvrir un dépôt local (⌘O)
git-git-push-sets-upstream-on-first-push = git push (définit l’upstream au premier push)
git-move-to-the-trash = Placer dans la corbeille (⌘⌫)
git-branch-tag-or-commit-o = Branche, tag ou commit (⇧⌘O)
git-new-branch = Nouvelle branche
git-no-repository-open = Aucun dépôt ouvert
git-local-branches = Branches locales
git-tags = Tags ({ $count })
git-branches = Branches
git-check-out-o = Basculer… (⇧⌘O)
git-double-click-to-check-out = { $b }{ $u }{"\u000A"}{ $target } { $subject }{"\u000A"}{"\u000A"}Double-cliquez pour basculer
git-double-click-to-check-out-detached = { $target } { $subject }{"\u000A"}{ $date }{"\u000A"}{"\u000A"}Double-cliquez pour basculer (HEAD détachée)
git-delete = Supprimer…
git-delete-from = Supprimer de { $remote }…
git-check-out-as-local = Basculer en local « { $local_name } »
git-delete-branch = Supprimer la branche…
git-delete-from-the-remote = Supprimer du dépôt distant…
git-create-branch-from-here = Créer une branche à partir d’ici…
git-copy-name = Copier le nom
git-check-out-detached = Basculer (HEAD détachée)
git-create-branch-from-tag = Créer une branche depuis le tag…
git-tag = tag { $name }
git-nothing-to-commit-on = Rien à commiter sur { $head }
git-nothing-to-commit = Rien à commiter
git-show-all-files = Afficher tous les fichiers
git-in = dans { $dir }/
git-the-working-tree-is-clean = L’arbre de travail est propre
git-discard = Annuler…
git-add-to-gitignore = Ajouter à .gitignore
git-see-console = Voir la console
git-writing = Rédaction…
git-writes-the-message-from-building-on-anything-you = { $agent } rédige le message à partir de { $what }, en s’appuyant sur ce que vous avez saisi. S’il ne tourne pas, il démarre dans la console.
git-draft = Rédiger
git-amend = Amender
git-amend-last-commit = Amender le dernier commit
git-commits = Commite { $what }  ⌘↩
git-files-selected = { $count } fichiers sélectionnés
git-select-a-file-to-see-its-changes = Sélectionnez un fichier pour voir ses modifications
git-changes = Modifications
git-nothing-staged = Rien d’indexé
git-stage-files-to-see-what-the-next-commit = Indexez des fichiers pour voir le contenu du prochain commit.
git-journal = Journal
git-click-a-commit-to-see-what-it-changed = Cliquez sur un commit pour voir ce qu’il a modifié
git-history-of = Historique de { $name }
git-history-of-tooltip = Les commits qui ont modifié { $path }, renommages suivis. Cliquez pour revoir tout le Journal.
git-no-commits-changed-it = Aucun commit n’a modifié ce fichier
git-it-is-not-committed-yet = Il est nouveau, ou n’a jamais été commité sur cette branche.
git-no-commits-yet = Pas encore de commit
git-stage-some-files-and-make-the-first-one = Indexez des fichiers et créez le premier
git-show-changes = Afficher les modifications
git-check-out-this-commit-detached = Basculer sur ce commit (HEAD détachée)
git-create-branch-here = Créer une branche ici…
git-copy-sha = Copier le SHA
git-copy-message = Copier le message
git-working-tree = Arbre de travail
git-back-esc = Retour (Échap)
git-the-commit-could-not-be-read = Impossible de lire le commit
git-from = { $directory }  (depuis { $orig })
git-this-commit-changes-no-files = Ce commit ne modifie aucun fichier
git-committer = Committer
git-committed = Commité
git-parents = Parents
git-parent = Parent
git-none-this-is-the-first-commit = aucun, c’est le premier commit
git-select-a-file = Sélectionnez un fichier
git-nothing = (rien)
git-this-commit = Ce commit
git-git-could-not-read-this-folder = Git n’a pas pu lire ce dossier
git-the-project-works-without-it-but-its-changes = Le projet fonctionne sans, mais ses modifications et son historique ne peuvent pas s’afficher.
git-this-folder-has-no-git-repository = Ce dossier ne contient pas de dépôt Git
git-the-project-works-without-git-start-a-repository = Le projet fonctionne sans Git. Créez un dépôt pour suivre ses modifications et son historique.
git-try-again = Réessayer
git-initialize-a-repository-here = Initialiser un dépôt ici
git-find-repositories-inside = Rechercher des dépôts à l’intérieur
git-open-a-project-to-see-its-changes-branches = Ouvrez un projet pour voir ses modifications, ses branches et son historique.
git-open-project-o = Ouvrir un projet…   ⌘O
git-unified = Unifié
git-side-by-side = Côte à côte
git-compact-hide-unchanged-lines-away-from-changes = Compact : masquer les lignes inchangées loin des modifications
git-compact = Compact
git-hide-unchanged-lines-away-from-changes = Masquer les lignes inchangées loin des modifications
git-no-diff = Aucun diff
git-binary-file = Fichier binaire
git-its-versions-differ-as-bytes = Ses versions diffèrent au niveau des octets.
git-too-large-to-show = Trop volumineux pour être affiché
git-the-file-is-too-large-to-diff-here = Le fichier est trop volumineux pour un diff ici.
git-no-differences = Aucune différence
git-and-are-the-same = { $left } et { $right } sont identiques.
git-changed = { $count ->
    [one] { $count } modifié
   *[other] { $count } modifiés
    }
git-changed-staged = { $count ->
    [one] { $count } modifié
   *[other] { $count } modifiés
    }, { $staged_count ->
    [one] { $staged_count } indexé
   *[other] { $staged_count } indexés
    }

## console

console-console = Console
console-the-installer-is-still-at-work-end-stops = L’installateur travaille encore ; Terminer l’arrête
console-ended = { $name } s’est terminé
console-ended-resume = { $name } s’est terminé ; Continuer le reprend
console-your-own-agent-has-no-command-yet-give = Votre agent n’a pas encore de commande. Donnez-lui-en une dans Paramètres, Agents, puis cliquez de nouveau sur Rédiger.
console-find-in-terminal = Rechercher dans le terminal
console-of = { $i } sur { $n }
console-previous-match-return = Occurrence précédente (Entrée)
console-next-match-shift-return = Occurrence suivante (Maj+Entrée)
console-close-esc = Fermer (Échap)
console-open-a-project-to-run-a-shell-in = Ouvrez un projet pour y lancer un shell.
console-not-running = Arrêté
console-starting = Démarrage
console-connected = Connecté
console-not-connected = Non connecté
console-running = En cours
console-working = Au travail
console-installing = Installation
console-ended-2 = Terminé
console-is-not-running = { $agent } n’est pas lancé
console-is-starting = { $agent } démarre
console-sees-the-editor-its-selection-open-files-and = { $agent } voit l’éditeur : sa sélection, ses fichiers ouverts et ses problèmes ; il y propose des modifications à accepter ou rejeter, et ses éditions s’affichent au fur et à mesure
console-runs-but-has-not-connected-to-the-editor = { $agent } tourne, mais ne s’est pas encore connecté à l’éditeur
console-runs-the-files-it-changes-show-as-it = { $agent } tourne ; les fichiers qu’il modifie s’affichent au fur et à mesure
console-is-at-work-on-your-prompt = { $agent } travaille sur votre prompt
console-is-being-installed = { $agent } est en cours d’installation
console-has-ended = { $agent } s’est terminé
console-the-shell = Le shell
console-follow = Suivre
console-following-the-editor-goes-to-the-files-the = Suivi activé : l’éditeur va aux fichiers que l’agent modifie, pendant qu’il écrit
console-not-following-the-editor-stays-where-you-are = Suivi désactivé : l’éditeur reste où vous êtes ; des points marquent ce que l’agent a modifié
console-start-again-with-a-new-conversation = Recommencer avec une nouvelle conversation
console-start-a-new-shell = Lancer un nouveau shell
console-new = Nouveau
console-continue = Continuer
console-start-again-on-the-most-recent-conversation-in = Reprendre la conversation la plus récente de ce dossier
console-end = Terminer
console-end-this-session = Terminer cette session
console-hide-j-the-session-keeps-running = Masquer (⌘J) ; la session continue
console-starting-the-shell = Démarrage du shell…
console-starting-2 = Démarrage de { $agent }…
console-the-shell-is-not-running = Le shell n’est pas lancé
console-start-a-shell-in-the-project = Lancez un shell dans le projet.
console-start-shell = Lancer le shell
console-start-claude = Lancer Claude
console-start-codex = Lancer Codex
console-start-opencode = Lancer opencode
console-start-amp = Lancer Amp
console-start-agent = Lancer l’agent
console-it-sees-the-editor-and-its-edits-type = Il voit l’éditeur, et ses modifications s’écrivent d’elles-mêmes, dans sa couleur, pour que vous les relisiez.
console-the-files-it-changes-type-themselves-in-in = Les fichiers qu’il modifie s’écrivent d’eux-mêmes, dans sa couleur, pour que vous les relisiez.
console-continue-last-session = Reprendre la dernière session
console-install-here = Installer lance son installateur ici, dans la console. S’il est installé ailleurs, indiquez son chemin dans Paramètres, Agents.
console-install-here-node = Installer lance son installateur ici, dans la console. Il faut Node.js. S’il est installé ailleurs, indiquez son chemin dans Paramètres, Agents.
console-is-not-installed = { $agent } n’est pas installé
console-set-its-path-in-settings-agents = Indiquez son chemin dans Paramètres, Agents.
console-your-own-agent-has-no-command-yet = Votre agent n’a pas encore de commande
console-name-the-command-line-agent-to-run-here = Indiquez l’agent en ligne de commande à lancer ici et le nom à afficher, dans Paramètres, Agents.
console-agent-settings = Paramètres de l’agent
console-set-its-path = Indiquer son chemin
console-s-developers-recommend-running-it-in-wsl = Les développeurs de { $name } recommandent de l’exécuter dans WSL.
console-looking-for-in-wsl = Recherche de { $name } dans WSL…
console-open-a-folder-in = Ouvrir un dossier dans { $d }
console-is-installed-in = { $name } est installé dans { $and_value }
console-open-a-folder-in-and-the-console-runs = { $makers } Ouvrez un dossier dans { $has } et la console y lancera { $name }.
console-this-pc-has-no-wsl-distribution-yet-wsl = { $makers } Ce PC n’a pas encore de distribution WSL : `wsl --install` dans PowerShell en installe une. Pour exécuter plutôt { $name } sur Windows, installez-le avec la ligne ci-dessous ou indiquez son chemin.
console-on-windows-itself = Sur Windows
console-install-runs-its-installer-in-then-open-a = { $makers } Installer lance son installateur dans { $first } ; ouvrez ensuite un dossier là-bas et la console y lancera { $name }. Sur Windows, il s’installe avec la deuxième ligne.
console-install-in = Installer dans { $d }
console-in-wsl = Dans WSL
console-the-installer = L’installateur
console-try-again = Réessayer
console-start-a-new-session = Lancer une nouvelle session
console-open-link = Ouvrir le lien
console-copy-link = Copier le lien
console-install-claude-code = Installer Claude Code
console-install-codex = Installer Codex
console-install-opencode = Installer opencode
console-install-amp = Installer Amp
console-install = Installer
console-starts-with-the-next-session = { $agent } démarrera à la prochaine session
console-draft-stopped-the-console-started-a-new-session = Rédaction arrêtée : la console a lancé une nouvelle session. Cliquez sur Rédiger pour demander à { $name }.
console-could-not-start = Impossible de lancer { $program } : { $e }
console-is-not-installed-yet-install-it-in-the = { $agent } n’est pas encore installé. Installez-le dans la console, puis cliquez de nouveau sur Rédiger.
console-is-not-running-2 = { $agent } n’est pas lancé.
console-set-up = { $a } (à configurer)
console-install-2 = { $a } (à installer)
console-open-a-project-to-run-in-it = Ouvrez un projet pour y lancer { $agent }.

## workspace

workspace-could-not-make = Impossible de créer { $path } : { $e }
workspace-do-you-want-to-save-the-changes-you = Voulez-vous enregistrer les modifications apportées à { $one } ?
workspace-do-you-want-to-save-the-changes-you-2 = Voulez-vous enregistrer les modifications apportées à { $count } fichiers ?
workspace-your-changes-are-lost-if-you-dont-save = Vos modifications seront perdues si vous ne les enregistrez pas.
workspace-dont-save = Ne pas enregistrer
workspace-open-a-project-to-see-its-structure = Les paramètres du projet sont pour un projet : ce dossier ne contient aucun build Gradle, Maven ou Node
workspace-settings-on-machine = Votre settings.json se trouve sur { $machine } : ouvrez-le depuis une fenêtre sur { $machine }
workspace-these-settings-are-not-kept-in-a-file = Ces paramètres ne sont pas conservés dans un fichier
workspace-open-a-project-to-edit-its-settings = Ouvrez un projet pour modifier ses paramètres
workspace-the-projects-settings-are-read-again = Les paramètres du projet ont été relus
workspace-has-an-error-so-its-last-good-values = { $file } contient une erreur, ses dernières valeurs valides restent donc en vigueur : { $e }
workspace-reading-the-projects-folders = Lecture des dossiers du projet
workspace-reading-the-open-file-and-the-projects-folders = Lecture du fichier ouvert et des dossiers du projet
workspace-reading-of-open-files-and-the-projects-folders = Lecture de { $n } fichiers ouverts sur { $total } et des dossiers du projet
workspace-nothing-changed-on-disk = Rien n’a changé sur le disque
workspace-1-file-changed-on-disk = 1 fichier a changé sur le disque
workspace-files-changed-on-disk = { $n } fichiers ont changé sur le disque
workspace-drop-a-folder-to-open-it = Déposez un dossier pour l’ouvrir

## tools

tools-problems = Problèmes
tools-references = Références
tools-show-output = Afficher la sortie
tools-no-notifications = Aucune notification
tools-errors-from-git-and-the-tools-show-here = Les erreurs de Git et des outils s’affichent ici.

## state

state-modified = Modifié
state-added = Ajouté
state-deleted = Supprimé
state-renamed = Renommé
state-copied = Copié
state-type-changed = Type modifié
state-untracked = Non suivi
state-conflicted = En conflit
state-staged = Indexé
state-partly-staged = Partiellement indexé

## splash

splash-starting-snoutcode = Démarrage de SnoutCode

## updates

updates-snoutcode-update = Mise à jour de SnoutCode
updates-updated-to-v = Mis à jour vers v{ $version }
updates-dismiss = Ignorer
updates-later = Plus tard
updates-download = Télécharger
updates-restart-to-install-the-update = Redémarrer pour installer la mise à jour
updates-restart-to-update = Redémarrer pour mettre à jour

## background

background-reading-the-gradle-model = Lecture du modèle Gradle
background-compiling-scala = Compilation de Scala
background-starting-java = Démarrage de Java
background-indexing = Indexation
background-restarting-java = Redémarrage de Java
background-waiting-for-snout-engine = En attente de snout-engine
background-attempt-of-5 = {" "}(tentative { $n } sur 5)
background-the-java-engine-stopped-unexpectedly-and-is-starting = Le moteur Java s’est arrêté de façon inattendue et redémarre{ $again } : { $doing }
background-java-stopped-starting-again = Java arrêté, redémarrage
background-building-snout-engine = Compilation de snout-engine

## quit

quit-debugging = Débogage
quit-gradle-build = Build Gradle
quit-compiling-the-scala-classes = Compilation des classes Scala
quit-claude-code-editing = Claude Code, modification de { $name }
quit-the-connection-to-and-what-the-project-runs = La connexion à { $host } et ce que le projet y exécute
quit-a-file-could-not-be-saved-so-snoutcode = Un fichier n’a pas pu être enregistré : SnoutCode n’a pas quitté

## terminal

terminal-custom-command = Commande personnalisée
terminal-command-prompt = Invite de commandes
terminal-the-custom-external-terminal-has-no-command-set = Le terminal externe personnalisé n’a pas de commande : définissez-en une dans Paramètres ▸ Outils ▸ Terminal.
terminal-could-not-run-the-external-terminal-command = Impossible d’exécuter la commande du terminal externe : { $e }
terminal-could-not-open-the-external-terminal = Impossible d’ouvrir le terminal externe : { $e }
terminal-could-not-open-the-external-terminal-2 = Impossible d’ouvrir le terminal externe : { $trim }

## debug

debug-breakpoints-go-in-a-java-kotlin-scala-typescript = Les points d’arrêt se placent dans un fichier Java, Kotlin, Scala, TypeScript ou JavaScript
debug-there-is-no-breakpoint-on-this-line-f8 = Aucun point d’arrêt sur cette ligne : ⌘F8 en pose un
debug-no-spring-bean-injection-or-endpoint-on-this = Aucun bean, injection ou endpoint Spring sur cette ligne
debug-run-to-cursor-works-while-the-debugger-is = Exécuter jusqu’au curseur fonctionne quand le débogueur est arrêté
debug-put-the-caret-on-a-line-of-code = Placez le curseur sur une ligne de code
debug-nothing-to-pause-no-program-is-running-in = Rien à suspendre : aucun programme ne tourne dans le débogueur
debug-nothing-ran-yet-to-run-again = Rien n’a encore été exécuté
debug-no-failed-tests-to-run-again = Aucun test en échec à réexécuter
debug-the-maven-goals-are-being-read-try-again = Lecture des goals Maven en cours ; réessayez dans un instant
debug-no-maven-goals-the-project-has-no-maven = Aucun goal Maven : le projet n’a pas de build Maven, ou il est en cours de lecture
debug-the-gradle-tasks-are-being-read-try-again = Lecture des tâches Gradle en cours ; réessayez dans un instant
debug-no-gradle-tasks-yet-the-projects-build-is = Pas encore de tâche Gradle : le build du projet est en cours de lecture
debug-on = Activé
debug-any-profile-every-bean-may-be-there = Tous les profils : chaque bean peut être présent
debug-as-the-application-files-say = Selon les fichiers de l’application ({ $join })
debug-the-maven-build-is-being-read-try-again = Lecture du build Maven en cours ; réessayez dans un instant
debug-stop-only-when-this-is-true-as-i = S’arrêter seulement si c’est vrai, comme `i > 3`
debug-if = si { $c }
debug-remove = Retirer
debug-no-breakpoints = Aucun point d’arrêt
debug-click-left-of-a-line-number-or-press = Cliquez à gauche d’un numéro de ligne, ou appuyez sur ⌘F8 dans un éditeur, pour en poser un.
debug-condition-of = Condition de { $path }:{ $line }
debug-breakpoints-count = { $count ->
    [one] { $count } point d’arrêt
   *[other] { $count } points d’arrêt
    }
debug-breakpoints-disabled = { $breakpoints }, { $off } désactivés
debug-untick-one-to-keep-it-without-stopping-there = Décochez-en un pour le garder sans s’y arrêter. Un double clic affiche sa ligne.
debug-remove-all = Tout retirer
debug-go-to-line = Aller à la ligne
debug-an-expression-as-total-2 = Une expression, comme `total * 2`
debug-evaluate-works-while-the-program-is-stopped-at = Évaluer fonctionne quand le programme est arrêté sur un point d’arrêt : déboguez-le (⌃D) et posez-en un (⌘F8).
debug-press-to-evaluate-it-in-the-frame-the = Appuyez sur ⏎ pour l’évaluer dans le cadre où le débogueur s’est arrêté.
debug-add-it-to-the-watches-to-open-its = Ajoutez-la aux espions (⌘⏎) pour ouvrir ses champs dans la fenêtre Débogage.
debug-evaluate-expression = Évaluer l’expression
debug-java-in-the-frame-the-debugger-stopped-in = Java, dans le cadre où le débogueur s’est arrêté.
debug-add-to-watches = Ajouter aux espions
debug-evaluate = Évaluer
debug-could-not-connect-the-debugger = Impossible de connecter le débogueur : { $e }
debug-debugger-attached = Débogueur attaché
debug-the-debugger-could-not-attach = Le débogueur n’a pas pu s’attacher : { $e }
debug-nothing-to-reload = Rien à recharger
debug-reloaded = { $classes } rechargée
debug-reloaded-classes = { $n } classes rechargées
debug-could-not-reload-the-changed-classes = Impossible de recharger les classes modifiées : { $e }

## lang

lang-symbols-come-from-the-java-server-which-starts = Les symboles viennent du serveur Java, qui démarre avec un fichier Java
lang-file-structure-shows-the-symbols-of-a-java = Structure du fichier affiche les symboles d’un fichier Java
lang-classes-come-from-the-java-engine-which-starts = Les classes viennent du moteur Java, qui démarre avec un projet Gradle
lang-hierarchies-come-from-the-java-engine-in-a = Les hiérarchies viennent du moteur Java, dans un fichier Java
lang-downloading-jdk = Téléchargement du JDK { $major }… { $pct } %
lang-downloading-jdk-2 = Téléchargement du JDK { $major }…
lang-jdk-could-not-be-downloaded = Impossible de télécharger le JDK { $major } : { $error }
lang-this-project-needs-jdk-download-it = Ce projet a besoin du JDK { $major }. Le télécharger ?
lang-jdk-installed-too-new = Ce projet a besoin du JDK { $major } : son Gradle tourne au plus sur Java { $max }, et les JDK trouvés ici sont { $installed }. Télécharger le JDK { $major } ?
lang-jdk-installed-too-old = Ce projet a besoin du JDK { $major }, et les JDK trouvés ici sont plus anciens ({ $installed }). Le télécharger ?
lang-use-another-jdk = Utiliser un autre JDK…
lang-not-now = Pas maintenant
lang-downloading-node = Téléchargement de Node { $prompt }… { $pct } %
lang-downloading-node-2 = Téléchargement de Node { $prompt }…
lang-node-could-not-be-downloaded = Impossible de télécharger Node { $prompt } : { $error }
lang-this-project-needs-node-which-is-not-on = Ce projet a besoin de Node { $prompt }, absent de cette machine. Le télécharger pour SnoutCode ?
lang-installing-the-packages-of-the-build-window-shows = Installation des paquets de { $dir }… La fenêtre de compilation montre l’avancement.
lang-installing-the-projects-packages-the-build-window-shows = Installation des paquets du projet… La fenêtre de compilation montre l’avancement.
lang-installing-playwrights-browsers-the-build-window-shows-h = Installation des navigateurs de Playwright… La fenêtre de compilation montre l’avancement.
lang-the-packages-of-are-not-installed-so-it = Les paquets de { $dir } ne sont pas installés : il ne peut pas encore s’exécuter.
lang-this-playwright-projects-packages-are-not-installed-so = Les paquets de ce projet Playwright ne sont pas installés : ses tests ne peuvent pas encore s’exécuter.
lang-this-projects-packages-are-not-installed-so-it = Les paquets de ce projet ne sont pas installés : il ne peut pas encore s’exécuter.
lang-install-packages = Installer les paquets ({ $command })
lang-playwright-needs-browsers-that-are-not-downloaded = Playwright { $version } a besoin de navigateurs non téléchargés : { $join }.
lang-install-browsers = Installer les navigateurs
lang-pom-changed = Un pom.xml a changé. Recharger le projet Maven ?
lang-build-files-changed = Des fichiers de build ont changé. Synchroniser ?
lang-gradle-files-changed = Des fichiers Gradle ont changé. Synchroniser ?
lang-the-sync-failed = La synchronisation a échoué : { $e }
lang-reload = Recharger
lang-sync = Synchroniser
lang-library-classes-open-once-the-java-engine-is = Les classes de bibliothèque s’ouvriront quand le moteur Java sera prêt
lang-the-java-engine-does-not-know = Le moteur Java ne connaît pas { $qualified }

## record

record-works-in-a-playwright-project = { $what } fonctionne dans un projet Playwright
record-open-a-spec-file-and-put-the-caret = { $what } : ouvrez un fichier de spec et placez le curseur à l’endroit voulu
record-a-recording-is-going-stop-it-first = Un enregistrement est en cours : arrêtez-le d’abord
record-it-did-not-start = Le démarrage a échoué
record-the-recorder-stopped-the-build-window-says-why = L’enregistreur s’est arrêté : la fenêtre de compilation en indique la raison
record-the-recording-ended-with-no-steps = L’enregistrement s’est terminé sans étape
record-recorded-1-step-into = 1 étape enregistrée dans { $unwrap_or_default }
record-recorded-steps-into = { $n } étapes enregistrées dans { $unwrap_or_default }
record-picking-is-on-click-an-element-in-the = Sélection active : cliquez sur un élément dans le navigateur
record-pick-locator-stopped = Choisir un localisateur s’est arrêté
record-picked = { $locator } choisi
record-ui-mode-works-in-a-playwright-project = Le mode UI fonctionne dans un projet Playwright
record-ui-mode-opens-in-your-browser = Le mode UI s’ouvre dans votre navigateur

## remote

remote-could-not-connect-to = Impossible de se connecter à { $name } : { $why }
remote-trust-this-host = Faire confiance à cet hôte ?
remote-key-passphrase = Phrase secrète de la clé
remote-no = Non
remote-yes = Oui
remote-password = Mot de passe :
remote-user-host-a-host-of-your-ssh-config = utilisateur@hôte, un Host de votre config ssh, ou WSL: Ubuntu
remote-user-host-or-a-host-of-your-ssh = utilisateur@hôte, ou un Host de votre config ssh
remote-recent = Récents
remote-from-your-ssh-config = Depuis votre config ssh
remote-connecting-to = Connexion à { $h }…
remote-connecting-to-2 = Connexion à { $host }…
remote-the-first-time-takes-a-little-longer-snoutcode = La première fois prend un peu plus de temps : SnoutCode y installe son serveur.
remote-no-hosts-in-your-ssh-config-type-one = Aucun hôte dans votre config ssh : saisissez-en un.
remote-connect-to-ssh-host-or-wsl = Se connecter à un hôte SSH ou WSL
remote-connect-to-ssh-host = Se connecter à un hôte SSH
remote-snoutcode-installs-its-server-on-the-host-the = SnoutCode installe son serveur sur l’hôte la première fois (Linux, x86_64 ou aarch64).
remote-host = Hôte
remote-connect = Se connecter
remote-on = { $path } sur { $host }
remote-connected-to = Connecté à { $name }
remote-reconnecting-to = Reconnexion à { $name }…
remote-reconnecting-to-attempt = Reconnexion à { $name }… (tentative { $attempt })
remote-the-connection-to-is-lost = La connexion à { $name } est perdue
remote-disconnected-from = Déconnecté de { $name }
remote-the-linux-server-builds-are-missing-run-scripts = Les builds du serveur Linux sont absents : lancez scripts/build-server-linux.sh
remote-connecting-to-3 = Connexion à { $name }
remote-connecting = Connexion
remote-installing-snout-server = Installation de snout-server
remote-java-engine-on = Moteur Java sur { $name }
remote-looking-for-the-engine = Recherche du moteur
remote-could-not-install-it = Impossible de l’installer : { $e }
remote-open-folder-on = Ouvrir un dossier sur { $name }
remote-no-folders-here = Aucun dossier ici
remote-show-hidden-folders = Afficher les dossiers masqués

## run

run-is-not-among-the-projects-sources = { $class } ne fait pas partie des sources du projet
run-the-file-in-front-has-no-tests-main = Le fichier au premier plan n’a ni test, ni classe principale, ni script à exécuter
run-nothing-to-run-here-put-the-caret-in = Rien à exécuter ici : placez le curseur dans une classe principale, un test ou un script
run-root = { $name } (racine)
run-reading-the-gradle-tasks = Lecture des tâches Gradle
run-the-builds-are-being-configured = Les builds sont en cours de configuration.
run-the-tasks-could-not-be-read = Impossible de lire les tâches
run-no-tasks-yet = Pas encore de tâche
run-the-tasks-appear-once-the-java-engine-has = Les tâches apparaissent une fois le projet importé par le moteur Java.
run-no-gradle-tasks = Aucune tâche Gradle
run-this-project-has-no-gradle-build = Ce projet n’a pas de build Gradle.
run-maven-wrapper = Maven { $version } (wrapper)
run-lifecycle = Cycle de vie
run-plugins = Plugins
run-delete-the-build-output-target = Supprimer la sortie du build (target)
run-check-the-project-is-correct-and-complete = Vérifier que le projet est correct et complet
run-compile-the-main-sources = Compiler les sources principales
run-run-the-unit-tests = Exécuter les tests unitaires
run-package-the-compiled-code-a-jar-or-war = Empaqueter le code compilé (jar ou war)
run-run-the-checks-on-the-package-integration-tests = Exécuter les vérifications sur le paquet, dont les tests d’intégration
run-put-the-package-in-the-local-repository = Placer le paquet dans le dépôt local
run-make-the-projects-site = Générer le site du projet
run-copy-the-package-to-the-remote-repository = Copier le paquet vers le dépôt distant
run-reading-the-maven-goals = Lecture des goals Maven
run-the-plugins-goals-are-being-read = Les goals des plugins sont en cours de lecture.
run-the-goals-could-not-be-read = Impossible de lire les goals
run-no-goals-yet = Pas encore de goal
run-the-goals-appear-once-the-java-engine-has = Les goals apparaissent une fois le projet importé par le moteur Java.
run-no-maven-builds = Aucun build Maven
run-this-project-has-no-pom-xml = Ce projet n’a pas de pom.xml.
run-offline = Hors ligne
run-run-maven-offline-o-only-the-local-repository = Exécuter Maven hors ligne (-o) : dépôt local uniquement
run-skip-tests = Ignorer les tests
run-run-goals-without-the-tests-dskiptests = Exécuter les goals sans les tests (-DskipTests)
run-active-in-the-pom = actif dans le POM
run-failed = { $label } (échecs)
run-paused-running = En pause, en cours depuis { $time }
run-running = En cours depuis { $time }
run-finished-in = Terminé en { $time }
run-failed-after = Échec après { $time }
run-stopped-after = Arrêté après { $time }
run-tests-count = { $count ->
    [one] { $parts }, { $count } test
   *[other] { $parts }, { $count } tests
    }
run-the-java-engine-is-not-ready-yet = Le moteur Java n’est pas encore prêt
run-the-java-engine-stopped-and-this-run-with = Le moteur Java s’est arrêté, et cette exécution avec lui.
run-could-not-save-the-output-to = Impossible d’enregistrer la sortie dans { $path } : { $e }
run-open-a-project-to-run-it = Ouvrez un projet pour l’exécuter.
run-has-no-gradle-or-maven-build-runs-tests = { $project } n’a pas de build Gradle ou Maven : les exécutions, les tests et le débogueur en ont besoin.
run-java-support-is-off = La prise en charge de Java est désactivée.
run-the-java-engine-is-not-running = Le moteur Java ne tourne pas : { $why }
run-starting = Démarrage de { $c }
run-running-2 = Exécution de { $t }
run-running-3 = Exécution de { $config }
run-testing = Test de { $label }
run-running-4 = Exécution de { $task }
run-running-mvn-in = Exécution de mvn { $goals } dans { $module }
run-attaching-to-the-node-listening-on-port = Connexion au Node à l’écoute sur le port { $port }
run-waiting-for-the-java-engine-the-run-starts = En attente du moteur Java ({ $doing }) : l’exécution démarrera dès qu’il sera prêt.
run-working-directory = Répertoire de travail : { $cwd }
run-the-java-engine-is-ready = Le moteur Java est prêt.
run-working-directory-2 = Répertoire de travail : { $dir }
run-finished = Terminé
run-failed-3 = Échec
run-stopped = Arrêté
run-with-exit-code = {" "}avec le code de sortie { $code }
run-the-scala-compile-failed-see-problems = La compilation Scala a échoué : voir Problèmes
run-compiling-the-changed-scala = Compilation du Scala modifié
run-reading-the-tasks = Lecture des tâches
run-the-engine-does-not-know-this-files-module = Le moteur ne connaît pas encore le module de ce fichier
run-downloading-the-javascript-debugger-js-debug = Téléchargement du débogueur JavaScript (js-debug { $version })
run-the-debugger-could-not-start-the-program = Le débogueur n’a pas pu lancer le programme : { $e }
run-detached = Détaché
run-could-not-connect-the-debugger-to-a-node = Impossible de connecter le débogueur à un processus Node : { $e }
run-npm-is-missing-beside-node = npm est absent à côté de Node
run-is-not-installed-the-projects-lockfile-is-s = { $name } n’est pas installé (le lockfile du projet est celui de { $name })
run-attach-to-port = S’attacher au port { $port }
run-the-configuration-names-no-script = La configuration ne nomme aucun script
run-the-configuration-names-no-file-to-run = La configuration ne nomme aucun fichier à exécuter
run-is-not-there = { $file } est introuvable
run-open-a-project-first = Ouvrez d’abord un projet
run-no-node-fits-this-project-download-it-from = Aucun Node ne convient à ce projet : téléchargez-le d’abord depuis la barre au-dessus de l’éditeur.
run-did-not-start = { $program } n’a pas démarré : { $e }
run-process-finished-with-exit-code = Processus terminé avec le code de sortie { $c }
run-process-ended-by-a-signal = Processus terminé par un signal
run-all-tests = Tous les tests
run-tests-in = Tests dans { $p }
run-open-a-playwright-project-first = Ouvrez d’abord un projet Playwright
run-passed = { $passed ->
    [one] { $passed } réussi
   *[other] { $passed } réussis
    }
run-skipped = { $skipped ->
    [one] { $skipped } ignoré
   *[other] { $skipped } ignorés
    }
run-failed-2 = { $failed } en échec
tests-watch-label = Surveiller

## structure

structure-included-build = build inclus
structure-compile = Compilation
structure-runtime = Exécution
structure-test = Test
structure-annotation-processor = Processeur d’annotations
structure-what-the-main-code-compiles-against-compile-provided = Ce sur quoi le code principal se compile (portées compile, provided et system).
structure-what-the-main-code-runs-with-compile-and = Ce avec quoi le code principal s’exécute (portées compile et runtime).
structure-what-the-tests-compile-and-run-with-every = Ce avec quoi les tests se compilent et s’exécutent (toutes les portées).
structure-what-the-main-code-compiles-against-compileclasspath = Ce sur quoi le code principal se compile (compileClasspath).
structure-what-the-main-code-runs-with-runtimeclasspath = Ce avec quoi le code principal s’exécute (runtimeClasspath).
structure-what-the-tests-compile-and-run-with-the = Ce avec quoi les tests se compilent et s’exécutent (le runtimeClasspath du source set de test).
structure-the-annotation-processors-javac-runs-on-the-main = Les processeurs d’annotations que javac exécute sur le code principal.
structure-project = projet { $path }
structure-the-gradle-model-could-not-be-read = Impossible de lire le modèle Gradle : { $e }
structure-the-gradle-model-is-not-json = Le modèle Gradle n’est pas du JSON : { $e }
structure-the-gradle-model-is-of-an-older-format = Le modèle Gradle est d’un ancien format : synchronisez à nouveau
structure-edit-in-build-file = Modifier dans le fichier de build
structure-sync-gradle = Synchroniser Gradle
structure-the-pages-fill-in-as-soon-as-the = Les pages se remplissent dès que le modèle de la dernière synchronisation est lu.
structure-no-gradle-model-yet = Pas encore de modèle Gradle
structure-this-project-has-not-been-synced-as-a = Ce projet n’a pas été synchronisé comme build Gradle. Synchronisez-le pour voir ses modules et dépendances.
structure-the-gradle-model-could-not-be-read-2 = Impossible de lire le modèle Gradle
structure-java = Java
structure-looking-for-jdks = Recherche de JDK…
structure-not-set-the-jdks-own = Non défini (celui du JDK)
structure-java-to-per-module = Java { $lo } à { $hi }, par module
structure-language-level = Niveau de langage
structure-gradle-and-maven = Gradle et Maven
structure-unknown = Inconnu
structure-did-not-sync = Non synchronisé
structure-maven-version = Version de Maven
structure-gradle-version = Version de Gradle
structure-maven-jvm = JVM de Maven
structure-gradle-jvm = JVM de Gradle
structure-not-known-until-the-build-syncs = Inconnu tant que le build n’est pas synchronisé
structure-the-jdk-maven-runs-on-and-compiles-with = Le JDK sur lequel Maven s’exécute et compile, choisi selon le niveau Java du build : le JDK ci-dessus s’il est défini.
structure-the-jvm-the-last-sync-ran-gradle-on = La JVM utilisée pour Gradle à la dernière synchronisation : le JDK ci-dessus s’il est défini.
structure-included-build-2 = Build inclus
structure-spring-boot = Spring Boot
structure-build-file = Fichier de build
structure-not-set = Non défini
structure-toolchain = toolchain { $t }
structure-target-compatibility = Compatibilité cible
structure-this-project-compiles-no-java-it-has-no = Ce projet ne compile pas de Java : il n’a aucun source set.
structure-java-bytecode = Java { $a }, bytecode { $b }
structure-content-roots = Racines de contenu
structure-not-there-yet = pas encore présent
structure-output-folders = Dossiers de sortie
structure-resources = Ressources
structure-classes = Classes
structure-no-module-compiles-java = Aucun module ne compile de Java
structure-dependencies-show-for-the-projects-that-apply-a = Les dépendances s’affichent pour les projets qui appliquent un plugin Java.
structure-conflicts-only = Conflits uniquement
structure-no-scope-dependencies = Aucune dépendance : { $scope }
structure-declares-nothing-for-this-scope = { $m } ne déclare rien pour cette portée. { $m_2 }
structure-declared-in-all = { $declared } déclarées, { $count } au total.
structure-version-conflicts = { $count ->
    [one] { $count } conflit de versions
   *[other] { $count } conflits de versions
    }
structure-unresolved = { $unresolved_count } non résolues
structure-no-version-lost-a-conflict-here = Aucune version n’a perdu de conflit ici.
structure-try-fewer-letters-of-the-group-name-or = Essayez moins de lettres du groupe, du nom ou de la version.
structure-on = sur { $h }
structure-on-machine = sur { $machine }
structure-download-jdk = Télécharger un JDK
structure-found-where-jdks-are-installed-and-added-by = Trouvés aux emplacements d’installation des JDK { $machine }, ou ajoutés par vous.
structure-add-from-folder = Ajouter depuis un dossier…
structure-downloading-jdk = Téléchargement du JDK { $major }
structure-no-jdk-found = Aucun JDK trouvé
structure-no-jdk-is-installed-where-jdks-usually-are = Aucun JDK installé { $machine } aux emplacements habituels. Téléchargez-en un ou ajoutez son dossier.
structure-downloaded = Téléchargé
structure-detected = Détecté
structure-this-projects = Celui de ce projet
structure-remove-from-the-list = Retirer de la liste
structure-no-jdk-there-now = Plus de JDK à cet endroit
structure-removing-an-added-jdk-takes-it-off-the = Retirer un JDK ajouté l’enlève de la liste ; ses fichiers restent en place.
structure-selected-jdk = JDK sélectionné
structure-version = Version
structure-home = Emplacement
structure-kept-in = Conservé dans
structure-your-settings = Vos paramètres
structure-found-by-snoutcode = Trouvé par SnoutCode
structure-this-project = Ce projet
structure-runs-on-it = S’exécute dessus
structure-use-for-this-project = Utiliser pour ce projet
structure-not-detected = Non détecté
structure-spring-settings = Paramètres Spring
structure-none-with-the-spring-boot-plugin = Aucun avec le plugin Spring Boot
structure-in-the-spring-model = { $beans } dans le modèle Spring
structure-read-once-the-java-engine-is-ready = Lu quand le moteur Java sera prêt
structure-profiles-known = Profils connus
structure-configuration-files = Fichiers de configuration
structure-no-application-properties-or-application-yml-in-the = Pas d’application.properties ni d’application.yml dans les ressources
structure-no-spring-here = Pas de Spring ici
structure-no-module-applies-the-spring-boot-plugin-and = Aucun module n’applique le plugin Spring Boot, et le modèle Spring n’a aucun bean.
structure-not-found = Introuvable
structure-claude-code-settings = Paramètres de Claude Code
structure-claude-was-not-found-on-path-or-the = claude est introuvable dans le PATH et aux emplacements habituels
structure-ide-connection = Connexion à l’IDE
structure-no-session-connected-a-claude-started-in-the = Aucune session connectée. Un claude lancé dans la console se connecte tout seul.
structure-1-session-connected-it-sees-the-open-files = 1 session connectée : elle voit les fichiers ouverts, la sélection et les problèmes
structure-sessions-connected = { $n } sessions connectées
structure-the-project-as-a-whole-the-jdk-it = Le projet dans son ensemble : à vous de choisir son JDK, le reste relève de Maven.
structure-every-maven-module-of-every-build-with-its = Tous les modules Maven de chaque build, avec leurs source sets, racines et sorties.
structure-what-each-module-declares-and-what-that-brings = Ce que chaque module déclare et ce que cela apporte, tel que Maven l’a résolu.
structure-the-project-as-a-whole-the-jdk-it-2 = Le projet dans son ensemble : à vous de choisir son JDK, le reste relève de Gradle.
structure-every-gradle-project-of-every-build-with-its = Tous les projets Gradle de chaque build, avec leurs source sets, racines et sorties.
structure-what-each-module-declares-and-what-that-brings-2 = Ce que chaque module déclare et ce que cela apporte, tel que Gradle l’a résolu.
structure-the-jdks-snoutcode-can-run-gradle-and-the = Les JDK sur lesquels SnoutCode peut exécuter Gradle et le moteur Java.
structure-frameworks-and-tools-as-they-stand-read-only = Frameworks et outils, tels quels. Lecture seule.
structure-unresolved-2 = Non résolue
structure-show-in-modules = Afficher { $name } dans Modules
structure-show-in-build-file = Afficher dans le fichier de build
structure-show-the-declaration-that-brings-it-in = Afficher la déclaration qui l’apporte
structure-show-its-dependencies = Afficher ses dépendances
structure-a-newer-version-asked-for-elsewhere-won = une version plus récente demandée ailleurs l’a emporté
structure-dependency-management-chose-it = la gestion des dépendances l’a choisie
structure-it-is-forced = elle est imposée
structure-a-constraint-asks-for-it = une contrainte la demande
structure-another-request-won = une autre demande l’a emporté
structure-was-asked-for-here-is-used = { $asked } était demandée ici ; { $version } est utilisée : { $why }
structure-conflict = Conflit
structure-won-over = l’a emporté sur { $join }
structure-show-in-version-catalog = Afficher dans le catalogue de versions
structure-modules-from-sync = { $count ->
    [one] { $name } · { $count } module de la dernière synchronisation { $tool }
   *[other] { $name } · { $count } modules de la dernière synchronisation { $tool }
    }
structure-project-structure = Paramètres du projet
structure-syncing = Synchronisation…
structure-reload-maven = Recharger Maven
structure-read-the-maven-builds-again = Relire les builds Maven
structure-read-the-gradle-builds-again = Relire les builds Gradle
structure-read-the-builds-again = Relire les builds
structure-facets = Facettes
structure-project-settings = Paramètres du projet
structure-platform-settings = Plateforme
structure-search = Rechercher
structure-filter-dependencies = Filtrer par groupe, nom ou version
structure-jdk-listed-already = Le JDK { $version } est déjà dans la liste
structure-jdk-added = JDK { $version } ajouté. Appliquer le conserve.
structure-no-jdk-in = Aucun JDK dans { $dir } : choisissez le dossier qui contient le dossier bin avec Java
structure-add-jdk = Ajouter un JDK
structure-jdk-removed = Retiré de la liste. Appliquer conserve le changement.
structure-jdk-downloaded = JDK { $version } téléchargé dans { $home }
structure-download-failed = Le téléchargement a échoué : { $e }
structure-build-file-not-found = Fichier de build introuvable
structure-syncing-note = Synchronisation du modèle Gradle. Les pages suivront une fois terminée.

## newproject

newproject-gradle-or-maven = Gradle ou Maven
newproject-from-start-spring-io = Depuis start.spring.io
newproject-end-to-end-tests-in-typescript = Tests E2E en TypeScript
newproject-project-name = Nom du projet
newproject-folder = Dossier
newproject-search-dependencies = Rechercher des dépendances
newproject-waiting-for-start-spring-io = En attente de start.spring.io…
newproject-start-spring-io-could-not-be-reached = Impossible de joindre start.spring.io ({ $e }).
newproject-the-project-goes-in = Le projet sera créé dans { $target }
newproject-asking-start-spring-io-what-it-offers = Interrogation de start.spring.io sur ce qu’il propose…
newproject-application-url = URL de l’application
newproject-the-address-the-tests-open-a-base-url = L’adresse qu’ouvrent les tests. Une variable d’environnement BASE_URL la remplace si elle est définie.
newproject-create-a-git-repository = Créer un dépôt Git
newproject-build-tool = Outil de build
newproject-coordinates = Coordonnées
newproject-none-chosen-yet = Aucune choisie pour l’instant.
newproject-no-dependency-matches = Aucune dépendance ne correspond.
newproject-dependencies = Dépendances
newproject-spring-boot-java-choose-what-the-application-uses = Spring Boot { $version }, Java { $java }. Choisissez ce qu’utilise l’application ; d’autres dépendances pourront être ajoutées au build plus tard.
newproject-getting-the-project-from-start-spring-io = Récupération du projet depuis start.spring.io…
newproject-creating-the-project = Création du projet…
newproject-new-project = Nouveau projet
newproject-a-java-or-spring-boot-project-built-with = Un projet Java ou Spring Boot, compilé avec Gradle ou Maven.
newproject-create-without-the-network = Créer sans réseau
newproject-previous = Précédent
newproject-next = Suivant
newproject-already-exists-and-is-not-empty = { $dir } existe déjà et n’est pas vide.
newproject-give-the-project-a-name = Donnez un nom au projet.
newproject-the-name-cannot-start-with-a-dot-or = Le nom ne peut pas commencer par un point ni contenir / : ou \.
newproject-choose-where-the-project-goes = Choisissez l’emplacement du projet.
newproject-the-group-is-a-dotted-name-such-as = Le groupe est un nom pointé, comme com.example.
newproject-the-artifact-holds-letters-digits-dots-dashes-and = L’artefact contient des lettres, chiffres, points, tirets et traits de soulignement.
newproject-the-package-is-a-dotted-java-name-such = Le paquet est un nom Java pointé, comme com.example.demo.
newproject-the-applications-address-starts-with-http-or-https = L’adresse de l’application commence par http:// ou https://.

## runwin

runwin-evaluate-an-expression = Évaluer une expression
runwin-clear-all = Tout effacer
runwin-copy-all = Tout copier
runwin-fix-with = Corriger avec { $agent }
runwin-save-output-to-file = Enregistrer la sortie dans un fichier…
runwin-rerun-r = Réexécuter (⌘R)
runwin-rerun-failed-tests-r = Réexécuter les tests en échec (⌥⌘R)
runwin-test-results = Résultats des tests
runwin-passed = Réussi
runwin-skipped = Ignoré
runwin-nothing-printed = { $what } ; rien d’affiché
runwin-expected = Attendu
runwin-actual = Obtenu
runwin-errors-warnings = { $e } erreurs, { $w } avertissements
runwin-up-to-date = à jour
runwin-from-cache = depuis le cache
runwin-no-source = aucune source
runwin-debugger = Débogueur
runwin-resume-f9 = Reprendre (F9)
runwin-pause-program-f9 = Suspendre le programme (⌥⌘F9)
runwin-step-over-f8 = Pas à pas principal (F8)
runwin-step-into-f7 = Pas à pas détaillé (F7)
runwin-step-out-f8 = Pas à pas sortant (⇧F8)
runwin-the-program-ended = Le programme est terminé.
runwin-waiting-for-the-program-to-start = En attente du démarrage du programme…
runwin-the-program-is-running-breakpoints-stop-it-here = Le programme tourne. Les points d’arrêt l’arrêtent ici.
runwin-add-to-watches = Ajouter aux espions (⇧↩)
runwin-nothing-running = Rien en cours
runwin-run-a-configuration-from-the-title-bar-or = Exécutez une configuration depuis la barre de titre, ou un test depuis la gouttière.
runwin-no-debug-session = Aucune session de débogage
runwin-debug-a-configuration-from-the-title-bar-or = Déboguez une configuration depuis la barre de titre, ou un test depuis la gouttière.
runwin-no-build-output = Aucune sortie de build
runwin-build-the-project-or-run-one-of-its = Compilez le projet, ou exécutez l’une de ses tâches Gradle ou l’un de ses goals Maven.
runwin-run-task = Exécuter une tâche…

## spring

spring-any-profile = Tous les profils
spring-from-the-files = { $config_active } (selon les fichiers)
spring-text = { $name } ({ $from })
spring-consumes = {"\u000A"}Consomme { $join }
spring-produces = {"\u000A"}Produit { $join }
spring-bean-not-active-with-the-active-profiles = Bean '{ $b }' ({ $stereotype_label }) : inactif avec les profils actifs
spring-not-injected-anywhere = injecté nulle part
spring-injected-in-1-place = injecté à 1 endroit
spring-injected-in-places = injecté à { $n } endroits
spring-bean = Bean '{ $b }' ({ $stereotype_label }), { $used }
spring-text-2 = { $what } : { $value }
spring-bean-2 = { $what } : bean '{ $one }'
spring-no-bean = { $what } : aucun bean
spring-beans = { $what } : { $count } beans
spring-all-sources = Toutes les sources
spring-default-package = (paquet par défaut)
spring-search-beans-by-name-or-type = Rechercher des beans par nom ou type
spring-search-paths-and-handlers = Rechercher des chemins et handlers
spring-all-stereotypes = Tous les stéréotypes
spring-no-profile = Aucun profil
spring-profile = Profil { $p }
spring-inactive = { $inactive } inactifs
spring-by-package = Par paquet
spring-by-stereotype = Par stéréotype
spring-no-beans = Aucun bean
spring-the-project-declares-no-spring-bean-yet = Le projet ne déclare pas encore de bean Spring.
spring-no-bean-matches = Aucun bean ne correspond
spring-try-fewer-letters-or-another-stereotype-or-source = Essayez moins de lettres, ou un autre stéréotype ou source.
spring-beans-from-libraries = { $count } beans, { $library } de bibliothèques
spring-not-injected-anywhere-2 = Injecté nulle part
spring-injected-in-1-place-2 = Injecté à 1 endroit
spring-injected-in-places-2 = Injecté à { $n } endroits
spring-profiles = {"\u000A"}Profils : { $join }
spring-not-active-note = {"\u000A"}Inactif : les profils actifs l’excluent
spring-primary = Principal
spring-of-beans = { $shown } beans sur { $bean_count }
spring-method-all = Toutes
spring-method = Méthode
spring-path = Chemin
spring-handler = Handler
spring-no-endpoints = Aucun endpoint
spring-no-controller-of-the-project-maps-a-request = Aucun contrôleur du projet ne mappe encore de requête.
spring-no-endpoint-matches = Aucun endpoint ne correspond
spring-try-fewer-letters-of-the-path-or-the = Essayez moins de lettres du chemin ou de la méthode.
spring-of-endpoints = { $shown } endpoints sur { $total }
spring-as-runs = Comme '{ $c }' s’exécute ({ $join })
spring-read-the-spring-model = Lire le modèle Spring
spring-reading-the-spring-model = Lecture du modèle Spring
spring-the-beans-and-endpoints-are-being-read = Lecture des beans et endpoints en cours.
spring-no-spring-model-yet = Pas encore de modèle Spring
spring-beans-and-endpoints-appear-once-the-java-engine = Les beans et endpoints apparaissent une fois le projet importé par le moteur Java.
spring-beans-2 = Beans { $bean_count }
spring-endpoints = Endpoints { $count }
spring-reading = Lecture…
spring-data-repository = Dépôt de données

## config

config-arguments-the-program-gets = Arguments passés au programme
config-9229-empty-run-the-file = 9229 (vide : exécuter le fichier)
config-unnamed = Sans nom { $next }
config-no-configuration-add-one-with = Aucune configuration. Ajoutez-en une avec +.
config-choose-a-module = Choisir un module
config-kind = Type
config-main-class = Classe principale
config-goals = Goals
config-script = Script
config-file = Fichier
config-task = Tâche
config-gradle-arguments = Arguments Gradle
config-maven-arguments = Arguments Maven
config-playwright-arguments = Arguments Playwright
config-script-arguments = Arguments du script
config-program-arguments = Arguments du programme
config-node-options = Options de Node
config-vm-options = Options de la VM
config-environment-variables = Environnement
config-attach-to-port = S’attacher au port
config-from-the-build-with-your-changes-remove-puts = Issue du build, avec vos modifications. Retirer la remet telle que le build la définit.
config-from-the-build-a-change-you-make-stands = Issue du build. Une modification de votre part la remplace.
config-your-own-configuration = Votre propre configuration.
config-run-configurations = Configurations d’exécution
config-what-run-and-debug-start-an-application-a = Ce que lancent Exécuter et Déboguer : une application, une tâche Gradle, des tests, un script.
config-add-configuration = Ajouter une configuration
config-remove-or-put-back-as-the-build-has = Retirer, ou remettre comme dans le build
config-application = Application
config-package-script = Script de paquet
config-scripts = Scripts

## tests

tests-filter-tests = Filtrer les tests
tests-not-run-yet = Pas encore exécuté
tests-did-not-start = N’a pas démarré
tests-workers = { $count ->
    [one] { $projects } · { $count } worker
   *[other] { $projects } · { $count } workers
    }
tests-about-left = { $elapsed } · environ { $left } restantes
tests-live-run = Exécution en direct
tests-history = Historique
tests-run-with-the-browsers-showing-one-test-at = Exécuter avec les navigateurs visibles, un test à la fois
tests-rerun-failed = Relancer les échecs
tests-run-all = Tout exécuter
tests-no-environment = Aucun environnement
tests-environment = Environnement : { $label }, { $d }
tests-environment-2 = Environnement : { $label }
tests-no-environment-the-configurations-own = Aucun environnement (celui de la configuration)
tests-all = Tous { $total }
tests-failed = Échecs { $failed }
tests-flaky = Instables { $flaky }
tests-slow-over-4-s = Lents, plus de 4 s
tests-last-10-runs = 10 dernières
tests-flaky-2 = Instable
tests-finding-the-tests = Recherche des tests…
tests-no-test-matches-the-filter = Aucun test ne correspond au filtre.
tests-no-tests-yet-run-them-or-add-a = Pas encore de tests. Exécutez-les ou ajoutez un fichier de spec.
tests-failed-in = Échec dans { $project }
tests-flaky-in = Instable dans { $project }
tests-in = { $other } dans { $project }
tests-retries = { $count ->
    [one]  · { $count } nouvelle tentative
   *[other]  · { $count } nouvelles tentatives
    }
tests-screenshot-comparison = Comparaison de captures d’écran
tests-difference = Différence
tests-accept-new-screenshot = Accepter la nouvelle capture
tests-runs-the-test-again-keeping-its-screenshots-as = Réexécute le test en conservant ses captures telles quelles
tests-steps = Étapes
tests-from-the-trace = Depuis la trace
tests-console-error = erreur de console
tests-actions-no-failed-requests-or-console-errors = { $count } actions ; aucune requête en échec ni erreur de console.
tests-the-trace-could-not-be-read = Impossible de lire la trace : { $why }
tests-reading-the-trace = Lecture de la trace…
tests-open-full-trace = Ouvrir la trace complète
tests-video = Vidéo
tests-run-this-test = Exécuter ce test
tests-debug-this-test = Déboguer ce test
tests-debug-in-inspector = Déboguer dans l’inspecteur
tests-show-in-editor = Afficher dans l’éditeur
tests-copy-error = Copier l’erreur
tests-when = Quand
tests-what = Quoi
tests-branch = Branche
tests-time = Durée
tests-new-since-the-run-before = Nouveau depuis l’exécution précédente
tests-runs-of-this-project = Exécutions de ce projet
tests-kept-on-this-machine-outside-the-repository = Conservées sur cette machine, hors du dépôt
tests-no-runs-kept-yet = Aucune exécution conservée pour l’instant.
tests-min-ago = il y a { $secs } min
tests-h-ago = il y a { $secs } h
tests-days-ago = il y a { $days } jours
tests-the-playwright-test-runner-is-not-installed = Le lanceur de tests Playwright n’est pas installé.
tests-no-node-fits-this-project = Aucun Node ne convient à ce projet.
tests-trace-viewer = Visionneuse de trace
tests-no-tests-found = Aucun test trouvé
tests-open-a-playwright-project-or-a-project-whose = Ouvrez un projet Playwright, ou un projet dont les paquets testent avec Vitest ou Jest, pour exécuter ses tests ici.
tests-in-2 = { $runner } dans { $rel }
tests-is-not-installed-install-the-packages-first = { $runner } n’est pas installé : installez d’abord les paquets.
tests-the-snoutcode-reporter-could-not-be-written = Impossible d’écrire le rapporteur SnoutCode : { $e }
tests-tests = { $count } tests
tests-failed-tests = Tests en échec
tests-the-playwright-test-runner-is-not-installed-install = Le lanceur de tests Playwright n’est pas installé : installez d’abord les paquets.
tests-playwright-listed-no-tests = Playwright n’a listé aucun test ({ $status })
tests-recording = Enregistrement
tests-ui-mode = Mode UI
tests-the-snoutcode-locator-picker-could-not-be-written = Impossible d’écrire le sélecteur de localisateurs SnoutCode : { $e }
tests-new-since-the-last-run = {" "}({ $fresh } nouveaux depuis la dernière exécution)
tests-passed-failed = { $title } : { $passed } réussis, { $failed } en échec{ $new }{ $new_2 }
tests-count-passed = { $count ->
    [one] { $count } réussi
   *[other] { $count } réussis
    }
tests-count-failed = { $count ->
    [one] { $count } en échec
   *[other] { $count } en échec
    }
tests-count-flaky = { $count ->
    [one] { $count } instable
   *[other] { $count } instables
    }
tests-count-skipped = { $count ->
    [one] { $count } ignoré
   *[other] { $count } ignorés
    }
tests-flaky-3 = , { $flaky ->
    [one] { $flaky } instable
   *[other] { $flaky } instables
    }

## env

env-environment = Environnement { $count }
env-no-environment-add-one-with = Aucun environnement. Ajoutez-en un avec +.
env-variables = Variables
env-in-use = utilisé
env-where-the-tests-point-the-applications-url-and = Là où pointent les tests : l’URL de l’application et les variables qu’ils lisent.
env-add-environment = Ajouter un environnement
env-remove-environment = Retirer l’environnement

## pwsetup

pwsetup-ffmpeg-for-videos = FFmpeg (pour les vidéos)
pwsetup-playwright-not-installed = Playwright { $v }, non installé
pwsetup-no-node-fits = aucun Node ne convient
pwsetup-install-packages = Installer les paquets
pwsetup-install-playwright-browsers = Installer les navigateurs Playwright
pwsetup-no-node-fits-this-project-download-it-from = Aucun Node ne convient à ce projet : téléchargez-le d’abord depuis la barre ci-dessus.
pwsetup-the-install-did-not-finish-see-the-build = L’installation n’est pas terminée : voir la fenêtre de compilation.

## pwreport

pwreport-queued = En file d’attente
pwreport-flaky-passed-on-a-retry = Instable, réussi après une nouvelle tentative
pwreport-interrupted = Interrompu

## processes

processes-snoutcode-the-window-the-editors-and-git = SnoutCode : la fenêtre, les éditeurs et Git
processes-idle-for-min-stops-after = Inactif depuis { $m } min, s’arrête après { $limit }
processes-idle-stops-after-min = Inactif, s’arrête après { $limit } min
processes-java-engine-for-reads-the-gradle-or-maven = Moteur Java de { $project } : lit le build Gradle ou Maven, exécute et teste le code
processes-java-language-server-for-completion-errors-navigation = Serveur de langage Java de { $project } : complétion, erreurs, navigation
processes-maven-for-runs = Maven de { $project } : exécute { $title }
processes-maven-for-runs-goals = Maven de { $project } : exécute des goals
processes-tests-of-the-test-jvm-maven-started = Tests de { $title } : la JVM de test lancée par Maven
processes-tests-for-the-test-jvm-maven-started = Tests de { $project } : la JVM de test lancée par Maven
processes-your-program-started-by-maven = { $title } : votre programme, lancé par Maven
processes-program-maven-started-for = Programme lancé par Maven pour { $project }
processes-your-program-being-debugged = { $title } : votre programme, en cours de débogage
processes-your-program-started-with-run = { $title } : votre programme, lancé avec Exécuter
processes-program-started-by-the-java-engine-for = Programme lancé par le moteur Java pour { $project }
processes-gradle-daemon-for-syncs-the-build-runs-tasks = Daemon Gradle { $version } de { $project } : synchronise le build, exécute tâches et tests
processes-tests-of-the-test-jvm-gradle-started = Tests de { $title } : la JVM de test lancée par Gradle
processes-tests-for-the-test-jvm-gradle-started = Tests de { $project } : la JVM de test lancée par Gradle
processes-gradle-worker-for-compiles-or-processes-in-a = Worker Gradle de { $project } : compile ou traite dans sa propre JVM
processes-your-program-started-by-gradle = { $title } : votre programme, lancé par Gradle
processes-program-gradle-started-for = Programme lancé par Gradle pour { $project }
processes-in = {" "}dans { $p }
processes-java-engine-for-stopping-with-its-language-server = Moteur Java de { $folder } : arrêt en cours, avec son serveur de langage et ses daemons Gradle
processes-terminal-your-shell = Terminal : votre shell { $name }{ $within }
processes-claude-code-the-assistant-in-the-console = Claude Code : l’assistant dans la console
processes-started-by-snoutcode = { $name } : lancé par SnoutCode
processes-java-engine-for-left-over-from-an-earlier = Moteur Java de { $folder }, resté d’une session précédente
processes-java-language-server-left-over-from-an-earlier = Serveur de langage Java, resté d’une session précédente
processes-gradle-daemon-left-over-from-an-earlier-session = Daemon Gradle { $version }, resté d’une session précédente
processes-nothing-idle-to-stop = Rien d’inactif à arrêter.
processes-stopped-1-idle-process = 1 processus inactif arrêté.
processes-stopped-idle-processes = { $n } processus inactifs arrêtés.
processes-state-building = Compilation
processes-state-stopping = Arrêt
processes-state-left-over = Reste
processes-looking-at-the-processes = Examen des processus
processes-snoutcodes-own-processes-show-here-the-java-engine = Les processus de SnoutCode s’affichent ici : moteur Java, daemons Gradle, Maven, exécutions et terminaux.
processes-process = Processus
processes-memory = Mémoire
processes-uptime = Durée
processes-project = { $purpose }{"\u000A"}Projet : { $project }
processes-busy-at-work-now-a-filled-dot = Occupé : au travail en ce moment (point plein)
processes-waiting-alive-doing-nothing-now-a-hollow-dot = En attente : actif, sans rien faire pour le moment (point creux)
processes-summary = { $count ->
    [one] { $count } processus, { $memory } de mémoire, { $cpu } % de CPU
   *[other] { $count } processus, { $memory } de mémoire, { $cpu } % de CPU
    }
processes-idle = { $idle } inactifs

## editor

editor-tab = Tab, { $width }
editor-spaces = { $width } espaces
editor-could-not-read = Impossible de lire { $path } : { $e }
editor-could-not-save = Impossible d’enregistrer { $path } : { $e }
editor-this-file-changed-on-disk-and-this-tab = Ce fichier a changé sur le disque, et cet onglet contient des modifications non enregistrées.
editor-reload-from-disk = Recharger depuis le disque
editor-keep-my-changes = Garder mes modifications
editor-run = Exécuter '{ $name }'
editor-debug = Déboguer '{ $name }'
editor-do-you-want-to-save-the-changes-you = Voulez-vous enregistrer les modifications apportées à { $name } ?
editor-a-librarys-class-file-has-no-git-history = Le fichier de classe d’une bibliothèque n’a pas d’historique Git.
editor-not-a-git-project-no-git-repository-holds = Pas un projet Git : aucun dépôt Git ne contient ce fichier.
editor-not-committed-yet = Pas encore commité
editor-uncommitted = Non commité
editor-reading-history = Lecture de l’historique…
editor-close-annotations = Fermer les annotations
editor-breakpoint-condition = Condition du point d’arrêt…
editor-show-diff = Afficher le diff
editor-show-in-git-log = Afficher dans le journal Git
editor-copy-hash = Copier le hash
editor-copied = { $count } copié
editor-annotate-previous-revision = Annoter la révision précédente
editor-the-file-as-it-was-before-this-commit = Le fichier tel qu’il était avant ce commit
editor-this-commit-brought-the-file-in = Ce commit a introduit le fichier
editor-you-changed-this-line-since-the-last-commit = Vous avez modifié cette ligne depuis le dernier commit. Commitez-la et la colonne l’indiquera.
editor-authors-of = Auteurs de { $decl }
editor-committed-lines = { $total } lignes commitées
editor-last = { $email } · dernier { $now }
editor-1-line = 1 ligne
editor-lines = { $lines } lignes
editor-before = avant { $short_id }
editor-now = Maintenant
editor-back = Retour
editor-back-to-now = Retour au présent
editor-you-not-committed-yet = Vous, pas encore commité
editor-1-line-removed = 1 ligne supprimée
editor-lines-removed = { $n } lignes supprimées
editor-writer-editing-line = { $writer } modifie la ligne { $line } { $arrow }
editor-writer-edited-line = { $writer } a modifié la ligne { $line } { $arrow }
editor-claude-code-is-writing-a-change = Claude Code écrit une modification…
editor-claude-code-proposes-a-new-file = Claude Code propose un nouveau fichier
editor-claude-code-proposes-a-change = Claude Code propose une modification
editor-more = { $waiting } de plus
editor-reject = Rejeter
editor-accept = Accepter
editor-claudes-changes-all-reviewed = Modifications de Claude, toutes relues
editor-claude-changed-this-file-1-change-to-review = Claude a modifié ce fichier : 1 modification à relire
editor-claude-changed-this-file-changes-to-review = Claude a modifié ce fichier : { $n } modifications à relire
editor-next = ⌃⌥⇧↓ suivant
editor-revert-file = Annuler le fichier
editor-accept-file = Accepter le fichier
editor-hide-the-inline-changes = Masquer les modifications en ligne
editor-changed-by-at = Modifié par { $short } à { $t }
editor-changed-by = Modifié par { $short }
editor-lines-taken-out-here = Lignes retirées ici
editor-line = Ligne { $start }
editor-lines-2 = Lignes { $start }–{ $end }
editor-reviewed = Relu
editor-revert = Annuler
editor-ask-to-fix = Demander à { $agent } de corriger
editor-context-actions = Actions contextuelles
editor-looking-for-fixes = Recherche de correctifs…
editor-the-symbol = le symbole
editor-rename-to-enter-renames-everywhere = Renommer en (Entrée renomme partout)
editor-go-to-declaration-needs-the-java-engine-for = Aller à la déclaration a besoin du moteur Java pour ce fichier.
editor-no-declaration-found-here = Aucune déclaration trouvée ici.
editor-usages-of = Utilisations de { $word }
editor-no-usages-of-found = Aucune utilisation de { $word } trouvée.
editor-implementations-of = Implémentations de { $word }
editor-no-implementations-of-found = Aucune implémentation de { $word } trouvée.
editor-this-needs-the-java-engine-for-this-file = Il faut le moteur Java pour ce fichier.
editor-found = { $count } trouvés
editor-group-in-the-local-repository = groupe dans le dépôt local
editor-artifact-in-the-local-repository = artefact dans le dépôt local
editor-version-in-the-local-repository = version dans le dépôt local
editor-ask-to-fix-2 = Demander à { $short } de corriger
editor-more-actions = Plus d’actions…
editor-the-file-changed-meanwhile-try-fix-all-again = Le fichier a changé entre-temps : réessayez Tout corriger
editor-none-of-these-problems-could-be-fixed-this = Aucun de ces problèmes n’a pu être corrigé ainsi
editor-fixed-of-problems-the-rest-need-a-look = { $n } problèmes corrigés sur { $count } ; les autres demandent un examen
editor-no-problem-at-the-caret = Aucun problème au curseur
editor-no-quick-fix-for-this-problem-for-more = Aucun correctif rapide pour ce problème : { $more } pour plus d’actions
editor-no-actions-here = Aucune action ici
editor-recording-but-the-recorded-steps-were-edited-here = Enregistrement en cours, mais les étapes enregistrées ont été modifiées ici : la suite reste dans la fenêtre de l’enregistreur.
editor-no-steps-yet = aucune étape pour l’instant
editor-1-step = 1 étape
editor-steps = { $n } étapes
editor-recording-into-this-file-act-in-the-browser = Enregistrement dans ce fichier : { $n }. Agissez dans le navigateur, puis arrêtez ou fermez-le.
editor-picking-locators-click-an-element-in-the-browser = Sélection de localisateurs : cliquez sur un élément du navigateur pour insérer son localisateur au curseur.
editor-picking-locators-picked-click-another-or-stop = Sélection de localisateurs : { $picked } choisis. Cliquez sur un autre, ou arrêtez.
editor-cannot-extract-a-method-here-select-whole-statements = Impossible d’extraire une méthode ici : sélectionnez des instructions entières ou une expression
editor-cannot-extract-a-variable-here-select-an-expression = Impossible d’extraire une variable ici : sélectionnez une expression
editor-cannot-extract-a-constant-here-select-an-expression = Impossible d’extraire une constante ici : sélectionnez une expression de constantes, comme un littéral
editor-cannot-extract-a-field-here-select-an-expression = Impossible d’extraire un champ ici : sélectionnez une expression
editor-nothing-to-inline-here-put-the-caret-on = Rien à incorporer ici : placez le curseur sur une variable locale, une méthode ou une constante
editor-a-library-class-is-read-only = Une classe de bibliothèque est en lecture seule
editor-replace-all-occurrences = Remplacer toutes les occurrences
editor-replace-this-occurrence-only = Remplacer cette occurrence uniquement
editor-breakpoint-on-line = Point d’arrêt à la ligne { $line }
editor-filter-by-class-member-or-file = Filtrer par classe, membre ou fichier
editor-show-in-endpoints = Afficher dans Endpoints
editor-nothing-matches = Aucun résultat.
editor-of = { $matching } sur { $total }
editor-bean = Bean '{ $b }'
editor-bean-is-injected-into = Bean '{ $b }' injecté dans
editor-takes = '{ $name }' prend { $value }
editor-no-bean-for = Aucun bean pour '{ $name }'
editor-press-for-quick-fixes = { $why }. Appuyez sur ⌘. pour les correctifs rapides.
editor-could-take-any-of = '{ $name }' peut prendre l’un de
editor-takes-2 = '{ $name }' prend
editor-takes-the-bean = '{ $name }' prend le bean
editor-fix-all-in-file = Tout corriger « { $title } » dans le fichier
editor-supertypes = Supertypes
editor-subtypes = Sous-types
editor-callers = Appelants
editor-callees = Appelés
editor-put-the-caret-on-a-type-or-a = Placez le curseur sur un type ou une méthode, puis ⌃H ou ⌃⌥H.
editor-put-the-caret-on-a-class-or-an = Placez le curseur sur une classe ou une interface pour sa hiérarchie.
editor-put-the-caret-on-a-method-for-its = Placez le curseur sur une méthode pour ses appels.
editor-type-hierarchy-of = Hiérarchie des types de
editor-calls-of = Appels de
editor-reading-the-hierarchy = Lecture de la hiérarchie
editor-the-java-engine-is-looking-it-up = Le moteur Java effectue la recherche.
editor-no-hierarchy-yet = Pas encore de hiérarchie
editor-no-hierarchy = Aucune hiérarchie
editor-needs-jdk = JDK { $v } requis
editor-importing-gradle-model = Importation du modèle Gradle
editor-restarting = Redémarrage
editor-syncing-the-build-model = Synchronisation du modèle de build
editor-generating-sources = Génération des sources
editor-importing = Importation
editor-of-modules-ready = { $ready } modules prêts sur { $n }
editor-java = Java : { $summary }
editor-java-2 = Java : { $what }
editor-java-3 = Java : { $text }
editor-the-java-engine-is-not-built-run-snout = Le moteur Java n’est pas compilé (lancez snout.sh)
editor-the-java-engine-did-not-start = Le moteur Java n’a pas démarré : { $e }
editor-the-java-engine-keeps-stopping = Le moteur Java s’arrête sans cesse
editor-references-to = Références à { $name }
editor-find-references-f12-lists-them-here = Rechercher les références (⇧F12) les liste ici
editor-results = { $count ->
    [one] { $count } résultat
   *[other] { $count } résultats
    }
editor-title-results = { $title } : { $results }
editor-no-usages-found = Aucune utilisation trouvée
editor-no-usages-yet = Pas encore d’utilisations
editor-find-usages-on-a-symbol-lists-every-place = Rechercher les utilisations d’un symbole liste chaque endroit où il sert.
editor-find-usages-at-the-caret = Rechercher les utilisations au curseur
editor-filter-problems = Filtrer les problèmes
editor-show = Afficher { $what }
editor-expand-all = Tout déplier
editor-collapse-all = Tout replier
editor-no-problems = Aucun problème
editor-nothing-to-fix-in-the-files-the-java = Rien à corriger dans les fichiers vérifiés par le moteur Java.
editor-no-problems-match = Aucun problème ne correspond
editor-clear-the-filter-or-turn-on-more-of = Effacez le filtre, ou activez davantage d’erreurs, d’avertissements et d’informations ci-dessus.
editor-filter-by-name = Filtrer par nom
editor-sorted-by-name-click-for-file-order = Trié par nom (cliquez pour l’ordre du fichier)
editor-sort-by-name = Trier par nom
editor-no-structure = Aucune structure
editor-reading-the-structure = Lecture de la structure
editor-try-fewer-letters-of-the-name = Essayez moins de lettres du nom.
editor-nothing-declared = Rien de déclaré
editor-the-file-declares-no-class-field-or-method = Le fichier ne déclare aucune classe, aucun champ ni aucune méthode.
editor-package-private = privé au paquet
editor-finding-node = Recherche de Node
editor-needs-node = Node { $what } requis
editor-installing-the-language-server = Installation du serveur de langage
editor-typescript = TypeScript : { $what }
editor-or-newer = { $m } ou plus récent
editor-the-typescript-language-server-did-not-start = Le serveur de langage TypeScript n’a pas démarré : { $e }
editor-the-typescript-language-server-failed = Le serveur de langage TypeScript a échoué : { $e }
editor-the-typescript-language-server-keeps-stopping = Le serveur de langage TypeScript s’arrête sans cesse
editor-open-a-java-file = Ouvrez un fichier Java pour voir sa structure.
editor-java-engine-reading = Le moteur Java lit le fichier.
editor-problems-title = Problèmes
editor-downloading-java-for-the-engine = Téléchargement de Java { $ENGINE_JAVA } pour le moteur
editor-copying-the-java-engine = Copie du moteur Java
editor-java-engine-install-failed = Impossible d’installer le moteur Java : { $why }
editor-ts-install-failed = Impossible d’installer le serveur de langage TypeScript : { $why }

## blame

blame-month-1 = janvier
blame-month-2 = février
blame-month-3 = mars
blame-month-4 = avril
blame-month-5 = mai
blame-month-6 = juin
blame-month-7 = juillet
blame-month-8 = août
blame-month-9 = septembre
blame-month-10 = octobre
blame-month-11 = novembre
blame-month-12 = décembre
blame-long-date = { $day } { $month } { $year } à { $time }

## agents

agents-turn = Tour { $id }
agents-files = { $files } fichiers
agents-1-file = 1 fichier
agents-revert-what-this-prompt-changed = Annuler ce que ce prompt a modifié
agents-accept-what-this-prompt-changed = Accepter ce que ce prompt a modifié
agents-editing = En cours de modification
agents-waiting-for-your-review = En attente de votre relecture
agents-claude-asks-to-make-this-change = Claude demande à faire cette modification
agents-waiting-behind-another-change-to-this-file = En attente derrière une autre modification de ce fichier
agents-1-file-changed = 1 fichier modifié
agents-files-changed = { $files } fichiers modifiés
agents-revert-all = Tout annuler
agents-put-back-every-file-as-it-was-before = Remettre chaque fichier tel qu’il était avant que Claude le modifie
agents-accept-all = Tout accepter
agents-keep-every-change-the-files-leave-this-list = Garder toutes les modifications ; les fichiers quittent cette liste
agents-seen-on-disk-while-claude-code-was-connected = Vu sur le disque pendant que Claude Code était connecté ; ses hooks ne l’ont pas signalé
agents-revert-the-file = Annuler le fichier
agents-accept-the-file = Accepter le fichier
agents-lines-removed = { $count } lignes supprimées
agents-revert-this-change = Annuler cette modification
agents-accept-this-change = Accepter cette modification
agents-no-changes-from-agents-yet = Pas encore de modifications d’agents
agents-the-files-an-agent-changes-show-here-as = Les fichiers modifiés par un agent s’affichent ici au fil de son travail, chaque modification pouvant être acceptée ou annulée.
agents-open-the-agent-console = Ouvrir la console de l’agent
agents-a-new-file-the-agent-created-it-reverting = Un nouveau fichier : l’agent l’a créé ; l’annuler le supprime
agents-sent-line-to = { $name } ligne { $a } envoyée à { $agent }
agents-sent-lines-to = { $name } lignes { $a }–{ $b } envoyées à { $agent }
agents-sent-to = { $name } envoyé à { $agent }
agents-asked-to-fix-line = { $short } sollicité pour corriger { $name } ligne { $a }
agents-asked-to-fix-lines = { $short } sollicité pour corriger { $name } lignes { $a }–{ $b }
agents-snoutcode-cannot-reach-the-agent-in-this-window = SnoutCode ne parvient pas à joindre l’agent dans cette fenêtre. Fermez le projet et rouvrez-le.
agents-asked-for-the-commit-message = Message de commit demandé à { $name }
agents-asked-to-fix-the-sign-in = { $name } sollicité pour réparer la connexion de Git
agents-asked-to-fix-the-run = { $name } sollicité pour corriger { $title }
agents-claude-is-waiting-for-your-review = Claude attend votre relecture
agents-is-editing = { $short } modifie
agents-waiting = En attente
agents-done = Terminé
agents-no-calls-yet = Pas encore d’appels
agents-what-an-agent-in-the-console-asks-of = Ce qu’un agent dans la console demande à SnoutCode s’affiche ici.
agents-open-a-file-to-insert-into = Ouvrez un fichier où insérer
agents-insert = Insérer
agents-run-in-terminal = Exécuter dans le terminal
agents-nothing-shown-yet = Rien d’affiché pour l’instant
agents-what-an-agent-in-the-console-chooses-to = Ce qu’un agent dans la console choisit de vous montrer apparaît ici, avec des boutons sur son code.
agents-from = { $title }, de { $agent }
agents-from-2 = De { $agent }
agents-all = Les agents
agents-two = { $a } et { $b }
agents-n = { $n } agents
agents-changed-files = { $count ->
    [one] { $who } : 1 fichier modifié
   *[other] { $who } : { $count } fichiers modifiés
    }
agents-seen = vu
agents-new = nouveau
project-shared = partagé

## keys

keys-page-up = Page préc.
keys-page-down = Page suiv.
keys-backspace = Retour arrière

## json

json-true-or-false = true ou false
json-a-whole-number = un nombre entier
json-a-number = un nombre
json-a-string = une chaîne
json-a-list = une liste
json-an-object = un objet
json-from-to = { $only } de { $min } à { $max }
json-any-value = n’importe quelle valeur
json-or = { $join } ou { $last }
json-default = {"\u000A"}{"\u000A"}Par défaut : `{ $shown }`
json-unexpected-text-after-the-settings = Texte inattendu après les paramètres
json-the-settings = Les paramètres
json-takes = { $what } attend { $s }
json-is-at-least = { $what } vaut au moins { $min }
json-is-at-most = { $what } vaut au plus { $max }
json-unknown-setting-snoutcode-ignores-it = Paramètre inconnu `{ $key }` : SnoutCode l’ignore
json-needs = { $what } nécessite `{ $req }`
json-a-value-is-missing-here = Une valeur manque ici
json-a-key-in-double-quotes-is-expected-here = Une clé entre guillemets doubles est attendue ici
json-a-comma-or-a-closing-brace-is-expected = Une virgule ou une accolade fermante est attendue ici
json-a-comma-or-a-closing-bracket-is-expected = Une virgule ou un crochet fermant est attendu ici
json-this = ceci
json-is-not-a-json-value-strings-take-double = { $shown } n’est pas une valeur JSON (les chaînes prennent des guillemets doubles)
json-this-string-cannot-be-read = Impossible de lire cette chaîne : { $e }
json-this-string-has-no-closing-quote = Cette chaîne n’a pas de guillemet fermant
json-colon-expected = Deux-points attendus après « { $key } »

## core

core-head-to-worktree = HEAD ↔ Arbre de travail
core-index-to-worktree = Index ↔ Arbre de travail
core-head-to-index = HEAD ↔ Index
core-index = Index
core-scope-message-only = message seulement
core-scope-nothing = rien à commiter
core-what-staged = les modifications indexées
core-what-changed-file = le fichier modifié
core-what-all-changed = les { $n } fichiers modifiés
core-what-selected-file = le fichier sélectionné
core-what-selected = les { $n } fichiers sélectionnés
core-what-message = le nouveau message du dernier commit
core-what-nothing = rien
core-state-staged = { $state } (indexé)
core-state-partly-staged = { $state } (partiellement indexé)
core-state-unchanged = Inchangé
core-branch-dash = Un nom de branche ne peut pas commencer par un tiret.
core-branch-dots = Un nom de branche ne peut pas contenir « .. » ni « @{"{"} ».
core-branch-chars = Un nom de branche ne peut pas contenir ~ ^ : ? * [ ou \.
core-branch-ends = Un nom de branche ne peut pas commencer par « / » ni finir par « / », « . » ou « .lock ».
core-group-taken = Un groupe porte déjà ce nom
core-years-ago = { $count ->
    [one] il y a { $count } an
   *[other] il y a { $count } ans
    }
core-engine-built-in = Intégré
core-ghostty-unavailable = Cette version a été compilée sans Ghostty. Les versions publiées l’incluent.
core-gap-unchanged = { $count ->
    [one] ⋯ { $count } ligne inchangée
   *[other] ⋯ { $count } lignes inchangées
    }
core-update-checking = Recherche de mises à jour…
core-update-found = Mise à jour trouvée (v{ $version }), téléchargement…
core-update-preparing-now = Préparation de la mise à jour…
core-update-downloading-now = Téléchargement de la mise à jour…
core-update-ready = Mise à jour prête : v{ $version }
core-update-from-source = Cette version a été compilée depuis les sources et ne se met pas à jour seule. Recompilez-la depuis les dernières sources pour la mettre à jour.
core-update-up-to-date = Vous êtes à jour (v{ $version })
core-update-available = SnoutCode v{ $version } est disponible.
core-update-cannot-replace = SnoutCode v{ $version } est disponible, mais ne peut pas s’installer depuis cet emplacement. Déplacez-le dans vos applications ou téléchargez-le.
core-update-did-not-land = SnoutCode v{ $version } ne s’est pas installé. Téléchargez-le pour l’installer vous-même.
core-update-preparing = Préparation de la mise à jour
core-update-downloading = Téléchargement de la mise à jour
core-update-offline = Impossible de vérifier les mises à jour : cet ordinateur est hors ligne. Nouvel essai plus tard.
core-update-feed-down = Impossible de vérifier les mises à jour : GitHub ne répond pas. Votre installation n’a aucun problème ; nouvel essai plus tard.
core-update-rate-limited = Impossible de vérifier les mises à jour : GitHub limite les requêtes de ce réseau. Nouvel essai plus tard.
core-update-no-release = Impossible de vérifier les mises à jour : aucune version publiée n’a été trouvée.
core-update-signature = Une mise à jour a été téléchargée puis refusée : elle ne correspond pas à la signature attendue. Rien n’a été installé.
core-update-disk = Impossible d’enregistrer la mise à jour : l’espace disque est insuffisant.
core-update-unknown = Impossible de vérifier les mises à jour.
core-scope-staged = { $n ->
    [one] { $n } indexé
   *[other] { $n } indexés
    }
core-scope-all-changed = { $n ->
    [one] { $n } modifié
   *[other] les { $n } modifiés
    }
core-scope-selected = { $n ->
    [one] { $n } sélectionné
   *[other] { $n } sélectionnés
    }

## icons

icons-plain-text = Texte brut

## menu

menu-menu = Menu

## misc

newproject-location = Emplacement
newproject-offline-fallback = { $why } Voici les dépendances que SnoutCode connaît ; le projet est créé ici, et Gradle téléchargera Spring Boot dès qu’il y aura du réseau.
structure-modules-row = Modules
structure-jdk-hint = Gradle et le moteur Java s’exécutent dessus. Conservé dans .snout/settings.json ; le moteur l’utilisera à son prochain démarrage.
structure-locked-maven-hint = Les valeurs avec un cadenas viennent de Maven : Modifier dans le fichier de build ouvre la ligne du pom.xml qui les définit. Une fois enregistré, la barre au-dessus de l’éditeur propose de recharger.
structure-locked-gradle-hint = Les valeurs avec un cadenas viennent du build : Modifier dans le fichier de build ouvre la ligne qui les définit. Une fois enregistré, la barre au-dessus de l’éditeur propose de synchroniser.
console-installer-not-on-path = L’installation est terminée, mais { $agent } n’est pas dans le PATH vu par SnoutCode. Indiquez son chemin dans Paramètres, Agents, ou redémarrez SnoutCode.
console-no-project-open = Aucun projet ouvert : { $agent } démarre dans votre dossier personnel, où il peut en créer un. Ouvrez le projet une fois créé pour suivre ses modifications.
dialogs-group-example = Travail
dialogs-checkout-placeholder = Filtrer les branches et tags, ou saisir un commit (SHA, HEAD~2, …)
gutter-run-label = Exécuter ‘{ $label }’
gutter-debug-label = Déboguer ‘{ $label }’
editor-settings-source = Paramètres de SnoutCode
terminal-not-installed = { $name } n’est pas installé
terminal-is-mac-app = { $name } est une application Mac
terminal-not-mac-app = { $name } n’est pas une application Mac
terminal-not-windows-app = { $name } n’est pas une application Windows
terminal-default-linux = Terminal par défaut
model-could-not-open-in = Impossible d’ouvrir le projet sur { $connection }
model-no-repositories-found = Aucun dépôt trouvé
env-environments = Environnements
env-base-url-hint = Les exécutions reçoivent l’URL dans BASE_URL, lue par le baseURL de la configuration. Les variables du fichier passent d’abord, puis celles ci-dessus. Gardez les mots de passe dans le fichier .env, que le dépôt doit ignorer.
pwsetup-runner-missing = le lanceur de tests Playwright n’est pas installé
remote-password-title = Mot de passe
run-needs-jdk-note = Ce build a besoin du JDK { $v }, qui n’est pas installé. Téléchargez-le depuis la barre au-dessus de l’éditeur, ou choisissez un JDK dans Paramètres.
debug-js-download-failed = Impossible de télécharger le débogueur JavaScript : { $e }
debug-js-did-not-start = Le débogueur JavaScript n’a pas démarré : { $e }
run-ts-needs-tsx = Exécuter un fichier TypeScript demande le paquet tsx dans le projet ({ $manager } add -D tsx) ou Node 22.6 ou plus récent (ce projet utilise Node { $node }).
settings-open-project-settings = Ouvrir les paramètres du projet
settings-each-project-chooses-its-profiles = Chaque projet choisit ses profils actifs, dans Paramètres du projet.
structure-group-structure = Structure
structure-group-settings = Paramètres de ce projet
structure-download-jdk-dialog = Télécharger un JDK…
structure-looking-up-jdks = Recherche des JDK téléchargeables pour cette machine…
structure-jdks-could-not-be-looked-up = Impossible de trouver les JDK : { $e }
structure-java-lts = { $major } (support à long terme)
structure-vendor = Éditeur
structure-location = Emplacement



