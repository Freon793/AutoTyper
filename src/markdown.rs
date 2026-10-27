//! AutoTyper - Markdown 转纯文本
//!
//! 把 AI 输出常见的 Markdown 文本转换成适合输入到纯文本框的等宽纯文本：
//! 去掉语法记号、保留可读结构。这不是 Markdown 渲染器——不生成 HTML，
//! 也不追求版式还原。
//!
//! 处理分两阶段，顺序不可颠倒：
//!
//! 1. **块级**：按行识别围栏代码块、标题、引用、列表、表格、分隔线与空行；
//! 2. **行内**：只对未被代码块保护的段落处理粗体、斜体、行内代码、链接、
//!    HTML 标签与数学公式。
//!
//! 代码块内容绝对不做行内处理——否则 `#include <stdio.h>` 会被当成标题、
//! `_var_` 会被当成斜体。这是本模块必须分两阶段的原因。

/// 分隔线输出的横线宽度
const RULE_WIDTH: usize = 40;
/// 表格列之间的间隔空格数
const TABLE_GAP: usize = 3;
/// 表格分隔行的最小列宽
const MIN_COL_WIDTH: usize = 3;
/// 制表符展开的空格数
const TAB_WIDTH: usize = 4;

/// 允许剥离的行内 HTML 标签白名单
///
/// 只处理明确无歧义的标签：`a<b>c` 这类泛型/比较写法中的单字母标签
/// 不在白名单内，避免误删内容。
const HTML_TAG_WHITELIST: [&str; 17] = [
    "br", "p", "div", "span", "sup", "sub", "code", "kbd", "details", "summary", "em", "strong",
    "del", "ins", "hr", "small", "u",
];

/// 把 Markdown 文本转换为纯文本。
///
/// 纯文本输入会原样返回（除 CRLF 归一化、制表符展开与行尾空格清理外）。
pub fn to_plain_text(input: &str) -> String {
    let normalized = normalize(input);
    let lines: Vec<&str> = normalized.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;

    while i < lines.len() {
        let line = lines[i];

        // 围栏代码块：内容整体保护，不做任何行内处理
        if let Some((fence_ch, fence_len, indent)) = fence_open(line) {
            i = emit_code_block(&lines, i, fence_ch, fence_len, indent, &mut out);
            continue;
        }

        // 空行
        if line.trim().is_empty() {
            push_blank(&mut out);
            i += 1;
            continue;
        }

        // 引用：剥掉标记后按内层语法继续识别（支持 `> - item` 这类组合）
        let line = strip_blockquote(line);

        // 分隔线
        if is_thematic_break(line) {
            push_blank(&mut out);
            out.push("─".repeat(RULE_WIDTH));
            push_blank(&mut out);
            i += 1;
            continue;
        }

        // ATX 标题
        if let Some(text) = atx_heading(line) {
            push_blank(&mut out);
            push_text(&mut out, &inline(&text));
            push_blank(&mut out);
            i += 1;
            continue;
        }

        // 表格：当前行含 `|`，且下一行是分隔行
        if line.contains('|') {
            let is_table = lines
                .get(i + 1)
                .map(|next| table_delimiter(next))
                .unwrap_or(false);
            if is_table {
                i = emit_table(&lines, i, line, &mut out);
                continue;
            }
        }

        // 列表项
        if let Some((indent, marker, content)) = list_item(line) {
            push_text(&mut out, &render_list_item(&indent, &marker, &content));
            i += 1;
            continue;
        }

        // Setext 标题：下一行是纯 `===` 或 `---`
        if let Some(next) = lines.get(i + 1) {
            if setext_level(next).is_some() {
                push_blank(&mut out);
                push_text(&mut out, &inline(line));
                push_blank(&mut out);
                i += 2;
                continue;
            }
        }

        // 普通段落
        push_text(&mut out, &inline(line));
        i += 1;
    }

    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

// ---------------------------------------------------------------------------
// 归一化与输出辅助
// ---------------------------------------------------------------------------

/// 统一换行符、展开制表符、去掉 BOM。
///
/// 制表符必须展开：引擎会把 `\t` 转成 `VK_TAB` 按键，而在浏览器输入框里
/// Tab 会移动焦点，后续字符将输入到别处。
fn normalize(input: &str) -> String {
    let mut s = input.strip_prefix('\u{feff}').unwrap_or(input).to_string();
    if s.contains('\r') {
        s = s.replace("\r\n", "\n").replace('\r', "\n");
    }
    if s.contains('\t') {
        s = s.replace('\t', &" ".repeat(TAB_WIDTH));
    }
    s
}

/// 压入空行（不产生开头空行与连续空行）
fn push_blank(out: &mut Vec<String>) {
    if out.is_empty() || out.last().is_some_and(|l| l.is_empty()) {
        return;
    }
    out.push(String::new());
}

/// 压入文本行（清掉行尾空格——行尾空格会被真实输入出去）
fn push_text(out: &mut Vec<String>, s: &str) {
    out.push(s.trim_end().to_string());
}

// ---------------------------------------------------------------------------
// 块级识别
// ---------------------------------------------------------------------------

/// 识别围栏代码块起始行，返回 `(围栏字符, 围栏长度, 缩进空格数)`
fn fence_open(line: &str) -> Option<(char, usize, usize)> {
    let indent = line.len() - line.trim_start().len();
    let t = line.trim_start();
    let c = t.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let len = t.chars().take_while(|&x| x == c).count();
    if len < 3 {
        return None;
    }
    Some((c, len, indent))
}

/// 判断是否为围栏代码块结束行
fn fence_close(line: &str, c: char, len: usize) -> bool {
    let t = line.trim_start();
    let run = t.chars().take_while(|&x| x == c).count();
    run >= len && t.chars().skip(run).all(|x| x.is_whitespace())
}

/// 去掉行首至多 `n` 个空格
fn strip_indent(line: &str, n: usize) -> &str {
    let mut idx = 0usize;
    for (count, ch) in line.char_indices() {
        if count >= n || ch != ' ' {
            return &line[idx..];
        }
        idx = count + ch.len_utf8();
    }
    &line[idx..]
}

/// 输出一个围栏代码块，返回下一个待处理行下标
fn emit_code_block(
    lines: &[&str],
    start: usize,
    c: char,
    len: usize,
    indent: usize,
    out: &mut Vec<String>,
) -> usize {
    push_blank(out);
    let mut j = start + 1;
    while j < lines.len() {
        if fence_close(lines[j], c, len) {
            j += 1;
            break;
        }
        // 代码内容原样保留（含缩进与行尾空格，只按围栏缩进对齐）
        out.push(strip_indent(lines[j], indent).to_string());
        j += 1;
    }
    push_blank(out);
    j
}

/// 剥掉（可能嵌套的）块引用标记
fn strip_blockquote(line: &str) -> &str {
    let mut s = line;
    loop {
        let t = s.trim_start();
        match t.strip_prefix('>') {
            Some(rest) => s = rest.strip_prefix(' ').unwrap_or(rest),
            None => return s,
        }
    }
}

/// 判断分隔线：仅由同一种 `-` / `*` / `_` 组成（可含空格），至少 3 个
fn is_thematic_break(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    let mut kind: Option<char> = None;
    let mut count = 0usize;
    for ch in t.chars() {
        if ch == ' ' || ch == '\t' {
            continue;
        }
        if ch != '-' && ch != '*' && ch != '_' {
            return false;
        }
        match kind {
            None => kind = Some(ch),
            Some(k) if k != ch => return false,
            _ => {}
        }
        count += 1;
    }
    count >= 3
}

/// 识别 ATX 标题，返回标题文字
fn atx_heading(line: &str) -> Option<String> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    if rest.is_empty() {
        return Some(String::new());
    }
    if !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    Some(trim_closing_hashes(rest.trim()))
}

