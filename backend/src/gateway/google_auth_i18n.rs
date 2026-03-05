//! Localized messages for the `/google` credential setup flow.
//!
//! Extracted into its own file to respect the 500-line-per-file rule.
//! All functions are `pub(super)` -- consumed by `google_auth.rs`.

use super::google_auth_oauth::{gcp_api_library_url, gcp_console_url};

/// Step 1: Welcome + ask for Project ID.
/// Includes overwrite warning if `existing` is true.
pub(super) fn google_step_project_id_message(lang: &str, existing: bool) -> String {
    let warning = if existing {
        match lang {
            "Spanish" => "\n\n*Nota:* Ya tienes credenciales de Google configuradas. Este proceso las sobrescribira.",
            "Portuguese" => "\n\n*Nota:* Voce ja tem credenciais do Google configuradas. Este processo as substituira.",
            "French" => "\n\n*Note :* Vous avez deja des identifiants Google configures. Ce processus les ecrasera.",
            "German" => "\n\n*Hinweis:* Du hast bereits Google-Zugangsdaten konfiguriert. Dieser Vorgang wird sie uberschreiben.",
            "Italian" => "\n\n*Nota:* Hai gia credenziali Google configurate. Questo processo le sovrascrivera.",
            "Dutch" => "\n\n*Let op:* Je hebt al Google-inloggegevens geconfigureerd. Dit proces zal ze overschrijven.",
            "Russian" => "\n\n*Примечание:* У вас уже настроены учетные данные Google. Этот процесс их перезапишет.",
            _ => "\n\n*Note:* You already have Google credentials configured. This process will overwrite them.",
        }
    } else {
        ""
    };

    let base = match lang {
        "Spanish" => "Configuracion de cuenta Google\n\nEste proceso es 100% privado entre tu y Omega. Tus credenciales de Google se guardan localmente en tu dispositivo y *nunca* se envian al proveedor de IA.\n\n1. Ve a https://console.cloud.google.com\n2. Crea un nuevo proyecto (o usa uno existente)\n3. Envia tu *Project ID*\n\nPuedes escribir *cancel* en cualquier momento para cancelar.",
        "Portuguese" => "Configuracao de conta Google\n\nEste processo e 100% privado entre voce e o Omega. Suas credenciais do Google sao salvas localmente no seu dispositivo e *nunca* sao enviadas ao provedor de IA.\n\n1. Va a https://console.cloud.google.com\n2. Crie um novo projeto (ou use um existente)\n3. Envie seu *Project ID*\n\nVoce pode escrever *cancel* a qualquer momento para cancelar.",
        "French" => "Configuration du compte Google\n\nCe processus est 100% prive entre vous et Omega. Vos identifiants Google sont stockes localement sur votre appareil et ne sont *jamais* envoyes au fournisseur d'IA.\n\n1. Allez a https://console.cloud.google.com\n2. Creez un nouveau projet (ou utilisez un existant)\n3. Envoyez votre *Project ID*\n\nVous pouvez ecrire *cancel* a tout moment pour annuler.",
        "German" => "Google-Konto einrichten\n\nDieser Vorgang ist 100% privat zwischen dir und Omega. Deine Google-Zugangsdaten werden lokal auf deinem Gerat gespeichert und *niemals* an den KI-Anbieter gesendet.\n\n1. Gehe zu https://console.cloud.google.com\n2. Erstelle ein neues Projekt (oder verwende ein vorhandenes)\n3. Sende deine *Project ID*\n\nDu kannst jederzeit *cancel* schreiben, um abzubrechen.",
        "Italian" => "Configurazione account Google\n\nQuesto processo e 100% privato tra te e Omega. Le tue credenziali Google vengono salvate localmente sul tuo dispositivo e non vengono *mai* inviate al fornitore di IA.\n\n1. Vai a https://console.cloud.google.com\n2. Crea un nuovo progetto (o usa uno esistente)\n3. Invia il tuo *Project ID*\n\nPuoi scrivere *cancel* in qualsiasi momento per annullare.",
        "Dutch" => "Google-account instellen\n\nDit proces is 100% prive tussen jou en Omega. Je Google-inloggegevens worden lokaal op je apparaat opgeslagen en worden *nooit* naar de AI-provider gestuurd.\n\n1. Ga naar https://console.cloud.google.com\n2. Maak een nieuw project (of gebruik een bestaand)\n3. Stuur je *Project ID*\n\nJe kunt op elk moment *cancel* typen om te annuleren.",
        "Russian" => "Настройка аккаунта Google\n\nЭтот процесс на 100% приватен между вами и Omega. Ваши учетные данные Google хранятся локально на вашем устройстве и *никогда* не отправляются провайдеру ИИ.\n\n1. Перейдите на https://console.cloud.google.com\n2. Создайте новый проект (или используйте существующий)\n3. Отправьте ваш *Project ID*\n\nВы можете написать *cancel* в любой момент для отмены.",
        _ => "Google Account Setup\n\nThis process is 100% private between you and Omega. Your Google credentials are stored locally on your device and are *never* sent to the AI provider.\n\n1. Go to https://console.cloud.google.com\n2. Create a new project (or use an existing one)\n3. Send your *Project ID*\n\nYou can type *cancel* at any time to abort.",
    };

    format!("{base}{warning}")
}

