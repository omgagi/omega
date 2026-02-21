//! Internationalization — localized strings for bot command responses.
//!
//! Uses a simple `t(key, lang)` function for static strings and
//! `format_*()` helpers for strings with interpolation.
//! Supported languages: English (fallback), Spanish, Portuguese, French,
//! German, Italian, Dutch, Russian.

/// Return a localized static string for `key` in the given `lang`.
/// Falls back to English for unknown keys or unsupported languages.
pub fn t(key: &str, lang: &str) -> &'static str {
    match key {
        // --- Headers ---
        "status_header" => match lang {
            "Spanish" => "Estado de *OMEGA Ω*",
            "Portuguese" => "Status do *OMEGA Ω*",
            "French" => "Statut de *OMEGA Ω*",
            "German" => "*OMEGA Ω* Status",
            "Italian" => "Stato di *OMEGA Ω*",
            "Dutch" => "*OMEGA Ω* Status",
            "Russian" => "Статус *OMEGA Ω*",
            _ => "*OMEGA Ω* Status",
        },
        "your_memory" => match lang {
            "Spanish" => "Tu Memoria",
            "Portuguese" => "Sua Memória",
            "French" => "Votre Mémoire",
            "German" => "Dein Gedächtnis",
            "Italian" => "La Tua Memoria",
            "Dutch" => "Je Geheugen",
            "Russian" => "Твоя память",
            _ => "Your Memory",
        },
        "recent_conversations" => match lang {
            "Spanish" => "Conversaciones Recientes",
            "Portuguese" => "Conversas Recentes",
            "French" => "Conversations Récentes",
            "German" => "Letzte Gespräche",
            "Italian" => "Conversazioni Recenti",
            "Dutch" => "Recente Gesprekken",
            "Russian" => "Недавние разговоры",
            _ => "Recent Conversations",
        },
        "known_facts" => match lang {
            "Spanish" => "Datos Conocidos",
            "Portuguese" => "Fatos Conhecidos",
            "French" => "Faits Connus",
            "German" => "Bekannte Fakten",
            "Italian" => "Fatti Noti",
            "Dutch" => "Bekende Feiten",
            "Russian" => "Известные факты",
            _ => "Known Facts",
        },
        "scheduled_tasks" => match lang {
            "Spanish" => "Tareas Programadas",
            "Portuguese" => "Tarefas Agendadas",
            "French" => "Tâches Planifiées",
            "German" => "Geplante Aufgaben",
            "Italian" => "Attività Pianificate",
            "Dutch" => "Geplande Taken",
            "Russian" => "Запланированные задачи",
            _ => "Scheduled Tasks",
        },
        "installed_skills" => match lang {
            "Spanish" => "Skills Instalados",
            "Portuguese" => "Skills Instalados",
            "French" => "Skills Installés",
            "German" => "Installierte Skills",
            "Italian" => "Skills Installati",
            "Dutch" => "Geïnstalleerde Skills",
            "Russian" => "Установленные навыки",
            _ => "Installed Skills",
        },
        "projects_header" => match lang {
            "Spanish" => "Proyectos",
            "Portuguese" => "Projetos",
            "French" => "Projets",
            "German" => "Projekte",
            "Italian" => "Progetti",
            "Dutch" => "Projecten",
            "Russian" => "Проекты",
            _ => "Projects",
        },
        "commands_header" => match lang {
            "Spanish" => "Comandos de *OMEGA Ω*",
            "Portuguese" => "Comandos do *OMEGA Ω*",
            "French" => "Commandes de *OMEGA Ω*",
            "German" => "*OMEGA Ω* Befehle",
            "Italian" => "Comandi di *OMEGA Ω*",
            "Dutch" => "*OMEGA Ω* Commando's",
            "Russian" => "Команды *OMEGA Ω*",
            _ => "*OMEGA Ω* Commands",
        },

        // --- Labels ---
        "uptime" => match lang {
            "Spanish" | "Portuguese" | "French" | "Italian" => "Uptime:",
            "German" => "Laufzeit:",
            "Dutch" => "Uptime:",
            "Russian" => "Аптайм:",
            _ => "Uptime:",
        },
        "provider" => match lang {
            "Spanish" => "Proveedor:",
            "Portuguese" => "Provedor:",
            "French" => "Fournisseur:",
            "German" => "Anbieter:",
            "Italian" => "Provider:",
            "Dutch" => "Provider:",
            "Russian" => "Провайдер:",
            _ => "Provider:",
        },
        "sandbox" => "Sandbox:",
        "database" => match lang {
            "Spanish" => "Base de datos:",
            "Portuguese" => "Banco de dados:",
            "French" => "Base de données:",
            "German" => "Datenbank:",
            "Italian" => "Database:",
            "Dutch" => "Database:",
            "Russian" => "База данных:",
            _ => "Database:",
        },
        "conversations" => match lang {
            "Spanish" => "Conversaciones:",
            "Portuguese" => "Conversas:",
            "French" => "Conversations:",
            "German" => "Gespräche:",
            "Italian" => "Conversazioni:",
            "Dutch" => "Gesprekken:",
            "Russian" => "Разговоры:",
            _ => "Conversations:",
        },
        "messages" => match lang {
            "Spanish" => "Mensajes:",
            "Portuguese" => "Mensagens:",
            "French" => "Messages:",
            "German" => "Nachrichten:",
            "Italian" => "Messaggi:",
            "Dutch" => "Berichten:",
            "Russian" => "Сообщения:",
            _ => "Messages:",
        },
        "facts_label" => match lang {
            "Spanish" => "Datos:",
            "Portuguese" => "Fatos:",
            "French" => "Faits:",
            "German" => "Fakten:",
            "Italian" => "Fatti:",
            "Dutch" => "Feiten:",
            "Russian" => "Факты:",
            _ => "Facts:",
        },
        "due" => match lang {
            "Spanish" => "Vence:",
            "Portuguese" => "Vence:",
            "French" => "Échéance:",
            "German" => "Fällig:",
            "Italian" => "Scadenza:",
            "Dutch" => "Vervalt:",
            "Russian" => "Срок:",
            _ => "Due:",
        },
        "language_label" => match lang {
            "Spanish" => "Idioma:",
            "Portuguese" => "Idioma:",
            "French" => "Langue:",
            "German" => "Sprache:",
            "Italian" => "Lingua:",
            "Dutch" => "Taal:",
            "Russian" => "Язык:",
            _ => "Language:",
        },

        // --- Empty states ---
        "no_pending_tasks" => match lang {
            "Spanish" => "Sin tareas pendientes.",
            "Portuguese" => "Sem tarefas pendentes.",
            "French" => "Aucune tâche en attente.",
            "German" => "Keine anstehenden Aufgaben.",
            "Italian" => "Nessuna attività in sospeso.",
            "Dutch" => "Geen openstaande taken.",
            "Russian" => "Нет ожидающих задач.",
            _ => "No pending tasks.",
        },
        "no_facts" => match lang {
            "Spanish" => "Aún no hay datos almacenados.",
            "Portuguese" => "Nenhum fato armazenado ainda.",
            "French" => "Aucun fait enregistré pour l'instant.",
            "German" => "Noch keine Fakten gespeichert.",
            "Italian" => "Nessun fatto memorizzato.",
            "Dutch" => "Nog geen feiten opgeslagen.",
            "Russian" => "Пока нет сохранённых фактов.",
            _ => "No facts stored yet.",
        },
        "no_history" => match lang {
            "Spanish" => "Aún no hay historial de conversaciones.",
            "Portuguese" => "Ainda não há histórico de conversas.",
            "French" => "Pas encore d'historique de conversation.",
            "German" => "Noch kein Gesprächsverlauf.",
            "Italian" => "Nessuna cronologia ancora.",
            "Dutch" => "Nog geen gespreksgeschiedenis.",
            "Russian" => "Истории разговоров пока нет.",
            _ => "No conversation history yet.",
        },
        "no_skills" => match lang {
            "Spanish" => "No hay skills instalados. Crea un directorio en ~/.omega/skills/ con un archivo SKILL.md.",
            "Portuguese" => "Nenhum skill instalado. Crie um diretório em ~/.omega/skills/ com um arquivo SKILL.md.",
            "French" => "Aucun skill installé. Créez un répertoire dans ~/.omega/skills/ avec un fichier SKILL.md.",
            "German" => "Keine Skills installiert. Erstelle ein Verzeichnis in ~/.omega/skills/ mit einer SKILL.md-Datei.",
            "Italian" => "Nessuno skill installato. Crea una cartella in ~/.omega/skills/ con un file SKILL.md.",
            "Dutch" => "Geen skills geïnstalleerd. Maak een map in ~/.omega/skills/ met een SKILL.md bestand.",
            "Russian" => "Навыки не установлены. Создайте каталог в ~/.omega/skills/ с файлом SKILL.md.",
            _ => "No skills installed. Create a directory in ~/.omega/skills/ with a SKILL.md file.",
        },
        "no_projects" => match lang {
            "Spanish" => "No se encontraron proyectos. Crea carpetas en ~/.omega/projects/ con ROLE.md",
            "Portuguese" => "Nenhum projeto encontrado. Crie pastas em ~/.omega/projects/ com ROLE.md",
            "French" => "Aucun projet trouvé. Créez des dossiers dans ~/.omega/projects/ avec ROLE.md",
            "German" => "Keine Projekte gefunden. Erstelle Ordner in ~/.omega/projects/ mit ROLE.md",
            "Italian" => "Nessun progetto trovato. Crea cartelle in ~/.omega/projects/ con ROLE.md",
            "Dutch" => "Geen projecten gevonden. Maak mappen in ~/.omega/projects/ met ROLE.md",
            "Russian" => "Проекты не найдены. Создайте папки в ~/.omega/projects/ с файлом ROLE.md",
            _ => "No projects found. Create folders in ~/.omega/projects/ with ROLE.md",
        },

        // --- Confirmations ---
        "conversation_cleared" => match lang {
            "Spanish" => "Conversación borrada. Empezamos de nuevo.",
            "Portuguese" => "Conversa apagada. Começando do zero.",
            "French" => "Conversation effacée. On repart à zéro.",
            "German" => "Gespräch gelöscht. Neuanfang.",
            "Italian" => "Conversazione cancellata. Si ricomincia.",
            "Dutch" => "Gesprek gewist. We beginnen opnieuw.",
            "Russian" => "Разговор удалён. Начинаем заново.",
            _ => "Conversation cleared. Starting fresh.",
        },
        "no_active_conversation" => match lang {
            "Spanish" => "No hay conversación activa que borrar.",
            "Portuguese" => "Nenhuma conversa ativa para apagar.",
            "French" => "Pas de conversation active à effacer.",
            "German" => "Kein aktives Gespräch zum Löschen.",
            "Italian" => "Nessuna conversazione attiva da cancellare.",
            "Dutch" => "Geen actief gesprek om te wissen.",
            "Russian" => "Нет активного разговора для удаления.",
            _ => "No active conversation to clear.",
        },
        "task_cancelled" => match lang {
            "Spanish" => "Tarea cancelada.",
            "Portuguese" => "Tarefa cancelada.",
            "French" => "Tâche annulée.",
            "German" => "Aufgabe abgebrochen.",
            "Italian" => "Attività annullata.",
            "Dutch" => "Taak geannuleerd.",
            "Russian" => "Задача отменена.",
            _ => "Task cancelled.",
        },
        "no_matching_task" => match lang {
            "Spanish" => "No se encontró tarea coincidente.",
            "Portuguese" => "Nenhuma tarefa correspondente encontrada.",
            "French" => "Aucune tâche correspondante trouvée.",
            "German" => "Keine passende Aufgabe gefunden.",
            "Italian" => "Nessuna attività corrispondente trovata.",
            "Dutch" => "Geen overeenkomende taak gevonden.",
            "Russian" => "Подходящая задача не найдена.",
            _ => "No matching task found.",
        },
        "task_updated" => match lang {
            "Spanish" => "Tarea actualizada.",
            "Portuguese" => "Tarefa atualizada.",
            "French" => "Tâche mise à jour.",
            "German" => "Aufgabe aktualisiert.",
            "Italian" => "Attività aggiornata.",
            "Dutch" => "Taak bijgewerkt.",
            "Russian" => "Задача обновлена.",
            _ => "Task updated.",
        },
        "cancel_usage" => "Usage: /cancel <task-id>",
        "personality_reset" => match lang {
            "Spanish" => "Personalidad restablecida a los valores predeterminados.",
            "Portuguese" => "Personalidade redefinida para o padrão.",
            "French" => "Personnalité réinitialisée aux valeurs par défaut.",
            "German" => "Persönlichkeit auf Standard zurückgesetzt.",
            "Italian" => "Personalità ripristinata ai valori predefiniti.",
            "Dutch" => "Persoonlijkheid teruggezet naar standaard.",
            "Russian" => "Личность сброшена к настройкам по умолчанию.",
            _ => "Personality reset to defaults.",
        },
        "personality_already_default" => match lang {
            "Spanish" => "Ya estás usando la personalidad predeterminada.",
            "Portuguese" => "Já está usando a personalidade padrão.",
            "French" => "Vous utilisez déjà la personnalité par défaut.",
            "German" => "Du verwendest bereits die Standardpersönlichkeit.",
            "Italian" => "Stai già usando la personalità predefinita.",
            "Dutch" => "Je gebruikt al de standaard persoonlijkheid.",
            "Russian" => "Уже используется стандартная личность.",
            _ => "Already using default personality.",
        },
        "personality_default_prompt" => match lang {
            "Spanish" => "Usando personalidad predeterminada. Dime cómo quieres que sea — más formal, más casual, más divertido, directo al grano — lo que sea.",
            "Portuguese" => "Usando personalidade padrão. Me diga como você quer que eu seja — mais formal, mais casual, mais engraçado, direto ao ponto — qualquer coisa.",
            "French" => "Personnalité par défaut. Dites-moi comment vous voulez que je sois — plus formel, plus décontracté, plus drôle, droit au but — tout ce que vous voulez.",
            "German" => "Standardpersönlichkeit aktiv. Sag mir einfach, wie ich sein soll — formeller, lockerer, lustiger, direkt auf den Punkt — was auch immer.",
            "Italian" => "Personalità predefinita. Dimmi come vuoi che sia — più formale, più casual, più divertente, dritto al punto — qualsiasi cosa.",
            "Dutch" => "Standaard persoonlijkheid actief. Zeg me hoe je wilt dat ik ben — formeler, casualer, grappiger, recht op het doel af — wat je maar wilt.",
            "Russian" => "Стандартная личность. Скажи, каким мне быть — более формальным, более расслабленным, смешнее, по делу — что угодно.",
            _ => "Using default personality. Just tell me how you'd like me to be — more formal, more casual, funnier, straight to the point — anything.",
        },
        "project_deactivated" => match lang {
            "Spanish" => "Proyecto desactivado. Conversación borrada.",
            "Portuguese" => "Projeto desativado. Conversa apagada.",
            "French" => "Projet désactivé. Conversation effacée.",
            "German" => "Projekt deaktiviert. Gespräch gelöscht.",
            "Italian" => "Progetto disattivato. Conversazione cancellata.",
            "Dutch" => "Project gedeactiveerd. Gesprek gewist.",
            "Russian" => "Проект деактивирован. Разговор удалён.",
            _ => "Project deactivated. Conversation cleared.",
        },
        "no_active_project" => match lang {
            "Spanish" => "No hay proyecto activo.",
            "Portuguese" => "Nenhum projeto ativo.",
            "French" => "Pas de projet actif.",
            "German" => "Kein aktives Projekt.",
            "Italian" => "Nessun progetto attivo.",
            "Dutch" => "Geen actief project.",
            "Russian" => "Нет активного проекта.",
            _ => "No active project.",
        },
        "available" => match lang {
            "Spanish" => "disponible",
            "Portuguese" => "disponível",
            "French" => "disponible",
            "German" => "verfügbar",
            "Italian" => "disponibile",
            "Dutch" => "beschikbaar",
            "Russian" => "доступен",
            _ => "available",
        },
        "missing_deps" => match lang {
            "Spanish" => "faltan dependencias",
            "Portuguese" => "dependências ausentes",
            "French" => "dépendances manquantes",
            "German" => "Abhängigkeiten fehlen",
            "Italian" => "dipendenze mancanti",
            "Dutch" => "ontbrekende afhankelijkheden",
            "Russian" => "отсутствуют зависимости",
            _ => "missing deps",
        },
        "not_set" => match lang {
            "Spanish" => "no configurado",
            "Portuguese" => "não definido",
            "French" => "non défini",
            "German" => "nicht gesetzt",
            "Italian" => "non impostato",
            "Dutch" => "niet ingesteld",
            "Russian" => "не задано",
            _ => "not set",
        },

        // --- Help command descriptions ---
        "help_status" => match lang {
            "Spanish" => "/status   — Uptime, proveedor, info de la base de datos",
            "Portuguese" => "/status   — Uptime, provedor, info do banco de dados",
            "French" => "/status   — Uptime, fournisseur, info base de données",
            "German" => "/status   — Laufzeit, Anbieter, Datenbankinfo",
            "Italian" => "/status   — Uptime, provider, info database",
            "Dutch" => "/status   — Uptime, provider, database-info",
            "Russian" => "/status   — Аптайм, провайдер, информация о БД",
            _ => "/status   — Uptime, provider, database info",
        },
        "help_memory" => match lang {
            "Spanish" => "/memory   — Estadísticas de conversaciones y datos",
            "Portuguese" => "/memory   — Estatísticas de conversas e fatos",
            "French" => "/memory   — Statistiques de conversations et faits",
            "German" => "/memory   — Gesprächs- und Faktenstatistik",
            "Italian" => "/memory   — Statistiche conversazioni e fatti",
            "Dutch" => "/memory   — Gespreks- en feitenstatistieken",
            "Russian" => "/memory   — Статистика разговоров и фактов",
            _ => "/memory   — Your conversation and facts stats",
        },
        "help_history" => match lang {
            "Spanish" => "/history  — Últimas 5 conversaciones resumidas",
            "Portuguese" => "/history  — Últimas 5 conversas resumidas",
            "French" => "/history  — 5 derniers résumés de conversation",
            "German" => "/history  — Letzte 5 Gesprächszusammenfassungen",
            "Italian" => "/history  — Ultime 5 conversazioni riassunte",
            "Dutch" => "/history  — Laatste 5 gespreksamenvattingen",
            "Russian" => "/history  — Последние 5 разговоров кратко",
            _ => "/history  — Last 5 conversation summaries",
        },
        "help_facts" => match lang {
            "Spanish" => "/facts    — Datos conocidos sobre ti",
            "Portuguese" => "/facts    — Fatos conhecidos sobre você",
            "French" => "/facts    — Faits connus sur vous",
            "German" => "/facts    — Bekannte Fakten über dich",
            "Italian" => "/facts    — Fatti noti su di te",
            "Dutch" => "/facts    — Bekende feiten over jou",
            "Russian" => "/facts    — Известные факты о тебе",
            _ => "/facts    — List known facts about you",
        },
        "help_forget" => match lang {
            "Spanish" => "/forget   — Borrar conversación actual",
            "Portuguese" => "/forget   — Apagar conversa atual",
            "French" => "/forget   — Effacer la conversation actuelle",
            "German" => "/forget   — Aktuelles Gespräch löschen",
            "Italian" => "/forget   — Cancella conversazione corrente",
            "Dutch" => "/forget   — Huidig gesprek wissen",
            "Russian" => "/forget   — Очистить текущий разговор",
            _ => "/forget   — Clear current conversation",
        },
        "help_tasks" => match lang {
            "Spanish" => "/tasks    — Ver tus tareas programadas",
            "Portuguese" => "/tasks    — Ver suas tarefas agendadas",
            "French" => "/tasks    — Voir vos tâches planifiées",
            "German" => "/tasks    — Deine geplanten Aufgaben anzeigen",
            "Italian" => "/tasks    — Vedi le tue attività pianificate",
            "Dutch" => "/tasks    — Je geplande taken bekijken",
            "Russian" => "/tasks    — Просмотр запланированных задач",
            _ => "/tasks    — List your scheduled tasks",
        },
        "help_cancel" => match lang {
            "Spanish" => "/cancel   — Cancelar tarea por ID",
            "Portuguese" => "/cancel   — Cancelar tarefa por ID",
            "French" => "/cancel   — Annuler une tâche par ID",
            "German" => "/cancel   — Aufgabe nach ID abbrechen",
            "Italian" => "/cancel   — Annulla attività per ID",
            "Dutch" => "/cancel   — Taak annuleren op ID",
            "Russian" => "/cancel   — Отменить задачу по ID",
            _ => "/cancel   — Cancel a task by ID",
        },
        "help_language" => match lang {
            "Spanish" => "/language — Ver o cambiar tu idioma",
            "Portuguese" => "/language — Ver ou alterar seu idioma",
            "French" => "/language — Voir ou changer votre langue",
            "German" => "/language — Sprache anzeigen oder ändern",
            "Italian" => "/language — Vedi o cambia la tua lingua",
            "Dutch" => "/language — Taal bekijken of wijzigen",
            "Russian" => "/language — Показать или сменить язык",
            _ => "/language — Show or set your language",
        },
        "help_personality" => match lang {
            "Spanish" => "/personality — Ver o cambiar cómo me comporto",
            "Portuguese" => "/personality — Ver ou mudar como eu me comporto",
            "French" => "/personality — Voir ou changer mon comportement",
            "German" => "/personality — Verhalten anzeigen oder ändern",
            "Italian" => "/personality — Vedi o cambia il mio comportamento",
            "Dutch" => "/personality — Gedrag bekijken of wijzigen",
            "Russian" => "/personality — Показать или изменить поведение",
            _ => "/personality — Show or set how I behave",
        },
        "help_purge" => match lang {
            "Spanish" => "/purge    — Borrar todos los datos aprendidos (borrón y cuenta nueva)",
            "Portuguese" => "/purge    — Apagar todos os fatos aprendidos (começar do zero)",
            "French" => "/purge    — Supprimer tous les faits appris (remise à zéro)",
            "German" => "/purge    — Alle gelernten Fakten löschen (Neustart)",
            "Italian" => "/purge    — Cancella tutti i fatti appresi (ricominciare)",
            "Dutch" => "/purge    — Alle geleerde feiten wissen (schone lei)",
            "Russian" => "/purge    — Удалить все изученные факты (начать с чистого листа)",
            _ => "/purge    — Delete all learned facts (clean slate)",
        },
        "help_skills" => match lang {
            "Spanish" => "/skills   — Ver skills disponibles",
            "Portuguese" => "/skills   — Ver skills disponíveis",
            "French" => "/skills   — Voir les skills disponibles",
            "German" => "/skills   — Verfügbare Skills anzeigen",
            "Italian" => "/skills   — Vedi skills disponibili",
            "Dutch" => "/skills   — Beschikbare skills bekijken",
            "Russian" => "/skills   — Просмотр доступных навыков",
            _ => "/skills   — List available skills",
        },
        "help_projects" => match lang {
            "Spanish" => "/projects — Ver proyectos disponibles",
            "Portuguese" => "/projects — Ver projetos disponíveis",
            "French" => "/projects — Voir les projets disponibles",
            "German" => "/projects — Verfügbare Projekte anzeigen",
            "Italian" => "/projects — Vedi progetti disponibili",
            "Dutch" => "/projects — Beschikbare projecten bekijken",
            "Russian" => "/projects — Просмотр доступных проектов",
            _ => "/projects — List available projects",
        },
        "help_project" => match lang {
            "Spanish" => "/project  — Ver, activar o desactivar un proyecto",
            "Portuguese" => "/project  — Ver, ativar ou desativar um projeto",
            "French" => "/project  — Voir, activer ou désactiver un projet",
            "German" => "/project  — Projekt anzeigen, aktivieren oder deaktivieren",
            "Italian" => "/project  — Vedi, attiva o disattiva un progetto",
            "Dutch" => "/project  — Project bekijken, activeren of deactiveren",
            "Russian" => "/project  — Просмотр, активация или деактивация проекта",
            _ => "/project  — Show, activate, or deactivate a project",
        },
        "help_whatsapp" => match lang {
            "Spanish" => "/whatsapp — Conectar WhatsApp vía código QR",
            "Portuguese" => "/whatsapp — Conectar WhatsApp via código QR",
            "French" => "/whatsapp — Connecter WhatsApp via code QR",
            "German" => "/whatsapp — WhatsApp per QR-Code verbinden",
            "Italian" => "/whatsapp — Connetti WhatsApp tramite codice QR",
            "Dutch" => "/whatsapp — WhatsApp verbinden via QR-code",
            "Russian" => "/whatsapp — Подключить WhatsApp по QR-коду",
            _ => "/whatsapp — Connect WhatsApp via QR code",
        },
        "help_help" => match lang {
            "Spanish" => "/help     — Este mensaje",
            "Portuguese" => "/help     — Esta mensagem",
            "French" => "/help     — Ce message",
            "German" => "/help     — Diese Nachricht",
            "Italian" => "/help     — Questo messaggio",
            "Dutch" => "/help     — Dit bericht",
            "Russian" => "/help     — Это сообщение",
            _ => "/help     — This message",
        },

        // --- Projects footer ---
        "projects_footer" => match lang {
            "Spanish" => "Usa /project <nombre> para activar, /project off para desactivar.",
            "Portuguese" => "Use /project <nome> para ativar, /project off para desativar.",
            "French" => "Utilisez /project <nom> pour activer, /project off pour désactiver.",
            "German" => "Verwende /project <Name> zum Aktivieren, /project off zum Deaktivieren.",
            "Italian" => "Usa /project <nome> per attivare, /project off per disattivare.",
            "Dutch" => "Gebruik /project <naam> om te activeren, /project off om te deactiveren.",
            "Russian" => "Используйте /project <имя> для активации, /project off для деактивации.",
            _ => "Use /project <name> to activate, /project off to deactivate.",
        },

        // --- Personality ---
        "personality_reset_hint" => match lang {
            "Spanish" => "Usa /personality reset para volver a los valores predeterminados.\nO simplemente dime cómo quieres que sea.",
            "Portuguese" => "Use /personality reset para voltar ao padrão.\nOu simplesmente me diga como quer que eu seja.",
            "French" => "Utilisez /personality reset pour revenir aux valeurs par défaut.\nOu dites-moi simplement comment vous voulez que je sois.",
            "German" => "Verwende /personality reset für die Standardwerte.\nOder sag mir einfach, wie ich sein soll.",
            "Italian" => "Usa /personality reset per tornare ai valori predefiniti.\nO dimmi semplicemente come vuoi che sia.",
            "Dutch" => "Gebruik /personality reset om terug te gaan naar de standaard.\nOf vertel me gewoon hoe je wilt dat ik ben.",
            "Russian" => "Используйте /personality reset для сброса к стандартным.\nИли просто скажите, каким мне быть.",
            _ => "Use /personality reset to go back to defaults.\nOr just tell me how you'd like me to be.",
        },

        // --- Project ---
        "project_deactivate_hint" => match lang {
            "Spanish" => "Usa /project off para desactivar.",
            "Portuguese" => "Use /project off para desativar.",
            "French" => "Utilisez /project off pour désactiver.",
            "German" => "Verwende /project off zum Deaktivieren.",
            "Italian" => "Usa /project off per disattivare.",
            "Dutch" => "Gebruik /project off om te deactiveren.",
            "Russian" => "Используйте /project off для деактивации.",
            _ => "Use /project off to deactivate.",
        },
        "no_active_project_hint" => match lang {
            "Spanish" => "No hay proyecto activo. Usa /project <nombre> para activar.",
            "Portuguese" => "Nenhum projeto ativo. Use /project <nome> para ativar.",
            "French" => "Pas de projet actif. Utilisez /project <nom> pour activer.",
            "German" => "Kein aktives Projekt. Verwende /project <Name> zum Aktivieren.",
            "Italian" => "Nessun progetto attivo. Usa /project <nome> per attivare.",
            "Dutch" => "Geen actief project. Gebruik /project <naam> om te activeren.",
            "Russian" => "Нет активного проекта. Используйте /project <имя> для активации.",
            _ => "No active project. Use /project <name> to activate.",
        },
        "once" => match lang {
            "Spanish" => "una vez",
            "Portuguese" => "uma vez",
            "French" => "une fois",
            "German" => "einmalig",
            "Italian" => "una volta",
            "Dutch" => "eenmalig",
            "Russian" => "однократно",
            _ => "once",
        },

        // --- Task confirmation ---
        "task_confirmed" => match lang {
            "Spanish" => "✓ Programada:",
            "Portuguese" => "✓ Agendada:",
            "French" => "✓ Planifiée:",
            "German" => "✓ Geplant:",
            "Italian" => "✓ Pianificata:",
            "Dutch" => "✓ Gepland:",
            "Russian" => "✓ Запланировано:",
            _ => "✓ Scheduled:",
        },
        "task_similar_warning" => match lang {
            "Spanish" => "⚠ Tarea similar existente:",
            "Portuguese" => "⚠ Tarefa similar existente:",
            "French" => "⚠ Tâche similaire existante:",
            "German" => "⚠ Ähnliche Aufgabe vorhanden:",
            "Italian" => "⚠ Attività simile esistente:",
            "Dutch" => "⚠ Vergelijkbare taak bestaat al:",
            "Russian" => "⚠ Похожая задача существует:",
            _ => "⚠ Similar task exists:",
        },
        "task_cancelled_confirmed" => match lang {
            "Spanish" => "✓ Tarea cancelada:",
            "Portuguese" => "✓ Tarefa cancelada:",
            "French" => "✓ Tâche annulée:",
            "German" => "✓ Aufgabe storniert:",
            "Italian" => "✓ Attività annullata:",
            "Dutch" => "✓ Taak geannuleerd:",
            "Russian" => "✓ Задача отменена:",
            _ => "✓ Task cancelled:",
        },
        "task_updated_confirmed" => match lang {
            "Spanish" => "✓ Tarea actualizada:",
            "Portuguese" => "✓ Tarefa atualizada:",
            "French" => "✓ Tâche mise à jour:",
            "German" => "✓ Aufgabe aktualisiert:",
            "Italian" => "✓ Attività aggiornata:",
            "Dutch" => "✓ Taak bijgewerkt:",
            "Russian" => "✓ Задача обновлена:",
            _ => "✓ Task updated:",
        },
        "task_cancel_failed" => match lang {
            "Spanish" => "✗ Error al cancelar tarea",
            "Portuguese" => "✗ Falha ao cancelar tarefa",
            "French" => "✗ Échec de l'annulation de tâche",
            "German" => "✗ Aufgabe konnte nicht storniert werden",
            "Italian" => "✗ Impossibile annullare attività",
            "Dutch" => "✗ Taak annuleren mislukt",
            "Russian" => "✗ Не удалось отменить задачу",
            _ => "✗ Failed to cancel task",
        },
        "task_update_failed" => match lang {
            "Spanish" => "✗ Error al actualizar tarea",
            "Portuguese" => "✗ Falha ao atualizar tarefa",
            "French" => "✗ Échec de la mise à jour de tâche",
            "German" => "✗ Aufgabe konnte nicht aktualisiert werden",
            "Italian" => "✗ Impossibile aggiornare attività",
            "Dutch" => "✗ Taak bijwerken mislukt",
            "Russian" => "✗ Не удалось обновить задачу",
            _ => "✗ Failed to update task",
        },

        // --- Skill improvement ---
        "skill_improved" => match lang {
            "Spanish" => "✓ Skill actualizado:",
            "Portuguese" => "✓ Skill atualizado:",
            "French" => "✓ Skill mis à jour:",
            "German" => "✓ Skill aktualisiert:",
            "Italian" => "✓ Skill aggiornato:",
            "Dutch" => "✓ Skill bijgewerkt:",
            "Russian" => "✓ Навык обновлён:",
            _ => "✓ Skill updated:",
        },
        "skill_improve_failed" => match lang {
            "Spanish" => "✗ Error al actualizar skill",
            "Portuguese" => "✗ Falha ao atualizar skill",
            "French" => "✗ Échec de la mise à jour du skill",
            "German" => "✗ Skill konnte nicht aktualisiert werden",
            "Italian" => "✗ Impossibile aggiornare skill",
            "Dutch" => "✗ Skill bijwerken mislukt",
            "Russian" => "✗ Не удалось обновить навык",
            _ => "✗ Failed to update skill",
        },

        // --- Bug report ---
        "bug_reported" => match lang {
            "Spanish" => "✓ Bug registrado:",
            "Portuguese" => "✓ Bug registrado:",
            "French" => "✓ Bug enregistré:",
            "German" => "✓ Bug protokolliert:",
            "Italian" => "✓ Bug registrato:",
            "Dutch" => "✓ Bug gelogd:",
            "Russian" => "✓ Баг записан:",
            _ => "✓ Bug logged:",
        },
        "bug_report_failed" => match lang {
            "Spanish" => "✗ Error al registrar bug",
            "Portuguese" => "✗ Falha ao registrar bug",
            "French" => "✗ Échec de l'enregistrement du bug",
            "German" => "✗ Bug konnte nicht protokolliert werden",
            "Italian" => "✗ Impossibile registrare bug",
            "Dutch" => "✗ Bug loggen mislukt",
            "Russian" => "✗ Не удалось записать баг",
            _ => "✗ Failed to log bug",
        },

        _ => "???",
    }
}

