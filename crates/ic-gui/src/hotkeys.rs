use super::*;

// (клавиши, русский текст, английский текст); пустые клавиши — заголовок раздела
pub const SHORTCUTS: &[(&str, &str, &str)] = &[
    ("", "Общие", "General"),
    ("Ctrl+S", "Сохранить", "Save"),
    ("F5", "Запустить", "Run"),
    ("Ctrl+Shift+B", "Собрать exe", "Build exe"),
    ("Ctrl+P", "Быстро открыть файл", "Quick open file"),
    ("Ctrl+Shift+P", "Палитра команд", "Command palette"),
    ("Ctrl+F", "Поиск по проекту", "Search in project"),
    ("Ctrl+,", "Настройки программы", "Settings"),
    ("F1", "Эта подсказка", "This help"),
    ("", "Файлы и вкладки", "Files and tabs"),
    ("Ctrl+N", "Новый файл", "New file"),
    ("Ctrl+Shift+N", "Новая папка", "New folder"),
    ("F2", "Переименовать", "Rename"),
    (
        "Ctrl+C / Ctrl+V",
        "Копировать и вставить файл или папку",
        "Copy and paste a file or folder",
    ),
    ("Delete", "Удалить выбранный файл", "Delete selected file"),
    ("Ctrl+W", "Закрыть вкладку", "Close tab"),
    ("Ctrl+Tab", "Следующая вкладка", "Next tab"),
    ("Ctrl+Shift+Tab", "Предыдущая вкладка", "Previous tab"),
    ("", "Окно", "Window"),
    ("Ctrl+B", "Показать или скрыть проводник", "Toggle explorer"),
    ("Ctrl+J", "Показать или скрыть журнал", "Toggle log"),
    ("Ctrl+1", "Режим «Скрипт»", "Script mode"),
    ("Ctrl+2", "Режим «Редактор сцен»", "Scene editor mode"),
    ("", "Код", "Code"),
    (
        "Ctrl+/",
        "Закомментировать или раскомментировать строки",
        "Toggle line comment",
    ),
    ("Ctrl+Z / Ctrl+Y", "Отменить / вернуть", "Undo / redo"),
    ("", "Редактор сцен", "Scene editor"),
    ("Ctrl+A", "Выделить всё", "Select all"),
    (
        "Shift+клик",
        "Добавить узел к выделению",
        "Add node to selection",
    ),
    (
        "Ctrl+C / Ctrl+V",
        "Копировать / вставить узлы",
        "Copy / paste nodes",
    ),
    ("Ctrl+D", "Дублировать", "Duplicate"),
    ("Delete", "Удалить узлы", "Delete nodes"),
    (
        "Стрелки",
        "Сдвинуть на 1 (Shift — на 10)",
        "Nudge by 1 (Shift: 10)",
    ),
    (
        "F / Home",
        "Показать выделенное / весь кадр",
        "Focus selection / fit frame",
    ),
    ("Esc", "Снять выделение", "Clear selection"),
    (
        "Alt при перетаскивании",
        "Без привязки к сетке",
        "Disable snapping",
    ),
    (
        "Пробел+мышь",
        "Перемещать холст (или средняя кнопка)",
        "Pan the canvas (or middle button)",
    ),
    ("", "Перетаскивание", "Drag and drop"),
    (
        "Файлы из ОС",
        "В окно проекта: копируются в assets",
        "Into a project window: copied to assets",
    ),
    (
        "Папка проекта",
        "В главное меню: открывает проект",
        "Into the main menu: opens the project",
    ),
    (
        ".lang / .theme",
        "В главное меню: ставятся в IC-data",
        "Into the main menu: installed into IC-data",
    ),
    (
        "Файлы в проводнике",
        "Перенос в другую папку",
        "Move to another folder",
    ),
    (
        "Строки дерева сцены",
        "Меняют порядок слоёв",
        "Change the layer order",
    ),
];

// Тексты для файлов перевода (английские строки из таблицы)
pub fn english_texts() -> Vec<&'static str> {
    SHORTCUTS.iter().map(|(_, _, en)| *en).collect()
}

pub fn show(ctx: &egui::Context, open: &mut bool, lang: Lang) {
    egui::Window::new(t2(lang, "Горячие клавиши", "Keyboard shortcuts"))
        .open(open)
        .default_width(560.0)
        .default_height(520.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("hotkeys_grid")
                    .num_columns(2)
                    .spacing([28.0, 6.0])
                    .show(ui, |ui| {
                        for (keys, ru, en) in SHORTCUTS {
                            if keys.is_empty() {
                                ui.label("");
                                ui.label("");
                                ui.end_row();
                                ui.label(
                                    RichText::new(
                                        i18n::custom_text(en).unwrap_or(if lang == Lang::Ru {
                                            ru
                                        } else {
                                            en
                                        }),
                                    )
                                    .strong(),
                                );
                                ui.label("");
                                ui.end_row();
                            } else {
                                ui.label(RichText::new(*keys).monospace());
                                ui.label(i18n::custom_text(en).unwrap_or(if lang == Lang::Ru {
                                    ru
                                } else {
                                    en
                                }));
                                ui.end_row();
                            }
                        }
                    });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_window_renders_without_panics() {
        let ctx = egui::Context::default();
        let mut open = true;
        for lang in [Lang::Ru, Lang::En] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show(ctx, &mut open, lang));
        }
        assert!(open);
        assert!(english_texts().len() > 30);
    }
}