/// Step 2: Comprehensive setup guide with project-specific links.
pub(super) fn google_step_setup_guide_message(lang: &str, project_id: &str) -> String {
    let gmail_url = gcp_api_library_url(project_id, "gmail.googleapis.com");
    let calendar_url = gcp_api_library_url(project_id, "calendar-json.googleapis.com");
    let drive_url = gcp_api_library_url(project_id, "drive.googleapis.com");
    let docs_url = gcp_api_library_url(project_id, "docs.googleapis.com");
    let sheets_url = gcp_api_library_url(project_id, "sheets.googleapis.com");
    let consent_url = gcp_console_url(project_id, "apis/credentials/consent");
    let cred_url = gcp_console_url(project_id, "apis/credentials/oauthclient");

    let guide = match lang {
        "Spanish" => format!(
            "Proyecto recibido: *{project_id}*\n\n\
             Sigue estos pasos:\n\n\
             *1. Habilitar APIs* (haz clic en cada enlace y activa):\n\
             - Gmail: {gmail_url}\n\
             - Calendar: {calendar_url}\n\
             - Drive: {drive_url}\n\
             - Docs: {docs_url}\n\
             - Sheets: {sheets_url}\n\n\
             *2. Pantalla de consentimiento OAuth*\n\
             {consent_url}\n\
             - Haz clic en \"Get Started\"\n\
             - Nombre: omega | Email: tu email\n\
             - Audiencia: External | Crea\n\n\
             *3. Crear credenciales OAuth*\n\
             {cred_url}\n\
             - Tipo: Web application\n\
             - URI de redireccion: https://omgagi.ai/oauth/callback/\n\
             - Crea y copia el Client ID y el Client Secret\n\n\
             *4. Publicar la app*\n\
             {consent_url}\n\
             - Ve a \"Audience\" y haz clic en \"Publish App\"\n\n\
             Pega el contenido completo del archivo JSON descargado cuando estes listo."
        ),
        "Portuguese" => format!(
            "Projeto recebido: *{project_id}*\n\n\
             Siga estes passos:\n\n\
             *1. Habilitar APIs* (clique em cada link e ative):\n\
             - Gmail: {gmail_url}\n\
             - Calendar: {calendar_url}\n\
             - Drive: {drive_url}\n\
             - Docs: {docs_url}\n\
             - Sheets: {sheets_url}\n\n\
             *2. Tela de consentimento OAuth*\n\
             {consent_url}\n\
             - Clique em \"Get Started\"\n\
             - Nome: omega | Email: seu email\n\
             - Audiencia: External | Crie\n\n\
             *3. Criar credenciais OAuth*\n\
             {cred_url}\n\
             - Tipo: Web application\n\
             - URI de redirecionamento: https://omgagi.ai/oauth/callback/\n\
             - Crie e copie o Client ID e o Client Secret\n\n\
             *4. Publicar o app*\n\
             {consent_url}\n\
             - Va a \"Audience\" e clique em \"Publish App\"\n\n\
             Cole o conteudo completo do arquivo JSON baixado quando estiver pronto."
        ),
        "French" => format!(
            "Projet recu : *{project_id}*\n\n\
             Suivez ces etapes :\n\n\
             *1. Activer les APIs* (cliquez sur chaque lien et activez) :\n\
             - Gmail : {gmail_url}\n\
             - Calendar : {calendar_url}\n\
             - Drive : {drive_url}\n\
             - Docs : {docs_url}\n\
             - Sheets : {sheets_url}\n\n\
             *2. Ecran de consentement OAuth*\n\
             {consent_url}\n\
             - Cliquez sur \"Get Started\"\n\
             - Nom : omega | Email : votre email\n\
             - Audience : External | Creez\n\n\
             *3. Creer des identifiants OAuth*\n\
             {cred_url}\n\
             - Type : Web application\n\
             - URI de redirection : https://omgagi.ai/oauth/callback/\n\
             - Creez et copiez le Client ID et le Client Secret\n\n\
             *4. Publier l'app*\n\
             {consent_url}\n\
             - Allez a \"Audience\" et cliquez sur \"Publish App\"\n\n\
             Collez le contenu complet du fichier JSON telecharge quand vous etes pret."
        ),
        "German" => format!(
            "Projekt erhalten: *{project_id}*\n\n\
             Folge diesen Schritten:\n\n\
             *1. APIs aktivieren* (klicke auf jeden Link und aktiviere):\n\
             - Gmail: {gmail_url}\n\
             - Calendar: {calendar_url}\n\
             - Drive: {drive_url}\n\
             - Docs: {docs_url}\n\
             - Sheets: {sheets_url}\n\n\
             *2. OAuth-Zustimmungsbildschirm*\n\
             {consent_url}\n\
             - Klicke auf \"Get Started\"\n\
             - Name: omega | E-Mail: deine E-Mail\n\
             - Zielgruppe: External | Erstellen\n\n\
             *3. OAuth-Zugangsdaten erstellen*\n\
             {cred_url}\n\
             - Typ: Web application\n\
             - Weiterleitungs-URI: https://omgagi.ai/oauth/callback/\n\
             - Erstelle und kopiere die Client ID und das Client Secret\n\n\
             *4. App veroffentlichen*\n\
             {consent_url}\n\
             - Gehe zu \"Audience\" und klicke auf \"Publish App\"\n\n\
             Fuge den vollstandigen Inhalt der heruntergeladenen JSON-Datei ein, wenn du bereit bist."
        ),
        "Italian" => format!(
            "Progetto ricevuto: *{project_id}*\n\n\
             Segui questi passaggi:\n\n\
             *1. Abilitare le API* (clicca su ogni link e attiva):\n\
             - Gmail: {gmail_url}\n\
             - Calendar: {calendar_url}\n\
             - Drive: {drive_url}\n\
             - Docs: {docs_url}\n\
             - Sheets: {sheets_url}\n\n\
             *2. Schermata di consenso OAuth*\n\
             {consent_url}\n\
             - Clicca su \"Get Started\"\n\
             - Nome: omega | Email: la tua email\n\
             - Pubblico: External | Crea\n\n\
             *3. Creare credenziali OAuth*\n\
             {cred_url}\n\
             - Tipo: Web application\n\
             - URI di reindirizzamento: https://omgagi.ai/oauth/callback/\n\
             - Crea e copia il Client ID e il Client Secret\n\n\
             *4. Pubblicare l'app*\n\
             {consent_url}\n\
             - Vai a \"Audience\" e clicca su \"Publish App\"\n\n\
             Incolla il contenuto completo del file JSON scaricato quando sei pronto."
        ),
        "Dutch" => format!(
            "Project ontvangen: *{project_id}*\n\n\
             Volg deze stappen:\n\n\
             *1. API's inschakelen* (klik op elke link en activeer):\n\
             - Gmail: {gmail_url}\n\
             - Calendar: {calendar_url}\n\
             - Drive: {drive_url}\n\
             - Docs: {docs_url}\n\
             - Sheets: {sheets_url}\n\n\
             *2. OAuth-toestemmingsscherm*\n\
             {consent_url}\n\
             - Klik op \"Get Started\"\n\
             - Naam: omega | E-mail: je e-mail\n\
             - Doelgroep: External | Maken\n\n\
             *3. OAuth-inloggegevens maken*\n\
             {cred_url}\n\
             - Type: Web application\n\
             - Omleidings-URI: https://omgagi.ai/oauth/callback/\n\
             - Maak en kopieer het Client ID en het Client Secret\n\n\
             *4. App publiceren*\n\
             {consent_url}\n\
             - Ga naar \"Audience\" en klik op \"Publish App\"\n\n\
             Plak de volledige inhoud van het gedownloade JSON-bestand wanneer je klaar bent."
        ),
        "Russian" => format!(
            "Проект получен: *{project_id}*\n\n\
             Выполните следующие шаги:\n\n\
             *1. Включить API* (нажмите на каждую ссылку и активируйте):\n\
             - Gmail: {gmail_url}\n\
             - Calendar: {calendar_url}\n\
             - Drive: {drive_url}\n\
             - Docs: {docs_url}\n\
             - Sheets: {sheets_url}\n\n\
             *2. Экран согласия OAuth*\n\
             {consent_url}\n\
             - Нажмите \"Get Started\"\n\
             - Название: omega | Email: ваш email\n\
             - Аудитория: External | Создать\n\n\
             *3. Создать учетные данные OAuth*\n\
             {cred_url}\n\
             - Тип: Web application\n\
             - URI перенаправления: https://omgagi.ai/oauth/callback/\n\
             - Создайте и скопируйте Client ID и Client Secret\n\n\
             *4. Опубликовать приложение*\n\
             {consent_url}\n\
             - Перейдите в \"Audience\" и нажмите \"Publish App\"\n\n\
             Вставьте полное содержимое скачанного JSON-файла, когда будете готовы."
        ),
        _ => format!(
            "Project received: *{project_id}*\n\n\
             Follow these steps:\n\n\
             *1. Enable APIs* (click each link and enable):\n\
             - Gmail: {gmail_url}\n\
             - Calendar: {calendar_url}\n\
             - Drive: {drive_url}\n\
             - Docs: {docs_url}\n\
             - Sheets: {sheets_url}\n\n\
             *2. OAuth consent screen*\n\
             {consent_url}\n\
             - Click \"Get Started\"\n\
             - Name: omega | Email: your email\n\
             - Audience: External | Create\n\n\
             *3. Create OAuth credentials*\n\
             {cred_url}\n\
             - Type: Web application\n\
             - Redirect URI: https://omgagi.ai/oauth/callback/\n\
             - Create and copy the Client ID and Client Secret\n\n\
             *4. Publish the app*\n\
             {consent_url}\n\
             - Go to \"Audience\" and click \"Publish App\"\n\n\
             Paste the full content of the downloaded JSON file when ready."
        ),
    };

    guide
}