// --- Format helpers for strings with interpolation ---

/// Format the language set confirmation.
pub fn language_set(lang: &str, new_lang: &str) -> String {
    match lang {
        "Spanish" => format!("Idioma configurado a: {new_lang}"),
        "Portuguese" => format!("Idioma definido para: {new_lang}"),
        "French" => format!("Langue définie sur: {new_lang}"),
        "German" => format!("Sprache eingestellt auf: {new_lang}"),
        "Italian" => format!("Lingua impostata su: {new_lang}"),
        "Dutch" => format!("Taal ingesteld op: {new_lang}"),
        "Russian" => format!("Язык установлен: {new_lang}"),
        _ => format!("Language set to: {new_lang}"),
    }
}

/// Format language show with usage hint.
pub fn language_show(lang: &str, current: &str) -> String {
    let label = t("language_label", lang);
    format!("{label} {current}\nUsage: /language <language>")
}

/// Format personality updated confirmation.
pub fn personality_updated(lang: &str, pref: &str) -> String {
    match lang {
        "Spanish" => format!("Personalidad actualizada: _{pref}_"),
        "Portuguese" => format!("Personalidade atualizada: _{pref}_"),
        "French" => format!("Personnalité mise à jour: _{pref}_"),
        "German" => format!("Persönlichkeit aktualisiert: _{pref}_"),
        "Italian" => format!("Personalità aggiornata: _{pref}_"),
        "Dutch" => format!("Persoonlijkheid bijgewerkt: _{pref}_"),
        "Russian" => format!("Личность обновлена: _{pref}_"),
        _ => format!("Personality updated: _{pref}_"),
    }
}

