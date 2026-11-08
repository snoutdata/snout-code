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
settings-blurb-gradle = Le JDK de chaque build et la durée de vie des daemons Gradle de SnoutCode.
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
cmd-project-structure = Structure du projet…
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

settings-search-settings = Rechercher dans les paramètres
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
settings-your-settings-for-every-project-a-project-can = Vos paramètres, pour tous les projets. Un projet peut en remplacer certains.
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
settings-each-builds-own-your-settings = Celui de chaque build (vos paramètres)
settings-every-build = Tous les builds
settings-jdk-per-project = JDK par projet
settings-no-gradle-build-is-open = Aucun build Gradle n’est ouvert.
settings-automatic-the-builds-java-level = Automatique (le niveau Java du build)
settings-the-java-server-restarts-on-the-chosen-jdk = Le serveur Java utilisera le JDK choisi à son prochain démarrage.
settings-gradle-daemons = Daemons Gradle
settings-stop-idle-daemons-after = Arrêter les daemons inactifs après
settings-double-click-a-task-in-the-gradle-tool = Double-cliquez sur une tâche dans la fenêtre Gradle pour l’exécuter. Sa sortie s’affiche dans la fenêtre de compilation.
settings-the-processes-window-lists-the-gradle-daemons-running = La fenêtre Processus liste les daemons Gradle en cours et arrête les inactifs à la demande.
settings-project-structure-downloads-jdks-and-sets-the-projects = Structure du projet (⌘;) télécharge des JDK et définit celui du projet.
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
settings-its-colour-caret-and-marks = Sa couleur, son curseur et ses marques
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
settings-scrollback = Historique de défilement
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
dialogs-dont-allow = Ne pas autoriser
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
welcome-java-or-spring-boot-app = Application Java ou Spring Boot
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
status-the-editor-works-meanwhile = L’éditeur reste utilisable pendant ce temps.
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
find-file-mask-java-test-java = Masque de fichier : *.java, !*Test.java
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
git-changed-staged = { $count } modifiés, { $staged_count } indexés
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
git-changed = { $count } modifiés
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

## workspace

workspace-could-not-make = Impossible de créer { $path } : { $e }
workspace-do-you-want-to-save-the-changes-you = Voulez-vous enregistrer les modifications apportées à { $one } ?
workspace-do-you-want-to-save-the-changes-you-2 = Voulez-vous enregistrer les modifications apportées à { $count } fichiers ?
workspace-your-changes-are-lost-if-you-dont-save = Vos modifications seront perdues si vous ne les enregistrez pas.
workspace-dont-save = Ne pas enregistrer
workspace-open-a-project-to-see-its-structure = Ouvrez un projet pour voir sa structure
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

## lang

lang-symbols-come-from-the-java-server-which-starts = Les symboles viennent du serveur Java, qui démarre avec un fichier Java
lang-file-structure-shows-the-symbols-of-a-java = Structure du fichier affiche les symboles d’un fichier Java
lang-classes-come-from-the-java-engine-which-starts = Les classes viennent du moteur Java, qui démarre avec un projet Gradle
lang-hierarchies-come-from-the-java-engine-in-a = Les hiérarchies viennent du moteur Java, dans un fichier Java
lang-downloading-jdk = Téléchargement du JDK { $major }… { $pct } %
lang-downloading-jdk-2 = Téléchargement du JDK { $major }…
lang-jdk-could-not-be-downloaded = Impossible de télécharger le JDK { $major } : { $error }
lang-this-project-needs-jdk-download-it = Ce projet a besoin du JDK { $major }. Le télécharger ?
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

## run

run-is-not-among-the-projects-sources = { $class } ne fait pas partie des sources du projet
run-the-file-in-front-has-no-tests-main = Le fichier au premier plan n’a ni test, ni classe principale, ni script à exécuter
run-nothing-to-run-here-put-the-caret-in = Rien à exécuter ici : placez le curseur dans une classe principale, un test ou un script