/// Error: input is not valid Google credentials JSON.
pub(super) fn google_invalid_json_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Por favor, pega el contenido completo del archivo JSON de credenciales descargado.",
        "Portuguese" => "Por favor, cole o conteudo completo do arquivo JSON de credenciais baixado.",
        "French" => "Veuillez coller le contenu complet du fichier JSON d'identifiants telecharge.",
        "German" => "Bitte fuge den vollstandigen Inhalt der heruntergeladenen JSON-Anmeldedatei ein.",
        "Italian" => "Per favore, incolla il contenuto completo del file JSON delle credenziali scaricato.",
        "Dutch" => "Plak alsjeblieft de volledige inhoud van het gedownloade JSON-referentiebestand.",
        "Russian" => "Пожалуйста, вставьте полное содержимое скачанного JSON-файла с учетными данными.",
        _ => "Please paste the full JSON content from the downloaded credentials file.",
    }
}

/// Step 3: OAuth URL + ask for auth code.
pub(super) fn google_step_auth_code_message(lang: &str, auth_url: &str) -> String {
    match lang {
        "Spanish" => format!(
            "Credenciales recibidas.\n\n\
             Abre este enlace para autorizar tu cuenta de Google:\n\
             {auth_url}\n\n\
             Haz clic en \"Advanced\" y \"Go to omega (unsafe)\" y luego \"Allow\".\n\n\
             Copia el codigo de autorizacion y envialo aqui."
        ),
        "Portuguese" => format!(
            "Credenciais recebidas.\n\n\
             Abra este link para autorizar sua conta do Google:\n\
             {auth_url}\n\n\
             Clique em \"Advanced\" e \"Go to omega (unsafe)\" e depois \"Allow\".\n\n\
             Copie o codigo de autorizacao e envie aqui."
        ),
        "French" => format!(
            "Identifiants recus.\n\n\
             Ouvrez ce lien pour autoriser votre compte Google :\n\
             {auth_url}\n\n\
             Cliquez sur \"Advanced\" puis \"Go to omega (unsafe)\" puis \"Allow\".\n\n\
             Copiez le code d'autorisation et envoyez-le ici."
        ),
        "German" => format!(
            "Zugangsdaten erhalten.\n\n\
             Offne diesen Link, um dein Google-Konto zu autorisieren:\n\
             {auth_url}\n\n\
             Klicke auf \"Advanced\" und \"Go to omega (unsafe)\" und dann \"Allow\".\n\n\
             Kopiere den Autorisierungscode und sende ihn hier."
        ),
        "Italian" => format!(
            "Credenziali ricevute.\n\n\
             Apri questo link per autorizzare il tuo account Google:\n\
             {auth_url}\n\n\
             Clicca su \"Advanced\" e \"Go to omega (unsafe)\" poi \"Allow\".\n\n\
             Copia il codice di autorizzazione e invialo qui."
        ),
        "Dutch" => format!(
            "Inloggegevens ontvangen.\n\n\
             Open deze link om je Google-account te autoriseren:\n\
             {auth_url}\n\n\
             Klik op \"Advanced\" en \"Go to omega (unsafe)\" en dan \"Allow\".\n\n\
             Kopieer de autorisatiecode en stuur deze hier."
        ),
        "Russian" => format!(
            "Учетные данные получены.\n\n\
             Откройте эту ссылку для авторизации вашего аккаунта Google:\n\
             {auth_url}\n\n\
             Нажмите \"Advanced\" и \"Go to omega (unsafe)\" затем \"Allow\".\n\n\
             Скопируйте код авторизации и отправьте его сюда."
        ),
        _ => format!(
            "Credentials received.\n\n\
             Open this link to authorize your Google account:\n\
             {auth_url}\n\n\
             Click \"Advanced\" then \"Go to omega (unsafe)\" then \"Allow\".\n\n\
             Copy the authorization code and send it here."
        ),
    }
}