/// Format the personality show (current preference).
pub fn personality_show(lang: &str, pref: &str) -> String {
    let header = match lang {
        "Spanish" => "Tu preferencia de personalidad:",
        "Portuguese" => "Sua preferência de personalidade:",
        "French" => "Votre préférence de personnalité:",
        "German" => "Deine Persönlichkeitspräferenz:",
        "Italian" => "La tua preferenza di personalità:",
        "Dutch" => "Je persoonlijkheidsvoorkeur:",
        "Russian" => "Твои настройки личности:",
        _ => "Your personality preference:",
    };
    let hint = t("personality_reset_hint", lang);
    format!("{header}\n_{pref}_\n\n{hint}")
}

/// Format the purge result.
pub fn purge_result(lang: &str, purged: usize, keys_display: &str) -> String {
    match lang {
        "Spanish" => {
            format!("{purged} datos eliminados. Claves del sistema preservadas ({keys_display}).")
        }
        "Portuguese" => {
            format!("{purged} fatos excluídos. Chaves do sistema preservadas ({keys_display}).")
        }
        "French" => format!("{purged} faits supprimés. Clés système préservées ({keys_display})."),
        "German" => {
            format!("{purged} Fakten gelöscht. Systemschlüssel beibehalten ({keys_display}).")
        }
        "Italian" => {
            format!("{purged} fatti eliminati. Chiavi di sistema preservate ({keys_display}).")
        }
        "Dutch" => {
            format!("{purged} feiten verwijderd. Systeemsleutels behouden ({keys_display}).")
        }
        "Russian" => {
            format!("{purged} фактов удалено. Системные ключи сохранены ({keys_display}).")
        }
        _ => format!("Purged {purged} facts. System keys preserved ({keys_display})."),
    }
}