/// 去掉 CommonMark 允许的标题结尾井号（须与前文空格分隔）
fn trim_closing_hashes(text: &str) -> String {
    let t = text.trim_end();
    if !t.ends_with('#') {
        return t.to_string();
    }
    let without = t.trim_end_matches('#');
    if without.is_empty() || without.ends_with(' ') || without.ends_with('\t') {
        without.trim_end().to_string()
    } else {
        t.to_string()
    }
}

/// 识别 Setext 标题下划线
fn setext_level(line: &str) -> Option<u8> {
    let t = line.trim();
    if t.is_empty() {
        return None;
    }
    if t.chars().all(|c| c == '=') {
        return Some(1);
    }
    if t.chars().all(|c| c == '-') {
        return Some(2);
    }
    None
}

/// 判断表格分隔行
fn table_delimiter(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || !t.contains('|') {
        return false;
    }
    let cells = split_cells(t);
    !cells.is_empty() && cells.iter().all(|c| is_delim_cell(c))
}

/// 单个分隔单元格：可选冒号 + 至少一个短横 + 可选冒号
fn is_delim_cell(cell: &str) -> bool {
    let t = cell.trim();
    let t = t.strip_prefix(':').unwrap_or(t);
    let t = t.strip_suffix(':').unwrap_or(t);
    !t.is_empty() && t.chars().all(|c| c == '-')
}

/// 按未转义的 `|` 拆分表格行，并去掉首尾的空列
fn split_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);

    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut escaped = false;
    for ch in t.chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => {
                escaped = true;
                cur.push(ch);
            }
            '|' => {
                cells.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

/// 输出一张表格（按显示宽度对齐），返回下一个待处理行下标
fn emit_table(lines: &[&str], start: usize, header_line: &str, out: &mut Vec<String>) -> usize {
    let header: Vec<String> = split_cells(header_line);
    let ncols = header.len();

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut j = start + 2;
    while j < lines.len() {
        let l = lines[j];
        if l.trim().is_empty() || !l.contains('|') || fence_open(l).is_some() {
            break;
        }
        if table_delimiter(l) {
            break;
        }
        rows.push(split_cells(l));
        j += 1;
    }

    let header_out: Vec<String> = header.iter().map(|c| inline(c)).collect();
    let rows_out: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            let mut cells: Vec<String> = r.iter().map(|c| inline(c)).collect();
            cells.resize(ncols, String::new());
            cells
        })
        .collect();

    let mut widths = vec![MIN_COL_WIDTH; ncols];
    for (k, cell) in header_out.iter().enumerate() {
        widths[k] = widths[k].max(display_width(cell));
    }
    for row in &rows_out {
        for (k, cell) in row.iter().enumerate().take(ncols) {
            widths[k] = widths[k].max(display_width(cell));
        }
    }

    push_blank(out);
    out.push(render_table_row(&header_out, &widths));
    // 分隔行用 U+2500：ASCII 短横会被再次识别成分隔线，破坏重复转换的稳定性
    let separator: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
    out.push(separator.join(&" ".repeat(TABLE_GAP)));
    for row in &rows_out {
        out.push(render_table_row(row, &widths));
    }
    push_blank(out);
    j
}

/// 渲染一行表格：按显示宽度左对齐补空格
fn render_table_row(cells: &[String], widths: &[usize]) -> String {
    let mut line = String::new();
    for (k, cell) in cells.iter().enumerate().take(widths.len()) {
        if k > 0 {
            line.push_str(&" ".repeat(TABLE_GAP));
        }
        line.push_str(cell);
        let pad = widths[k].saturating_sub(display_width(cell));
        line.push_str(&" ".repeat(pad));
    }
    line.trim_end().to_string()
}