/// Completion: Google connected with email.
pub(super) fn google_step_complete_message(lang: &str, email: &str) -> String {
    match lang {
        "Spanish" => format!("Google conectado correctamente — {email}"),
        "Portuguese" => format!("Google conectado com sucesso — {email}"),
        "French" => format!("Google connecte avec succes — {email}"),
        "German" => format!("Google erfolgreich verbunden — {email}"),
        "Italian" => format!("Google connesso con successo — {email}"),
        "Dutch" => format!("Google succesvol verbonden — {email}"),
        "Russian" => format!("Google успешно подключен — {email}"),
        _ => format!("Google connected successfully — {email}"),
    }
}

/// Token exchange error.
pub(super) fn google_token_exchange_error_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Error al intercambiar el codigo. Verifica tus credenciales y usa /google para reiniciar.",
        "Portuguese" => "Erro ao trocar o codigo. Verifique suas credenciais e use /google para reiniciar.",
        "French" => "Erreur lors de l'echange du code. Verifiez vos identifiants et utilisez /google pour recommencer.",
        "German" => "Fehler beim Code-Austausch. Uberprufe deine Zugangsdaten und verwende /google, um neu zu starten.",
        "Italian" => "Errore nello scambio del codice. Verifica le tue credenziali e usa /google per ricominciare.",
        "Dutch" => "Fout bij het uitwisselen van de code. Controleer je inloggegevens en gebruik /google om opnieuw te beginnen.",
        "Russian" => "Ошибка при обмене кода. Проверьте учетные данные и используйте /google, чтобы начать заново.",
        _ => "Token exchange failed. Check your credentials and use /google to start again.",
    }
}