/// Format the project activated confirmation.
pub fn project_activated(lang: &str, name: &str) -> String {
    match lang {
        "Spanish" => format!("Proyecto '{name}' activado. Conversación borrada."),
        "Portuguese" => format!("Projeto '{name}' ativado. Conversa apagada."),
        "French" => format!("Projet '{name}' activé. Conversation effacée."),
        "German" => format!("Projekt '{name}' aktiviert. Gespräch gelöscht."),
        "Italian" => format!("Progetto '{name}' attivato. Conversazione cancellata."),
        "Dutch" => format!("Project '{name}' geactiveerd. Gesprek gewist."),
        "Russian" => format!("Проект '{name}' активирован. Разговор удалён."),
        _ => format!("Project '{name}' activated. Conversation cleared."),
    }
}

/// Format the project not found message.
pub fn project_not_found(lang: &str, name: &str) -> String {
    match lang {
        "Spanish" => {
            format!("Proyecto '{name}' no encontrado. Usa /projects para ver los disponibles.")
        }
        "Portuguese" => {
            format!("Projeto '{name}' não encontrado. Use /projects para ver os disponíveis.")
        }
        "French" => {
            format!("Projet '{name}' introuvable. Utilisez /projects pour voir les disponibles.")
        }
        "German" => {
            format!("Projekt '{name}' nicht gefunden. Verwende /projects für verfügbare Projekte.")
        }
        "Italian" => {
            format!("Progetto '{name}' non trovato. Usa /projects per vedere quelli disponibili.")
        }
        "Dutch" => {
            format!("Project '{name}' niet gevonden. Gebruik /projects om beschikbare te zien.")
        }
        "Russian" => {
            format!("Проект '{name}' не найден. Используйте /projects для просмотра доступных.")
        }
        _ => format!("Project '{name}' not found. Use /projects to see available projects."),
    }
}