/// 识别列表项，返回 `(缩进, 标记, 内容)`
fn list_item(line: &str) -> Option<(String, String, String)> {
    let indent_len = line.len() - line.trim_start().len();
    let indent = line[..indent_len].to_string();
    let t = &line[indent_len..];
    let first = t.chars().next()?;

    if first == '-' || first == '*' || first == '+' {
        let rest = &t[first.len_utf8()..];
        if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
            return Some((indent, "•".to_string(), rest.trim_start().to_string()));
        }
        return None;
    }

    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let after = &t[digits..];
    if !(after.starts_with('.') || after.starts_with(')')) {
        return None;
    }
    let rest = &after[1..];
    if !(rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')) {
        return None;
    }
    Some((
        indent,
        format!("{}.", &t[..digits]),
        rest.trim_start().to_string(),
    ))
}

/// 渲染单个列表项，处理任务列表复选框
fn render_list_item(indent: &str, marker: &str, content: &str) -> String {
    let mut body = content;
    let mut checkbox = "";

    for (prefix, label) in [
        ("[ ]", "[未完成] "),
        ("[x]", "[已完成] "),
        ("[X]", "[已完成] "),
    ] {
        if let Some(rest) = body.strip_prefix(prefix) {
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                checkbox = label;
                body = rest.trim_start();
                break;
            }
        }
    }

    let text = inline(body).trim_end().to_string();
    if text.is_empty() {
        format!("{indent}{marker}")
    } else if checkbox.is_empty() {
        format!("{indent}{marker} {text}")
    } else {
        format!("{indent}{marker} {checkbox}{text}")
    }
}

// ---------------------------------------------------------------------------
// 行内处理
// ---------------------------------------------------------------------------

/// 行内语法处理入口
fn inline(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    inline_chars(&cs)
}

/// 行内语法处理（作用在字符切片上，便于递归）
fn inline_chars(cs: &[char]) -> String {
    let mut out = String::new();
    let n = cs.len();
    let mut i = 0usize;

    while i < n {
        let c = cs[i];

        // 反斜杠转义：只对 ASCII 标点生效（`\frac` 这类反斜杠+字母不算转义）
        if c == '\\' && i + 1 < n {
            let next = cs[i + 1];
            if next.is_ascii_punctuation() {
                out.push(next);
                i += 2;
                continue;
            }
        }

        // 行内代码：等长反引号配对，内容原样输出
        if c == '`' {
            let run = count_run(cs, i, '`');
            if let Some(close) = find_code_close(cs, i + run, run) {
                let content: String = cs[i + run..close].iter().collect();
                out.push_str(content.trim());
                i = close + run;
                continue;
            }
            out.push_str(&"`".repeat(run));
            i += run;
            continue;
        }

        // 图片
        if c == '!' && i + 1 < n && cs[i + 1] == '[' {
            if let Some((alt, end)) = parse_link(cs, i + 1) {
                let alt_text = inline_chars(&alt);
                if alt_text.trim().is_empty() {
                    out.push_str("[图片]");
                } else {
                    out.push_str(&format!("[图片: {}]", alt_text.trim()));
                }
                i = end;
                continue;
            }
        }

        // 链接：[文字](url) 只保留文字
        if c == '[' {
            if let Some((text, end)) = parse_link(cs, i) {
                out.push_str(&inline_chars(&text));
                i = end;
                continue;
            }
        }

        // 数学公式
        if c == '$' {
            if let Some((content, end)) = parse_math(cs, i) {
                out.push_str(&math_to_text(&content));
                i = end;
                continue;
            }
        }

        // 行内 HTML 标签
        if c == '<' {
            if let Some((replacement, end)) = parse_html_tag(cs, i) {
                out.push_str(replacement);
                i = end;
                continue;
            }
        }

        // HTML 实体
        if c == '&' {
            if let Some((replacement, end)) = parse_entity(cs, i) {
                out.push_str(replacement);
                i = end;
                continue;
            }
        }

        // 强调
        if c == '*' || c == '_' || c == '~' {
            if let Some((content, end)) = parse_emphasis(cs, i) {
                out.push_str(&inline_chars(&content));
                i = end;
                continue;
            }
        }

        out.push(c);
        i += 1;
    }

    out
}

/// 统计从 `i` 起连续出现的字符 `c` 的个数
fn count_run(cs: &[char], i: usize, c: char) -> usize {
    cs[i..].iter().take_while(|&&x| x == c).count()
}

/// 查找长度恰为 `run` 的反引号串位置
fn find_code_close(cs: &[char], start: usize, run: usize) -> Option<usize> {
    let mut j = start;
    while j < cs.len() {
        if cs[j] == '`' {
            let r = count_run(cs, j, '`');
            if r == run {
                return Some(j);
            }
            j += r;
            continue;
        }
        j += 1;
    }
    None
}

