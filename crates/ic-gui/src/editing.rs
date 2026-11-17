// Комментирование строк (Ctrl+/): работает с индексами символов, как egui

// Префикс комментария по имени файла
pub fn comment_prefix(file_name: &str) -> &'static str {
    match file_name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "lua" | "sql" => "-- ",
        "py" | "sh" | "toml" | "yml" | "yaml" | "lang" | "theme" | "ps1" => "# ",
        _ => "// ",
    }
}

// Возвращает новый текст и новое выделение (все затронутые строки)
pub fn toggle_comment(text: &str, start: usize, end: usize, prefix: &str) -> (String, usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let (lo, hi) = (start.min(end).min(chars.len()), start.max(end).min(chars.len()));
    let mut first = lo;
    while first > 0 && chars[first - 1] != '\n' {
        first -= 1;
    }
    // Выделение, заканчивающееся в начале строки, следующую строку не затрагивает
    let mut last = if hi > lo && chars.get(hi - 1) == Some(&'\n') { hi - 1 } else { hi };
    while last < chars.len() && chars[last] != '\n' {
        last += 1;
    }
    let block: String = chars[first..last].iter().collect();
    let marker = prefix.trim_end();
    let lines: Vec<&str> = block.split('\n').collect();
    let all_commented = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .all(|l| l.trim_start().starts_with(marker));
    let has_text = lines.iter().any(|l| !l.trim().is_empty());
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let out: Vec<String> = lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                (*l).to_string()
            } else if has_text && all_commented {
                let indent = l.len() - l.trim_start().len();
                let rest = &l[indent..][marker.len()..];
                let rest = rest.strip_prefix(' ').unwrap_or(rest);
                format!("{}{}", &l[..indent], rest)
            } else {
                format!("{}{}{}", &l[..min_indent], prefix, &l[min_indent..])
            }
        })
        .collect();
    let new_block = out.join("\n");
    let mut result: String = chars[..first].iter().collect();
    result.push_str(&new_block);
    result.extend(chars[last..].iter());
    (result, first, first + new_block.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_uncomments_lua_lines() {
        let text = "local a = 1\n    local b = 2\nprint(a)\n";
        let (commented, s, e) = toggle_comment(text, 0, 20, "-- ");
        assert_eq!(commented, "-- local a = 1\n--     local b = 2\nprint(a)\n");
        assert_eq!(s, 0);
        let (back, _, _) = toggle_comment(&commented, s, e, "-- ");
        assert_eq!(back, text);
    }

    #[test]
    fn single_line_and_blank_lines() {
        let (t, _, _) = toggle_comment("a\n\nb\n", 0, 4, "// ");
        assert_eq!(t, "// a\n\n// b\n");
        let (t, _, _) = toggle_comment("x = 1\n", 2, 2, "# ");
        assert_eq!(t, "# x = 1\n");
    }

    #[test]
    fn selection_ending_at_line_start_skips_next_line() {
        let (t, _, _) = toggle_comment("one\ntwo\n", 0, 4, "// ");
        assert_eq!(t, "// one\ntwo\n");
    }

    #[test]
    fn mixed_block_gets_commented_once_more() {
        let (t, _, _) = toggle_comment("-- a\nb\n", 0, 6, "-- ");
        assert_eq!(t, "-- -- a\n-- b\n");
    }

    #[test]
    fn prefix_by_extension() {
        assert_eq!(comment_prefix("main.lua"), "-- ");
        assert_eq!(comment_prefix("a.cpp"), "// ");
        assert_eq!(comment_prefix("ic.toml"), "# ");
    }
}