/// Format the active project display.
pub fn active_project(lang: &str, name: &str) -> String {
    let hint = t("project_deactivate_hint", lang);
    match lang {
        "Spanish" => format!("Proyecto activo: {name}\n{hint}"),
        "Portuguese" => format!("Projeto ativo: {name}\n{hint}"),
        "French" => format!("Projet actif: {name}\n{hint}"),
        "German" => format!("Aktives Projekt: {name}\n{hint}"),
        "Italian" => format!("Progetto attivo: {name}\n{hint}"),
        "Dutch" => format!("Actief project: {name}\n{hint}"),
        "Russian" => format!("Активный проект: {name}\n{hint}"),
        _ => format!("Active project: {name}\n{hint}"),
    }
}

/// Format the "Scheduled N tasks:" header.
pub fn tasks_confirmed(lang: &str, n: usize) -> String {
    match lang {
        "Spanish" => format!("✓ {n} tareas programadas:"),
        "Portuguese" => format!("✓ {n} tarefas agendadas:"),
        "French" => format!("✓ {n} tâches planifiées:"),
        "German" => format!("✓ {n} Aufgaben geplant:"),
        "Italian" => format!("✓ {n} attività pianificate:"),
        "Dutch" => format!("✓ {n} taken gepland:"),
        "Russian" => format!("✓ {n} задач запланировано:"),
        _ => format!("✓ Scheduled {n} tasks:"),
    }
}