/// 解析 `[文字](地址)`，返回（文字字符、结束下标）
fn parse_link(cs: &[char], i: usize) -> Option<(Vec<char>, usize)> {
    let n = cs.len();
    if i >= n || cs[i] != '[' {
        return None;
    }

    let mut depth = 0usize;
    let mut j = i;
    let mut text = Vec::new();
    loop {
        if j >= n {
            return None;
        }
        match cs[j] {
            '[' => {
                depth += 1;
                if depth > 1 {
                    text.push(cs[j]);
                }
            }
            ']' => {
                depth -= 1;
                if depth == 0 {
                    j += 1;
                    break;
                }
                text.push(cs[j]);
            }
            ch => text.push(ch),
        }
        j += 1;
    }

    // 紧跟的必须是 `(`
    if j >= n || cs[j] != '(' {
        return None;
    }

    // 找配对的右括号（地址里可能含括号）
    let mut pdepth = 0usize;
    loop {
        if j >= n {
            return None;
        }
        match cs[j] {
            '(' => pdepth += 1,
            ')' => {
                pdepth -= 1;
                if pdepth == 0 {
                    j += 1;
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }

    Some((text, j))
}

/// 解析 `$...$` 或 `$$...$$`
fn parse_math(cs: &[char], i: usize) -> Option<(String, usize)> {
    let n = cs.len();
    if i + 1 >= n {
        return None;
    }

    if cs[i + 1] == '$' {
        let mut j = i + 2;
        while j + 1 < n {
            if cs[j] == '$' && cs[j + 1] == '$' {
                let content: String = cs[i + 2..j].iter().collect();
                if content.trim().is_empty() {
                    return None;
                }
                return Some((content, j + 2));
            }
            j += 1;
        }
        return None;
    }

    // 单 `$`：内容不能以空白开头/结尾，否则更可能是货币符号
    if cs[i + 1].is_whitespace() {
        return None;
    }
    let mut j = i + 1;
    while j < n && cs[j] != '$' {
        j += 1;
    }
    if j >= n || j == i + 1 {
        return None;
    }
    let content: String = cs[i + 1..j].iter().collect();
    if content
        .chars()
        .next_back()
        .map_or(true, |c| c.is_whitespace())
    {
        return None;
    }
    Some((content, j + 1))
}

/// 解析行内 HTML 标签，返回（替换文本、结束下标）
fn parse_html_tag(cs: &[char], i: usize) -> Option<(&'static str, usize)> {
    let n = cs.len();
    let mut j = i + 1;
    let closing = j < n && cs[j] == '/';
    if closing {
        j += 1;
    }

    let name_start = j;
    while j < n && cs[j].is_ascii_alphanumeric() {
        j += 1;
    }
    if j == name_start {
        return None;
    }
    let name: String = cs[name_start..j]
        .iter()
        .collect::<String>()
        .to_ascii_lowercase();

    // 跳过属性直到 `>`
    while j < n && cs[j] != '>' {
        if cs[j] == '<' {
            return None;
        }
        j += 1;
    }
    if j >= n {
        return None;
    }
    let end = j + 1;

    if !HTML_TAG_WHITELIST.contains(&name.as_str()) {
        return None;
    }
    match name.as_str() {
        "br" | "hr" => Some(("\n", end)),
        "p" | "div" => Some(("\n", end)),
        _ => Some(("", end)),
    }
}

/// 解析常见 HTML 实体
fn parse_entity(cs: &[char], i: usize) -> Option<(&'static str, usize)> {
    let n = cs.len();
    let mut j = i + 1;
    while j < n && j - i <= 10 && cs[j] != ';' {
        j += 1;
    }
    if j >= n || cs[j] != ';' {
        return None;
    }
    let name: String = cs[i..=j].iter().collect();
    let replacement = match name.as_str() {
        "&nbsp;" => " ",
        "&amp;" => "&",
        "&lt;" => "<",
        "&gt;" => ">",
        "&quot;" => "\"",
        "&apos;" => "'",
        "&#39;" => "'",
        "&mdash;" => "—",
        "&ndash;" => "–",
        "&hellip;" => "…",
        "&middot;" => "·",
        "&times;" => "×",
        "&divide;" => "÷",
        "&plusmn;" => "±",
        "&copy;" => "©",
        _ => return None,
    };
    Some((replacement, j + 1))
}

/// 解析强调（`**粗**`、`*斜*`、`__粗__`、`_斜_`、`~~删除~~`）
fn parse_emphasis(cs: &[char], i: usize) -> Option<(Vec<char>, usize)> {
    let c = cs[i];
    let run = count_run(cs, i, c);

    // `~` 只认 `~~`；`*` / `_` 认 1–3 个
    let delim = if c == '~' {
        if run < 2 {
            return None;
        }
        2
    } else {
        if run == 0 || run > 3 {
            return None;
        }
        run
    };

    let prev = if i == 0 { None } else { Some(cs[i - 1]) };
    let next = cs.get(i + delim).copied();
    if !can_open(c, prev, next) {
        return None;
    }

    // 查找配对的结束标记
    let mut j = i + delim;
    while j < cs.len() {
        if cs[j] == c {
            let close_run = count_run(cs, j, c);
            if close_run >= delim {
                let close_prev = if j == 0 { None } else { Some(cs[j - 1]) };
                let close_next = cs.get(j + delim).copied();
                if can_close(c, close_prev, close_next) {
                    let content = cs[i + delim..j].to_vec();
                    if content.is_empty() {
                        return None;
                    }
                    return Some((content, j + delim));
                }
            }
            j += close_run.max(1);
            continue;
        }
        j += 1;
    }
    None
}

/// 起始标记判定
///
/// `*` 只要满足左翼即可开启（支持 `*注意*此处` 这类中文紧邻写法）；
/// `_` 额外拒绝词内强调，从而保护 `snake_case` 这类标识符。
fn can_open(delim: char, prev: Option<char>, next: Option<char>) -> bool {
    let left = left_flanking(prev, next);
    if delim != '_' {
        return left;
    }
    let right = right_flanking(prev, next);
    left && (!right || prev.is_some_and(is_punct))
}

/// 结束标记判定
fn can_close(delim: char, prev: Option<char>, next: Option<char>) -> bool {
    let right = right_flanking(prev, next);
    if delim != '_' {
        return right;
    }
    let left = left_flanking(prev, next);
    right && (!left || next.is_some_and(is_punct))
}

/// 左翼：后面不是空白，且（后面不是标点，或前面是空白/边界/标点）
fn left_flanking(prev: Option<char>, next: Option<char>) -> bool {
    let Some(next) = next else {
        return false;
    };
    if next.is_whitespace() {
        return false;
    }
    !is_punct(next) || ws_or_boundary(prev) || prev.is_some_and(is_punct)
}

/// 右翼：前面不是空白，且（前面不是标点，或后面是空白/边界/标点）
fn right_flanking(prev: Option<char>, next: Option<char>) -> bool {
    let Some(prev) = prev else {
        return false;
    };
    if prev.is_whitespace() {
        return false;
    }
    !is_punct(prev) || ws_or_boundary(next) || next.is_some_and(is_punct)
}

/// 左侧为边界或空白
fn ws_or_boundary(c: Option<char>) -> bool {
    c.map_or(true, |c| c.is_whitespace())
}

/// 是否为标点（ASCII 标点或中日韩标点）
fn is_punct(c: char) -> bool {
    c.is_ascii_punctuation() || is_cjk_punctuation(c)
}

/// 中日韩标点（可充当强调定界符的前后边界）
fn is_cjk_punctuation(c: char) -> bool {
    matches!(c,
        '\u{3000}'..='\u{303F}'     // CJK 符号和标点（、。「」等）
        | '\u{FF01}'..='\u{FF20}'   // 全角标点（！＂＃…）
        | '\u{FF3B}'..='\u{FF40}'   // 全角括号与连字符
        | '\u{FF5B}'..='\u{FF65}'   // 全角花括号、竖线、波浪线
    )
}

// ---------------------------------------------------------------------------
// 数学公式
// ---------------------------------------------------------------------------

/// 把 LaTeX 公式内容转成可读文本
///
/// 采用白名单策略：只转换确定含义的命令，未识别的命令原样保留——
/// 留着 `\unknown` 比猜错更有用。
fn math_to_text(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;

    while i < cs.len() {
        let c = cs[i];

        // 命令
        if c == '\\' {
            if i + 1 >= cs.len() {
                out.push('\\');
                break;
            }
            if cs[i + 1] == '\\' {
                out.push('\n');
                i += 2;
                continue;
            }
            if cs[i + 1].is_ascii_alphabetic() {
                let mut name = String::new();
                let mut j = i + 1;
                while j < cs.len() && cs[j].is_ascii_alphabetic() {
                    name.push(cs[j]);
                    j += 1;
                }
                match math_command(&name, &cs, j) {
                    Some((text, next)) => {
                        out.push_str(&text);
                        i = next;
                    }
                    None => {
                        // 未识别：原样保留命令与其 {...} 参数，
                        // 不能把分组花括号当成排版符号删掉
                        out.push('\\');
                        out.push_str(&name);
                        i = j;
                        if i < cs.len() && cs[i] == '{' {
                            if let Some((body, next)) = read_group(&cs, i) {
                                out.push('{');
                                out.push_str(&body);
                                out.push('}');
                                i = next;
                            }
                        }
                    }
                }
                continue;
            }
            // 间距与转义符号
            match cs[i + 1] {
                ',' | ';' | ':' | '!' | ' ' | '\'' => {}
                ch => out.push(ch),
            }
            i += 2;
            continue;
        }

        // 上下标
        if c == '^' || c == '_' {
            if let Some((text, next)) = math_script(cs.as_slice(), i, c) {
                out.push_str(&text);
                i = next;
                continue;
            }
        }

        // 分组花括号不输出
        if c == '{' || c == '}' {
            i += 1;
            continue;
        }

        out.push(c);
        i += 1;
    }

    out
}

/// 处理数学命令；`rest` 为命令名之后的下标
fn math_command(name: &str, cs: &[char], rest: usize) -> Option<(String, usize)> {
    match name {
        "frac" | "dfrac" | "tfrac" => {
            let (a, j) = read_group(cs, rest)?;
            let (b, k) = read_group(cs, j)?;
            let num = wrap_complex(&math_to_text(&a));
            let den = wrap_complex(&math_to_text(&b));
            Some((format!("{num}/{den}"), k))
        }
        "sqrt" => {
            // 可选的根指数 [n] 丢弃
            let mut j = rest;
            if j < cs.len() && cs[j] == '[' {
                while j < cs.len() && cs[j] != ']' {
                    j += 1;
                }
                j += 1;
            }
            let (a, k) = read_group(cs, j)?;
            let inner = math_to_text(&a);
            let body = if inner.chars().count() > 1 {
                format!("({inner})")
            } else {
                inner
            };
            Some((format!("√{body}"), k))
        }
        "text" | "mathrm" | "mathbf" | "mathit" | "operatorname" => {
            let (a, k) = read_group(cs, rest)?;
            Some((a, k))
        }
        "left" | "right" => Some((String::new(), rest)),
        "quad" => Some((" ".to_string(), rest)),
        "qquad" => Some(("  ".to_string(), rest)),
        // 函数名：去掉反斜杠直接保留，`\log n` → `log n`
        "log" | "ln" | "lg" | "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "exp" | "max"
        | "min" | "sup" | "inf" | "lim" | "det" | "gcd" | "mod" | "bmod" | "arg" | "deg" => {
            Some((name.to_string(), rest))
        }
        other => math_symbol(other).map(|s| (s.to_string(), rest)),
    }
}

/// 读取一个 `{...}` 分组，返回（内容、结束后下标）
fn read_group(cs: &[char], i: usize) -> Option<(String, usize)> {
    if i >= cs.len() || cs[i] != '{' {
        return None;
    }
    let mut depth = 1usize;
    let mut j = i + 1;
    let mut s = String::new();
    while j < cs.len() {
        match cs[j] {
            '{' => {
                depth += 1;
                s.push('{');
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((s, j + 1));
                }
                s.push('}');
            }
            ch => s.push(ch),
        }
        j += 1;
    }
    None
}

/// 分子/分母含运算符时补括号，避免 `x+1/2` 这类歧义
fn wrap_complex(s: &str) -> String {
    let needs = s.chars().count() > 1 && s.chars().any(|c| c == '+' || c == '-' || c == ' ');
    if needs {
        format!("({s})")
    } else {
        s.to_string()
    }
}

/// 处理 `^` / `_` 上下标
fn math_script(cs: &[char], i: usize, kind: char) -> Option<(String, usize)> {
    let mut j = i + 1;
    if j >= cs.len() {
        return None;
    }

    let body = if cs[j] == '{' {
        let (content, next) = read_group(cs, j)?;
        j = next;
        content
    } else {
        let ch = cs[j];
        j += 1;
        ch.to_string()
    };

    let chars: Vec<char> = body.chars().collect();
    if chars.len() == 1 {
        let mapped = if kind == '^' {
            superscript(chars[0])
        } else {
            subscript(chars[0])
        };
        if let Some(m) = mapped {
            return Some((m.to_string(), j));
        }
    }

    // 无法映射：保留原始写法（去分组花括号）
    Some((format!("{kind}{body}"), j))
}

/// 单字符上标映射
fn superscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' => '⁻',
        'n' => 'ⁿ',
        'i' => 'ⁱ',
        _ => return None,
    })
}