/// Email fallback: could not detect email automatically.
pub(super) fn google_email_fallback_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "No se pudo detectar tu email automaticamente. Por favor, envia tu direccion de Gmail.",
        "Portuguese" => "Nao foi possivel detectar seu email automaticamente. Por favor, envie seu endereco do Gmail.",
        "French" => "Impossible de detecter votre email automatiquement. Veuillez envoyer votre adresse Gmail.",
        "German" => "Deine E-Mail konnte nicht automatisch erkannt werden. Bitte sende deine Gmail-Adresse.",
        "Italian" => "Impossibile rilevare la tua email automaticamente. Per favore, invia il tuo indirizzo Gmail.",
        "Dutch" => "Je e-mail kon niet automatisch worden gedetecteerd. Stuur alsjeblieft je Gmail-adres.",
        "Russian" => "Не удалось определить ваш email автоматически. Пожалуйста, отправьте ваш адрес Gmail.",
        _ => "Could not detect your email automatically. Please send your Gmail address.",
    }
}

/// Cancellation confirmation.
pub(super) fn google_cancelled_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Configuracion de Google cancelada.",
        "Portuguese" => "Configuracao do Google cancelada.",
        "French" => "Configuration Google annulee.",
        "German" => "Google-Einrichtung abgebrochen.",
        "Italian" => "Configurazione Google annullata.",
        "Dutch" => "Google-instelling geannuleerd.",
        "Russian" => "Настройка Google отменена.",
        _ => "Google setup cancelled.",
    }
}