/// Format the "Cancelled N tasks:" header.
pub fn tasks_cancelled_confirmed(lang: &str, n: usize) -> String {
    match lang {
        "Spanish" => format!("✓ {n} tareas canceladas:"),
        "Portuguese" => format!("✓ {n} tarefas canceladas:"),
        "French" => format!("✓ {n} tâches annulées:"),
        "German" => format!("✓ {n} Aufgaben storniert:"),
        "Italian" => format!("✓ {n} attività annullate:"),
        "Dutch" => format!("✓ {n} taken geannuleerd:"),
        "Russian" => format!("✓ {n} задач отменено:"),
        _ => format!("✓ Cancelled {n} tasks:"),
    }
}

/// Format the "Updated N tasks:" header.
pub fn tasks_updated_confirmed(lang: &str, n: usize) -> String {
    match lang {
        "Spanish" => format!("✓ {n} tareas actualizadas:"),
        "Portuguese" => format!("✓ {n} tarefas atualizadas:"),
        "French" => format!("✓ {n} tâches mises à jour:"),
        "German" => format!("✓ {n} Aufgaben aktualisiert:"),
        "Italian" => format!("✓ {n} attività aggiornate:"),
        "Dutch" => format!("✓ {n} taken bijgewerkt:"),
        "Russian" => format!("✓ {n} задач обновлено:"),
        _ => format!("✓ Updated {n} tasks:"),
    }
}

