//! Локализация. Добавить язык = добавить вариант в `Lang` и ветки в `tr`
//! (компилятор не даст забыть перевод: match без `_`).

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
    ProjectFolder,
    NewProject,
    Run,
    Build,
    Log,
    ClearLog,
    Hint,
    IcMissing,
}

pub fn tr(lang: Lang, k: K) -> &'static str {
    use K::*;
    match (lang, k) {
        (Lang::Ru, Theme) => "Тема",
        (Lang::En, Theme) => "Theme",
        (Lang::Ru, Dark) => "Тёмная",
        (Lang::En, Dark) => "Dark",
        (Lang::Ru, Light) => "Светлая",
        (Lang::En, Light) => "Light",
        (Lang::Ru, Language) => "Язык",
        (Lang::En, Language) => "Language",
        (Lang::Ru, ProjectFolder) => "Папка проекта",
        (Lang::En, ProjectFolder) => "Project folder",
        (Lang::Ru, NewProject) => "Создать проект",
        (Lang::En, NewProject) => "New project",
        (Lang::Ru, Run) => "Запустить",
        (Lang::En, Run) => "Run",
        (Lang::Ru, Build) => "Собрать exe",
        (Lang::En, Build) => "Build exe",
        (Lang::Ru, Log) => "Журнал",
        (Lang::En, Log) => "Log",
        (Lang::Ru, ClearLog) => "Очистить",
        (Lang::En, ClearLog) => "Clear",
        (Lang::Ru, Hint) => "«Собрать exe» кладёт один самостоятельный файл в <папка>/build/",
        (Lang::En, Hint) => "\"Build exe\" writes one standalone file to <folder>/build/",
        (Lang::Ru, IcMissing) => "Не найден файл ic рядом со Studio. Положи их в одну папку.",
        (Lang::En, IcMissing) => {
            "The 'ic' file was not found next to Studio. Keep them in one folder."
        }
    }
}
