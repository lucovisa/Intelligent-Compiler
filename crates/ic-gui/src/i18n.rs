#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Ru, Lang::En];

    pub fn label(self) -> &'static str {
        match self {
            Lang::Ru => "Русский",
            Lang::En => "English",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
        }
    }

    pub fn from_code(c: &str) -> Option<Lang> {
        Self::ALL.into_iter().find(|l| l.code() == c)
    }
}

#[derive(Clone, Copy)]
pub enum K {
    Theme,
    Dark,
    Light,
    Language,
    StartTitle,
    StartSub,
    ProjectName,
    Location,
    Create,
    OpenExisting,
    Recent,
    ErrName,
    ErrNotProject,
    ErrCreate,
    IcMissing,
    Save,
    Close,
    Unsaved,
    NotText,
    FoundProject,
    WillCreate,
    FoundList,
    ProjSettings,
    ProjSettingsNote,
    SetTitle,
    SetWindow,
    SetTick,
    SetBackground,
    Cancel,
    SettingsSaved,
    Check,
    CheckHint,
    OpenHint,
    SetName,
    SetNameHint,
    SetTarget,
    TargetThis,
    TargetHint,
    ErrRename,
    KindQuestion,
    KindApp,
    KindAppHint,
    KindGame,
    KindGameHint,
    GameType,
    Dim3DSoon,
    DimCustom,
    CustomLabel,
    AddNow,
    GenScene,
    GenFps,
    GenPhysics,
    GenCamera,
    DefaultCode,
    DefaultCodeHint,
    LaterHint,
    CreateBtn,
    Explorer,
    NewFile,
    NewFolder,
    NewFileShort,
    NewFolderShort,
    AddGen,
    Rename,
    Delete,
    ErrExists,
    GenAdded,
    SetKind,
    Run,
    Build,
    BuildHint,
    Log,
    ClearLog,
}

fn pick(lang: Lang, ru: &'static str, en: &'static str) -> &'static str {
    match lang {
        Lang::Ru => ru,
        Lang::En => en,
    }
}

