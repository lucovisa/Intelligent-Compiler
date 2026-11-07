//! #локализация — тексты интерфейса.
//! Добавить язык = добавить вариант в `Lang` и третий аргумент в каждый `pick`.
//! Ключ без перевода не соберётся: `match` по `K` без `_`.

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
    // тема / язык
    Theme,
    Dark,
    Light,
    Language,
    // #старт — экран создания проекта
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
    // #редактор
    Files,
    Save,
    Close,
    Unsaved,
    NotText,
    // #запуск и #компиляция
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
            "Не найден файл ic рядом со Studio. Держи их в одной папке.",
            "The 'ic' file was not found next to Studio. Keep them in one folder.",
        ),

        K::Files => pick(lang, "ФАЙЛЫ", "FILES"),
        K::Save => pick(lang, "Сохранить", "Save"),
        K::Close => pick(lang, "Закрыть проект", "Close project"),
        K::Unsaved => pick(lang, "не сохранено", "unsaved"),
        K::NotText => pick(lang, "Этот файл нельзя открыть как текст", "This file cannot be opened as text"),

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