/// Session expired.
pub(super) fn google_expired_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => {
            "La sesion de configuracion de Google ha expirado. Usa /google para reiniciar."
        }
        "Portuguese" => "A sessao de configuracao do Google expirou. Use /google para reiniciar.",
        "French" => {
            "La session de configuration Google a expire. Utilisez /google pour recommencer."
        }
        "German" => {
            "Die Google-Einrichtungssitzung ist abgelaufen. Verwende /google, um neu zu starten."
        }
        "Italian" => {
            "La sessione di configurazione Google e scaduta. Usa /google per ricominciare."
        }
        "Dutch" => {
            "De Google-installatiesessie is verlopen. Gebruik /google om opnieuw te beginnen."
        }
        "Russian" => "Сессия настройки Google истекла. Используйте /google, чтобы начать заново.",
        _ => "Google setup session expired. Use /google to start again.",
    }
}

/// Concurrent session guard.
pub(super) fn google_conflict_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Ya tienes una sesion de configuracion de Google activa. Completala o escribe *cancel* para cancelarla.",
        "Portuguese" => "Voce ja tem uma sessao de configuracao do Google ativa. Complete-a ou escreva *cancel* para cancela-la.",
        "French" => "Vous avez deja une session de configuration Google active. Terminez-la ou ecrivez *cancel* pour l'annuler.",
        "German" => "Du hast bereits eine aktive Google-Einrichtungssitzung. Schliesse sie ab oder schreibe *cancel*, um sie abzubrechen.",
        "Italian" => "Hai gia una sessione di configurazione Google attiva. Completala o scrivi *cancel* per annullarla.",
        "Dutch" => "Je hebt al een actieve Google-installatiesessie. Voltooi deze of typ *cancel* om te annuleren.",
        "Russian" => "У вас уже есть активная сессия настройки Google. Завершите её или напишите *cancel* для отмены.",
        _ => "You already have an active Google setup session. Complete it or type *cancel* to abort.",
    }
}

/// Validation error: empty input.
pub(super) fn google_empty_input_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "La entrada no puede estar vacia. Por favor, envia un valor.",
        "Portuguese" => "A entrada nao pode estar vazia. Por favor, envie um valor.",
        "French" => "L'entree ne peut pas etre vide. Veuillez envoyer une valeur.",
        "German" => "Die Eingabe darf nicht leer sein. Bitte sende einen Wert.",
        "Italian" => "L'input non puo essere vuoto. Per favore, invia un valore.",
        "Dutch" => "De invoer mag niet leeg zijn. Stuur alsjeblieft een waarde.",
        "Russian" => "Ввод не может быть пустым. Пожалуйста, отправьте значение.",
        _ => "Input cannot be empty. Please send a value.",
    }
}

/// Validation error: invalid email format.
pub(super) fn google_invalid_email_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Formato de email invalido. Por favor, envia una direccion de email valida.",
        "Portuguese" => "Formato de email invalido. Por favor, envie um endereco de email valido.",
        "French" => "Format d'email invalide. Veuillez envoyer une adresse email valide.",
        "German" => "Ungultiges E-Mail-Format. Bitte sende eine gultige E-Mail-Adresse.",
        "Italian" => "Formato email non valido. Per favore, invia un indirizzo email valido.",
        "Dutch" => "Ongeldig e-mailformaat. Stuur alsjeblieft een geldig e-mailadres.",
        "Russian" => {
            "Неверный формат электронной почты. Пожалуйста, отправьте действительный адрес."
        }
        _ => "Invalid email format. Please send a valid email address.",
    }
}

/// Error: failed to start session.
pub(super) fn google_start_error_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "No se pudo iniciar la configuracion de Google. Intentalo de nuevo.",
        "Portuguese" => "Nao foi possivel iniciar a configuracao do Google. Tente novamente.",
        "French" => "Impossible de demarrer la configuration Google. Reessayez.",
        "German" => "Google-Einrichtung konnte nicht gestartet werden. Versuche es erneut.",
        "Italian" => "Impossibile avviare la configurazione Google. Riprova.",
        "Dutch" => "Kan Google-instelling niet starten. Probeer het opnieuw.",
        "Russian" => "Не удалось начать настройку Google. Попробуйте снова.",
        _ => "Failed to start Google setup. Please try again.",
    }
}