pub fn tr(lang: Lang, k: K) -> &'static str {
    match k {
        K::Theme => pick(lang, "Тема", "Theme"),
        K::Dark => pick(lang, "тёмная", "dark"),
        K::Light => pick(lang, "светлая", "light"),
        K::Language => pick(lang, "Язык", "Language"),

        K::StartTitle => pick(lang, "Новый проект", "New project"),
        K::StartSub => pick(
            lang,
            "Укажи название и папку — после этого откроется редактор.",
            "Choose a name and a folder — the editor opens next.",
        ),
        K::ProjectName => pick(lang, "Название проекта", "Project name"),
        K::Location => pick(lang, "Папка для проектов", "Projects folder"),
        K::Create => pick(lang, "Создать и открыть", "Create and open"),
        K::OpenExisting => pick(lang, "Открыть существующий", "Open existing"),
        K::Recent => pick(lang, "Недавние", "Recent"),
        K::ErrName => pick(
            lang,
            "Введи название без символов / и \\",
            "Enter a name without / or \\ characters",
        ),
        K::ErrNotProject => pick(lang, "В этой папке нет main.lua", "No main.lua in this folder"),
        K::ErrCreate => pick(lang, "Не удалось создать проект", "Could not create the project"),
        K::IcMissing => pick(
            lang,
            "Не удалось определить путь к самой программе.",
            "Could not determine the path of the program itself.",
        ),

        K::Save => pick(lang, "Сохранить", "Save"),
        K::Close => pick(lang, "Закрыть", "Close"),
        K::Unsaved => pick(lang, "не сохранено", "unsaved"),
        K::NotText => pick(lang, "Этот файл нельзя открыть как текст", "This file cannot be opened as text"),

        K::FoundProject => pick(lang, "Проект найден — можно открыть", "Project found — you can open it"),
        K::WillCreate => pick(lang, "Будет создан новый проект", "A new project will be created"),
        K::FoundList => pick(lang, "Проекты в этой папке:", "Projects in this folder:"),
        K::ProjSettings => pick(lang, "Настройки", "Settings"),
        K::ProjSettingsNote => pick(
            lang,
            "Настройки проекта хранятся в файле ic.toml в папке проекта. Они применяются при запуске и вшиваются в exe при сборке. Файл можно править и руками. Если в main.lua задан ic.title, он главнее названия отсюда.",
            "Project settings are stored in ic.toml inside the project folder. They apply on run and are embedded into the exe on build. You can also edit the file by hand. If main.lua sets ic.title, it takes priority over the title here.",
        ),
        K::SetTitle => pick(lang, "Название окна", "Window title"),
        K::SetWindow => pick(lang, "Размер окна", "Window size"),
        K::SetTick => pick(lang, "Частота логики", "Logic rate"),
        K::SetBackground => pick(lang, "Цвет фона", "Background color"),
        K::Cancel => pick(lang, "Отмена", "Cancel"),
        K::SettingsSaved => pick(lang, "Настройки сохранены в ic.toml", "Settings saved to ic.toml"),
        K::Check => pick(lang, "Проверить", "Check"),
        K::CheckHint => pick(lang, "Проверить Lua-код без запуска", "Check Lua code without running"),

        K::OpenHint => pick(
            lang,
            "Чтобы открыть существующий проект, укажи его название и папку — внутри должен лежать файл main.lua.",
            "To open an existing project, enter its name and folder — it must contain a main.lua file.",
        ),
        K::SetName => pick(lang, "Название проекта", "Project name"),
        K::SetNameHint => pick(
            lang,
            "Название проекта — это имя его папки и готового файла. При сохранении папка переименуется.",
            "The project name is the name of its folder and of the built file. The folder is renamed on save.",
        ),
        K::SetTarget => pick(lang, "Собрать для", "Build for"),
        K::TargetThis => pick(lang, "Эта система", "This system"),
        K::TargetHint => pick(
            lang,
            "Для другой системы положи её файл ic в папку templates рядом с ic (имена: ic-windows.exe, ic-linux, ic-macos).",
            "For another system, put its ic file into the templates folder next to ic (names: ic-windows.exe, ic-linux, ic-macos).",
        ),
        K::ErrRename => pick(lang, "Не удалось переименовать папку", "Could not rename the folder"),

        K::KindQuestion => pick(lang, "Что создаём?", "What are we creating?"),
        K::KindApp => pick(lang, "Программа", "Application"),
        K::KindAppHint => pick(lang, "Обычное окно без игровых заготовок: ничего лишнего, только пустой проект.", "A plain window without game presets: nothing extra, just an empty project."),
        K::KindGame => pick(lang, "Игра", "Game"),
        K::KindGameHint => pick(lang, "Игровой цикл, спрайты, ввод. Сцену, FPS, физику и камеру можно добавить сразу.", "Game loop, sprites, input. You can add a scene, FPS, physics and camera right away."),
        K::GameType => pick(lang, "Тип игры", "Game type"),
        K::Dim3DSoon => pick(lang, "3D — скоро", "3D — coming soon"),
        K::DimCustom => pick(lang, "Свой тип", "Custom"),
        K::CustomLabel => pick(lang, "Название типа", "Type name"),
        K::AddNow => pick(lang, "Добавить в проект сразу", "Add to the project now"),
        K::GenScene => pick(lang, "Сцена", "Scene"),
        K::GenFps => pick(lang, "FPS", "FPS"),
        K::GenPhysics => pick(lang, "Физика", "Physics"),
        K::GenCamera => pick(lang, "Камера", "Camera"),
        K::DefaultCode => pick(lang, "Вставить код по умолчанию?", "Insert the default code?"),
        K::DefaultCodeHint => pick(lang, "Да — в main.lua появится демо с летающими квадратами. Нет — файл будет пустым.", "Yes — main.lua gets the flying-squares demo. No — the file stays empty."),
        K::LaterHint => pick(lang, "Сцену, FPS, физику и камеру можно добавить и потом: кнопка «Добавить» в проводнике.", "You can add a scene, FPS, physics and camera later: the “Add” button in the explorer."),
        K::CreateBtn => pick(lang, "Создать", "Create"),
        K::Explorer => pick(lang, "ПРОВОДНИК", "EXPLORER"),
        K::NewFile => pick(lang, "Новый файл", "New file"),
        K::NewFolder => pick(lang, "Новая папка", "New folder"),
        K::NewFileShort => pick(lang, "+ Файл", "+ File"),
        K::NewFolderShort => pick(lang, "+ Папка", "+ Folder"),
        K::AddGen => pick(lang, "Добавить", "Add"),
        K::Rename => pick(lang, "Переименовать", "Rename"),
        K::Delete => pick(lang, "Удалить", "Delete"),
        K::ErrExists => pick(lang, "Уже существует", "Already exists"),
        K::GenAdded => pick(lang, "Добавлено", "Added"),
        K::SetKind => pick(lang, "Тип проекта", "Project type"),
        K::Run => pick(lang, "Запустить", "Run"),
        K::Build => pick(lang, "Собрать exe", "Build exe"),
        K::BuildHint => pick(
            lang,
            "Один самостоятельный файл появится в <проект>/build/",
            "One standalone file will appear in <project>/build/",
        ),
        K::Log => pick(lang, "ЖУРНАЛ", "LOG"),
        K::ClearLog => pick(lang, "Очистить", "Clear"),
    }
}