/// 单字符下标映射
fn subscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' => '₋',
        'n' => 'ₙ',
        'i' => 'ᵢ',
        _ => return None,
    })
}

/// 常见 LaTeX 符号白名单
fn math_symbol(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" | "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "pi" => "π",
        "rho" => "ρ",
        "sigma" => "σ",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" | "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        "times" => "×",
        "div" => "÷",
        "pm" => "±",
        "mp" => "∓",
        "cdot" => "·",
        "ast" => "*",
        "star" => "⋆",
        "circ" => "∘",
        "le" | "leq" => "≤",
        "ge" | "geq" => "≥",
        "ne" | "neq" => "≠",
        "approx" => "≈",
        "equiv" => "≡",
        "sim" => "~",
        "simeq" => "≃",
        "propto" => "∝",
        "ll" => "≪",
        "gg" => "≫",
        "to" | "rightarrow" => "→",
        "leftarrow" => "←",
        "Rightarrow" => "⇒",
        "Leftarrow" => "⇐",
        "leftrightarrow" => "↔",
        "Leftrightarrow" => "⇔",
        "infty" => "∞",
        "sum" => "Σ",
        "prod" => "∏",
        "int" => "∫",
        "iint" => "∬",
        "oint" => "∮",
        "partial" => "∂",
        "nabla" => "∇",
        "in" => "∈",
        "notin" => "∉",
        "ni" => "∋",
        "subset" => "⊂",
        "subseteq" => "⊆",
        "supset" => "⊃",
        "supseteq" => "⊇",
        "cup" => "∪",
        "cap" => "∩",
        "setminus" => "\\",
        "emptyset" | "varnothing" => "∅",
        "forall" => "∀",
        "exists" => "∃",
        "nexists" => "∄",
        "neg" | "lnot" => "¬",
        "land" | "wedge" => "∧",
        "lor" | "vee" => "∨",
        "oplus" => "⊕",
        "otimes" => "⊗",
        "perp" => "⊥",
        "angle" => "∠",
        "parallel" => "∥",
        "cong" => "≅",
        "because" => "∵",
        "therefore" => "∴",
        "cdots" => "⋯",
        "ldots" | "dots" => "…",
        "vdots" => "⋮",
        "ddots" => "⋱",
        "lfloor" => "⌊",
        "rfloor" => "⌋",
        "lceil" => "⌈",
        "rceil" => "⌉",
        "langle" => "⟨",
        "rangle" => "⟩",
        "degree" => "°",
        "prime" => "′",
        "hbar" => "ℏ",
        "ell" => "ℓ",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// 显示宽度
// ---------------------------------------------------------------------------

/// 字符串的等宽显示宽度（全角/宽字符记 2 列）
fn display_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// 单个字符的显示宽度
fn char_width(c: char) -> usize {
    let cp = c as u32;

    // 零宽：组合符号、变体选择符、零宽连接符
    if matches!(
        cp,
        0x0300..=0x036F
            | 0x0483..=0x0489
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x200B..=0x200F
            | 0x20D0..=0x20FF
            | 0xFE00..=0xFE0F
            | 0xFEFF
    ) {
        return 0;
    }

    // 宽字符（East Asian Wide / Fullwidth）
    if matches!(
        cp,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xA960..=0xA97F
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1F64F
            | 0x1F900..=0x1F9FF
            | 0x20000..=0x3FFFD
    ) {
        return 2;
    }

    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_unchanged() {
        assert_eq!(to_plain_text("你好\n世界"), "你好\n世界");
        assert_eq!(to_plain_text("hello world"), "hello world");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(to_plain_text(""), "");
        assert_eq!(to_plain_text("\n\n\n"), "");
        assert_eq!(to_plain_text("   "), "");
    }

    #[test]
    fn atx_heading_stripped() {
        assert_eq!(to_plain_text("## 解题思路"), "解题思路");
        assert_eq!(to_plain_text("# 一\n\n## 二"), "一\n\n二");
        // 结尾井号序列应去掉
        assert_eq!(to_plain_text("## 标题 ##"), "标题");
        // 没有空格分隔的井号不是标题
        assert_eq!(to_plain_text("#include <stdio.h>"), "#include <stdio.h>");
    }

    #[test]
    fn bold_italic_strike_stripped() {
        assert_eq!(to_plain_text("**贪心**即可"), "贪心即可");
        assert_eq!(to_plain_text("这是 __粗__ 与 *斜*"), "这是 粗 与 斜");
        assert_eq!(to_plain_text("~~删除~~掉"), "删除掉");
    }

    #[test]
    fn intraword_underscore_is_not_emphasis() {
        assert_eq!(
            to_plain_text("调用 snake_case_name 变量"),
            "调用 snake_case_name 变量"
        );
        assert_eq!(to_plain_text("_斜体_ 起头"), "斜体 起头");
    }

    #[test]
    fn multiplication_asterisks_are_not_emphasis() {
        assert_eq!(to_plain_text("2 * 3 * 4"), "2 * 3 * 4");
    }

    #[test]
    fn escaped_asterisk_is_literal() {
        assert_eq!(to_plain_text(r"字面 \*星号\*"), "字面 *星号*");
    }

    #[test]
    fn inline_code_is_protected() {
        assert_eq!(to_plain_text("执行 `a_b ** c` 即可"), "执行 a_b ** c 即可");
        assert_eq!(to_plain_text("`# 不是标题`"), "# 不是标题");
    }

    #[test]
    fn fenced_code_block_content_is_protected() {
        let src = "```c\n#include <stdio.h>\nint _var_ = **x;\n```";
        assert_eq!(to_plain_text(src), "#include <stdio.h>\nint _var_ = **x;");
    }

    #[test]
    fn fenced_code_block_keeps_indent() {
        let src = "```python\ndef f():\n    return 1\n```";
        assert_eq!(to_plain_text(src), "def f():\n    return 1");
    }

    #[test]
    fn unterminated_fence_consumes_to_end() {
        assert_eq!(to_plain_text("```\ncode"), "code");
    }

    #[test]
    fn link_keeps_text_drops_url() {
        assert_eq!(
            to_plain_text("见[官方题解](https://x.com/a)。"),
            "见官方题解。"
        );
        assert_eq!(
            to_plain_text("[带**粗体**的链接](https://y.com)"),
            "带粗体的链接"
        );
    }

    #[test]
    fn image_shows_alt() {
        assert_eq!(
            to_plain_text("![示意图](https://x.com/a.png)"),
            "[图片: 示意图]"
        );
        assert_eq!(to_plain_text("![](https://x.com/a.png)"), "[图片]");
    }

    #[test]
    fn unordered_list_uses_bullet() {
        assert_eq!(to_plain_text("- 排序\n- 扫描"), "• 排序\n• 扫描");
        assert_eq!(to_plain_text("* 甲\n+ 乙"), "• 甲\n• 乙");
    }

    #[test]
    fn ordered_list_keeps_number() {
        assert_eq!(to_plain_text("1. 排序\n2. 扫描"), "1. 排序\n2. 扫描");
        assert_eq!(to_plain_text("3) 第三点"), "3. 第三点");
    }

    #[test]
    fn nested_list_indent_preserved() {
        let src = "1. 外层\n   - 内层\n     - 更内层";
        assert_eq!(to_plain_text(src), "1. 外层\n   • 内层\n     • 更内层");
    }

    #[test]
    fn task_list_checkbox() {
        assert_eq!(to_plain_text("- [ ] 未做"), "• [未完成] 未做");
        assert_eq!(to_plain_text("- [x] 已做"), "• [已完成] 已做");
    }

    #[test]
    fn blockquote_marker_stripped() {
        assert_eq!(to_plain_text("> 注意边界"), "注意边界");
        assert_eq!(to_plain_text(">> 嵌套引用"), "嵌套引用");
        // 引用块里的列表按列表处理
        assert_eq!(to_plain_text("> - 项目"), "• 项目");
    }

    #[test]
    fn thematic_break_becomes_rule() {
        let out = to_plain_text("上文\n\n---\n\n下文");
        assert!(out.contains(&"─".repeat(RULE_WIDTH)));
        assert!(out.starts_with("上文"));
        assert!(out.ends_with("下文"));
    }

    #[test]
    fn setext_heading_stripped() {
        assert_eq!(to_plain_text("标题\n==="), "标题");
        assert_eq!(to_plain_text("标题\n---"), "标题");
    }

    #[test]
    fn table_aligns_cjk_columns() {
        let src = "| 输入 | 输出 |\n|------|------|\n| 3 | 6 |";
        assert_eq!(to_plain_text(src), "输入   输出\n────   ────\n3      6");
    }

    #[test]
    fn table_aligns_mixed_width() {
        let src = "| 名称 | n |\n|---|---|\n| abc | 12 |\n| 中文名字 | 3 |";
        // 用显示宽度构造期望值，避免手数空格出错
        let pad = |s: &str, w: usize| format!("{}{}", s, " ".repeat(w - display_width(s)));
        let gap = " ".repeat(TABLE_GAP);
        let expected = [
            format!("{}{gap}n", pad("名称", 8)),
            format!("{}{gap}{}", "─".repeat(8), "─".repeat(3)),
            format!("{}{gap}12", pad("abc", 8)),
            format!("{}{gap}3", pad("中文名字", 8)),
        ]
        .join("\n");
        assert_eq!(to_plain_text(src), expected);
    }

    #[test]
    fn table_without_outer_pipes() {
        let src = "输入 | 输出\n--- | ---\n3 | 6";
        assert_eq!(to_plain_text(src), "输入   输出\n────   ────\n3      6");
    }

    #[test]
    fn table_inline_markup_stripped_in_cells() {
        let src = "| **列** | 值 |\n|---|---|\n| `code` | 1 |";
        // 列宽取该列所有单元格显示宽度的最大值（列甲 2、code 4 → 4；列乙 2、1 → 3）
        let expected = [
            "列".to_string() + &" ".repeat(5) + "值",
            format!(
                "{}{}{}",
                "─".repeat(4),
                " ".repeat(TABLE_GAP),
                "─".repeat(3)
            ),
            "code   1".to_string(),
        ]
        .join("\n");
        assert_eq!(to_plain_text(src), expected);
    }

    #[test]
    fn math_dollar_stripped() {
        assert_eq!(to_plain_text("复杂度 $O(n)$。"), "复杂度 O(n)。");
        assert_eq!(to_plain_text("$$x + y$$"), "x + y");
    }

    #[test]
    fn math_currency_is_not_formula() {
        assert_eq!(to_plain_text("价格 $5 到 $6"), "价格 $5 到 $6");
    }

    #[test]
    fn math_whitelist_converts() {
        assert_eq!(to_plain_text("$a \\times b$"), "a × b");
        assert_eq!(to_plain_text("$x \\le y$"), "x ≤ y");
        assert_eq!(to_plain_text("$\\alpha + \\beta$"), "α + β");
        assert_eq!(to_plain_text("$n \\to \\infty$"), "n → ∞");
    }

    #[test]
    fn math_frac_and_sqrt() {
        assert_eq!(to_plain_text("$\\frac{a}{b}$"), "a/b");
        assert_eq!(to_plain_text("$\\frac{x+1}{2}$"), "(x+1)/2");
        assert_eq!(to_plain_text("$\\sqrt{x}$"), "√x");
        assert_eq!(to_plain_text("$\\sqrt{ab}$"), "√(ab)");
    }

    #[test]
    fn math_superscript_and_subscript() {
        assert_eq!(to_plain_text("$x^2$"), "x²");
        assert_eq!(to_plain_text("$x^{2}$"), "x²");
        assert_eq!(to_plain_text("$a_1$"), "a₁");
        // 多字符指数无法映射，保留原写法
        assert_eq!(to_plain_text("$x^{10}$"), "x^10");
    }

    #[test]
    fn math_function_names_lose_backslash() {
        assert_eq!(to_plain_text("$O(\\log n)$"), "O(log n)");
        assert_eq!(to_plain_text("$\\max(a, b)$"), "max(a, b)");
        assert_eq!(to_plain_text("$\\sin \\theta$"), "sin θ");
    }

    #[test]
    fn math_unknown_command_preserved() {
        assert_eq!(to_plain_text("$\\unknowncmd{x}$"), "\\unknowncmd{x}");
    }

    #[test]
    fn math_text_group_unwrapped() {
        assert_eq!(to_plain_text("$\\text{速度}$"), "速度");
    }

    #[test]
    fn html_br_becomes_newline() {
        assert_eq!(to_plain_text("第一行<br>第二行"), "第一行\n第二行");
    }

    #[test]
    fn html_sup_tag_stripped_content_kept() {
        assert_eq!(to_plain_text("x<sup>2</sup>"), "x2");
    }

    #[test]
    fn html_non_whitelist_tag_is_kept() {
        // `a<b>c` 中的 b 不在白名单，不能被当成 HTML 标签删掉
        assert_eq!(to_plain_text("a<b>c"), "a<b>c");
    }

    #[test]
    fn html_entities_decoded() {
        assert_eq!(to_plain_text("A &amp; B &lt;C&gt;"), "A & B <C>");
    }

    #[test]
    fn blank_lines_collapsed() {
        assert_eq!(to_plain_text("甲\n\n\n\n乙"), "甲\n\n乙");
    }

    #[test]
    fn crlf_normalized() {
        assert_eq!(to_plain_text("甲\r\n乙\r\n"), "甲\n乙");
        assert_eq!(to_plain_text("a\rb"), "a\nb");
    }

    #[test]
    fn tab_expanded_to_spaces() {
        assert_eq!(to_plain_text("a\tb"), "a    b");
        // 代码块内的制表符同样展开：引擎会把 \t 发成 VK_TAB 造成焦点跳转
        assert_eq!(to_plain_text("```\na\tb\n```"), "a    b");
    }

    #[test]
    fn bom_stripped() {
        assert_eq!(to_plain_text("\u{feff}# 标题"), "标题");
    }

    #[test]
    fn trailing_spaces_removed_from_prose() {
        assert_eq!(to_plain_text("文字   "), "文字");
    }

    #[test]
    fn display_width_of_cjk_is_two() {
        assert_eq!(display_width("中文"), 4);
        assert_eq!(display_width("ab"), 2);
        assert_eq!(display_width("中a"), 3);
    }

    /// 设计文档里的完整样例：验证各语法协同工作
    #[test]
    fn full_sample_document() {
        let src = "\
## 解题思路

**贪心**即可，复杂度 $O(n)$。

1. 排序
2. 扫描

> 注意边界

| 输入 | 输出 |
|------|------|
| 3    | 6    |

```python
for i in range(n):
    print(i)
```

见[题解](https://x.com)。
";
        let expected = "\
解题思路

贪心即可，复杂度 O(n)。

1. 排序
2. 扫描

注意边界

输入   输出
────   ────
3      6

for i in range(n):
    print(i)

见题解。";
        assert_eq!(to_plain_text(src), expected);
    }

    /// 对已转换结果再转换一次应保持不变（表格分隔行不用 ASCII 短横正是为此）
    #[test]
    fn conversion_is_idempotent_on_sample_document() {
        let src = "\
# 标题

- 第一项
- 第二项

| 列甲 | 列乙 |
|------|------|
| 1    | 2    |

> 引用
";
        let once = to_plain_text(src);
        let twice = to_plain_text(&once);
        assert_eq!(once, twice, "first={once:?} second={twice:?}");
    }

    /// 一段典型的 AI 中文回答：覆盖行内代码、数学函数名、表格内公式与 CJK 链接
    #[test]
    fn realistic_ai_answer() {
        let src = "\
### 关于二分查找

**时间复杂度**为 $O(\\log n)$，因为每次将区间缩小一半。

关键步骤：

1. 确定 `left` 与 `right` 的初始值
2. 循环条件用 `left <= right`
3. 注意 `mid = left + (right - left) / 2` 防溢出

> 提示：数组必须有序。

| 情况 | 复杂度 |
|------|--------|
| 最好 | $O(1)$ |
| 最坏 | $O(\\log n)$ |

```python
def binary_search(a, target):
    left, right = 0, len(a) - 1
    return -1
```

参考：[二分查找 - 维基百科](https://zh.wikipedia.org/wiki/二分查找)
";
        let expected = "\
关于二分查找

时间复杂度为 O(log n)，因为每次将区间缩小一半。

关键步骤：

1. 确定 left 与 right 的初始值
2. 循环条件用 left <= right
3. 注意 mid = left + (right - left) / 2 防溢出

提示：数组必须有序。

情况   复杂度
────   ────────
最好   O(1)
最坏   O(log n)

def binary_search(a, target):
    left, right = 0, len(a) - 1
    return -1

参考：二分查找 - 维基百科";
        assert_eq!(to_plain_text(src), expected);
    }
}