/// Error: failed to store credential in current step.
pub(super) fn google_store_error_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "No se pudo guardar el valor. Intentalo de nuevo.",
        "Portuguese" => "Nao foi possivel salvar o valor. Tente novamente.",
        "French" => "Impossible de sauvegarder la valeur. Reessayez.",
        "German" => "Der Wert konnte nicht gespeichert werden. Versuche es erneut.",
        "Italian" => "Impossibile salvare il valore. Riprova.",
        "Dutch" => "Kan de waarde niet opslaan. Probeer het opnieuw.",
        "Russian" => "Не удалось сохранить значение. Попробуйте снова.",
        _ => "Failed to save the value. Please try again.",
    }
}

/// Error: missing credential data at completion.
pub(super) fn google_missing_data_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Faltan datos de credenciales. Usa /google para reiniciar.",
        "Portuguese" => "Dados de credenciais ausentes. Use /google para reiniciar.",
        "French" => "Donnees d'identification manquantes. Utilisez /google pour recommencer.",
        "German" => "Fehlende Zugangsdaten. Verwende /google, um neu zu starten.",
        "Italian" => "Dati delle credenziali mancanti. Usa /google per ricominciare.",
        "Dutch" => "Ontbrekende inloggegevens. Gebruik /google om opnieuw te beginnen.",
        "Russian" => "Отсутствуют данные учетных данных. Используйте /google, чтобы начать заново.",
        _ => "Missing credential data. Use /google to start again.",
    }
}

/// Error: failed to write google.json.
pub(super) fn google_write_error_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "No se pudieron guardar las credenciales de Google. Usa /google para reiniciar.",
        "Portuguese" => "Nao foi possivel salvar as credenciais do Google. Use /google para reiniciar.",
        "French" => "Impossible de sauvegarder les identifiants Google. Utilisez /google pour recommencer.",
        "German" => "Google-Zugangsdaten konnten nicht gespeichert werden. Verwende /google, um neu zu starten.",
        "Italian" => "Impossibile salvare le credenziali Google. Usa /google per ricominciare.",
        "Dutch" => "Kan Google-inloggegevens niet opslaan. Gebruik /google om opnieuw te beginnen.",
        "Russian" => "Не удалось сохранить учетные данные Google. Используйте /google, чтобы начать заново.",
        _ => "Failed to save Google credentials. Use /google to start again.",
    }
}