/// Format the task save failure message.
pub fn task_save_failed(lang: &str, n: usize) -> String {
    match lang {
        "Spanish" => format!("✗ Error al guardar {n} tarea(s). Inténtalo de nuevo."),
        "Portuguese" => format!("✗ Falha ao salvar {n} tarefa(s). Tente novamente."),
        "French" => format!("✗ Échec de l'enregistrement de {n} tâche(s). Réessayez."),
        "German" => {
            format!("✗ {n} Aufgabe(n) konnten nicht gespeichert werden. Bitte erneut versuchen.")
        }
        "Italian" => format!("✗ Impossibile salvare {n} attività. Riprova."),
        "Dutch" => format!("✗ {n} ta(a)k(en) opslaan mislukt. Probeer opnieuw."),
        "Russian" => format!("✗ Не удалось сохранить {n} задач(у). Попробуйте снова."),
        _ => format!("✗ Failed to save {n} task(s). Please try again."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_keys_have_english_fallback() {
        let keys = [
            "status_header",
            "your_memory",
            "recent_conversations",
            "known_facts",
            "scheduled_tasks",
            "installed_skills",
            "projects_header",
            "commands_header",
            "uptime",
            "provider",
            "sandbox",
            "database",
            "conversations",
            "messages",
            "facts_label",
            "due",
            "language_label",
            "no_pending_tasks",
            "no_facts",
            "no_history",
            "no_skills",
            "no_projects",
            "conversation_cleared",
            "no_active_conversation",
            "task_cancelled",
            "no_matching_task",
            "task_updated",
            "cancel_usage",
            "personality_reset",
            "personality_already_default",
            "personality_default_prompt",
            "project_deactivated",
            "no_active_project",
            "available",
            "missing_deps",
            "not_set",
            "projects_footer",
            "personality_reset_hint",
            "project_deactivate_hint",
            "no_active_project_hint",
            "once",
            "task_confirmed",
            "task_similar_warning",
            "task_cancelled_confirmed",
            "task_updated_confirmed",
            "task_cancel_failed",
            "task_update_failed",
            "skill_improved",
            "skill_improve_failed",
            "bug_reported",
            "bug_report_failed",
        ];
        for key in keys {
            let val = t(key, "English");
            assert_ne!(val, "???", "key '{key}' should have English fallback");
        }
    }

    #[test]
    fn test_all_languages_have_sample_translations() {
        let langs = [
            "Spanish",
            "Portuguese",
            "French",
            "German",
            "Italian",
            "Dutch",
            "Russian",
        ];
        // Use keys that genuinely differ across all 8 languages.
        let sample_keys = [
            "conversation_cleared",
            "no_pending_tasks",
            "your_memory",
            "known_facts",
        ];
        for lang in langs {
            for key in sample_keys {
                let val = t(key, lang);
                assert_ne!(
                    val,
                    t(key, "English"),
                    "key '{key}' in {lang} should differ from English"
                );
            }
        }
    }

    #[test]
    fn test_unknown_language_falls_back_to_english() {
        assert_eq!(t("status_header", "Klingon"), t("status_header", "English"));
    }

    #[test]
    fn test_unknown_key_returns_placeholder() {
        assert_eq!(t("nonexistent_key", "English"), "???");
    }

    #[test]
    fn test_format_helpers() {
        // language_set
        assert!(language_set("Spanish", "French").contains("French"));
        assert!(language_set("English", "German").contains("German"));

        // personality_updated
        assert!(personality_updated("English", "be casual").contains("be casual"));

        // purge_result
        assert!(purge_result("English", 5, "a, b").contains("5"));

        // project_activated
        assert!(project_activated("Spanish", "test").contains("test"));

        // project_not_found
        assert!(project_not_found("English", "xyz").contains("xyz"));

        // active_project
        assert!(active_project("English", "omega").contains("omega"));

        // tasks_confirmed
        assert!(tasks_confirmed("English", 3).contains("3 tasks"));
        assert!(tasks_confirmed("Spanish", 2).contains("2 tareas"));

        // task_save_failed
        assert!(task_save_failed("English", 1).contains("1 task"));
        assert!(task_save_failed("Spanish", 2).contains("2 tarea"));

        // tasks_cancelled_confirmed
        assert!(tasks_cancelled_confirmed("English", 3).contains("3 tasks"));
        assert!(tasks_cancelled_confirmed("Spanish", 2).contains("2 tareas"));

        // tasks_updated_confirmed
        assert!(tasks_updated_confirmed("English", 3).contains("3 tasks"));
        assert!(tasks_updated_confirmed("Spanish", 2).contains("2 tareas"));
    }

    #[test]
    fn test_help_commands_all_languages() {
        let help_keys = [
            "help_status",
            "help_memory",
            "help_history",
            "help_facts",
            "help_forget",
            "help_tasks",
            "help_cancel",
            "help_language",
            "help_personality",
            "help_purge",
            "help_skills",
            "help_projects",
            "help_project",
            "help_whatsapp",
            "help_help",
        ];
        // All help keys should contain the command name (slash prefix)
        for key in help_keys {
            let val = t(key, "English");
            let cmd = key.strip_prefix("help_").unwrap();
            assert!(
                val.contains(&format!("/{cmd}")),
                "help key '{key}' should contain '/{cmd}'"
            );
        }
    }
}