/// Error: unknown step in state machine.
pub(super) fn google_unknown_step_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "La configuracion de Google encontro un estado inesperado. Usa /google para reiniciar.",
        "Portuguese" => "A configuracao do Google encontrou um estado inesperado. Use /google para reiniciar.",
        "French" => "La configuration Google a rencontre un etat inattendu. Utilisez /google pour recommencer.",
        "German" => "Die Google-Einrichtung hat einen unerwarteten Zustand erreicht. Verwende /google, um neu zu starten.",
        "Italian" => "La configurazione Google ha riscontrato uno stato imprevisto. Usa /google per ricominciare.",
        "Dutch" => "De Google-instelling heeft een onverwachte status bereikt. Gebruik /google om opnieuw te beginnen.",
        "Russian" => "Настройка Google столкнулась с неожиданным состоянием. Используйте /google, чтобы начать заново.",
        _ => "Google setup encountered an unexpected state. Use /google to start again.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_LANGUAGES: &[&str] = &[
        "English",
        "Spanish",
        "Portuguese",
        "French",
        "German",
        "Italian",
        "Dutch",
        "Russian",
    ];

    // ===================================================================
    // All messages return non-empty for all 8 languages
    // ===================================================================

    #[test]
    fn test_project_id_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_step_project_id_message(lang, false);
            assert!(
                !msg.is_empty(),
                "project_id({lang}, false) must not be empty"
            );
            let msg_existing = google_step_project_id_message(lang, true);
            assert!(
                !msg_existing.is_empty(),
                "project_id({lang}, true) must not be empty"
            );
            assert!(
                msg_existing.len() > msg.len(),
                "overwrite warning must add content"
            );
        }
    }

    #[test]
    fn test_setup_guide_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_step_setup_guide_message(lang, "test-project");
            assert!(!msg.is_empty(), "setup_guide({lang}) must not be empty");
            assert!(
                msg.contains("test-project"),
                "setup_guide must contain project ID"
            );
        }
    }

    #[test]
    fn test_invalid_json_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_invalid_json_message(lang);
            assert!(
                !msg.is_empty(),
                "invalid_json({lang}) must not be empty"
            );
        }
    }

    #[test]
    fn test_auth_code_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_step_auth_code_message(lang, "https://example.com/auth");
            assert!(!msg.is_empty(), "auth_code({lang}) must not be empty");
            assert!(
                msg.contains("https://example.com/auth"),
                "auth_code must contain URL"
            );
        }
    }

    #[test]
    fn test_complete_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_step_complete_message(lang, "user@example.com");
            assert!(!msg.is_empty(), "complete({lang}) must not be empty");
            assert!(
                msg.contains("user@example.com"),
                "complete must contain email"
            );
        }
    }

    #[test]
    fn test_token_exchange_error_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_token_exchange_error_message(lang);
            assert!(
                !msg.is_empty(),
                "token_exchange_error({lang}) must not be empty"
            );
        }
    }

    #[test]
    fn test_email_fallback_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_email_fallback_message(lang);
            assert!(!msg.is_empty(), "email_fallback({lang}) must not be empty");
        }
    }

    #[test]
    fn test_cancelled_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_cancelled_message(lang);
            assert!(!msg.is_empty(), "cancelled({lang}) must not be empty");
        }
    }

    #[test]
    fn test_expired_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_expired_message(lang);
            assert!(!msg.is_empty(), "expired({lang}) must not be empty");
        }
    }

    #[test]
    fn test_conflict_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_conflict_message(lang);
            assert!(!msg.is_empty(), "conflict({lang}) must not be empty");
        }
    }

    #[test]
    fn test_empty_input_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_empty_input_message(lang);
            assert!(!msg.is_empty(), "empty_input({lang}) must not be empty");
        }
    }

    #[test]
    fn test_invalid_email_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_invalid_email_message(lang);
            assert!(!msg.is_empty(), "invalid_email({lang}) must not be empty");
        }
    }

    // ===================================================================
    // English fallback for unknown languages
    // ===================================================================

    #[test]
    fn test_default_english_fallback() {
        let unknown_lang = "Klingon";
        assert_eq!(
            google_step_project_id_message(unknown_lang, false),
            google_step_project_id_message("English", false),
        );
        assert_eq!(
            google_invalid_json_message(unknown_lang),
            google_invalid_json_message("English"),
        );
        assert_eq!(
            google_cancelled_message(unknown_lang),
            google_cancelled_message("English"),
        );
        assert_eq!(
            google_expired_message(unknown_lang),
            google_expired_message("English"),
        );
        assert_eq!(
            google_conflict_message(unknown_lang),
            google_conflict_message("English"),
        );
        assert_eq!(
            google_empty_input_message(unknown_lang),
            google_empty_input_message("English"),
        );
    }

    // ===================================================================
    // Translations differ from English
    // ===================================================================

    #[test]
    fn test_spanish_differs_from_english() {
        assert_ne!(
            google_cancelled_message("Spanish"),
            google_cancelled_message("English"),
        );
    }

    #[test]
    fn test_french_differs_from_english() {
        assert_ne!(
            google_expired_message("French"),
            google_expired_message("English"),
        );
    }

    #[test]
    fn test_german_differs_from_english() {
        assert_ne!(
            google_conflict_message("German"),
            google_conflict_message("English"),
        );
    }

    #[test]
    fn test_russian_differs_from_english() {
        assert_ne!(
            google_invalid_json_message("Russian"),
            google_invalid_json_message("English"),
        );
    }

    #[test]
    fn test_portuguese_differs_from_english() {
        assert_ne!(
            google_empty_input_message("Portuguese"),
            google_empty_input_message("English"),
        );
    }

    #[test]
    fn test_italian_differs_from_english() {
        assert_ne!(
            google_invalid_email_message("Italian"),
            google_invalid_email_message("English"),
        );
    }

    #[test]
    fn test_dutch_differs_from_english() {
        assert_ne!(
            google_invalid_json_message("Dutch"),
            google_invalid_json_message("English"),
        );
    }

    // ===================================================================
    // Setup guide contains project-specific links
    // ===================================================================

    #[test]
    fn test_setup_guide_contains_api_links() {
        let msg = google_step_setup_guide_message("English", "my-proj-123");
        assert!(msg.contains("gmail.googleapis.com"));
        assert!(msg.contains("calendar-json.googleapis.com"));
        assert!(msg.contains("drive.googleapis.com"));
        assert!(msg.contains("my-proj-123"));
    }

    #[test]
    fn test_setup_guide_contains_console_links() {
        let msg = google_step_setup_guide_message("English", "my-proj");
        assert!(msg.contains("apis/credentials/consent"));
        assert!(msg.contains("apis/credentials/oauthclient"));
        assert!(msg.contains("omgagi.ai/oauth/callback"));
    }
}
