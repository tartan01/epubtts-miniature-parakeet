//! 极简 JSON（解析 + 序列化）与通用字符串工具。
//! 不引入 serde 等外部 crate：mini 必须零依赖、离线可构建。

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(v) => v.iter().find(|(n, _)| n == k).map(|(_, x)| x),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let J::Str(s) = self { Some(s) } else { None }
    }
    pub fn as_num(&self) -> Option<f64> {
        match self {
            J::Num(n) => Some(*n),
            J::Str(s) => s.parse().ok(),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            J::Bool(b) => Some(*b),
            J::Str(s) => match s.to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&Vec<J>> {
        if let J::Arr(a) = self { Some(a) } else { None }
    }
    pub fn as_obj(&self) -> Option<&Vec<(String, J)>> {
        if let J::Obj(o) = self { Some(o) } else { None }
    }
    pub fn str(&self, k: &str) -> String { self.get(k).and_then(J::as_str).unwrap_or("").to_string() }
    pub fn num(&self, k: &str) -> f64 { self.get(k).and_then(J::as_num).unwrap_or(0.0) }
    pub fn int(&self, k: &str) -> i64 { self.num(k) as i64 }
    pub fn bool(&self, k: &str) -> bool { self.get(k).and_then(J::as_bool).unwrap_or(false) }
}

pub fn obj(pairs: Vec<(&str, J)>) -> J {
    J::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}
pub fn s(x: &str) -> J { J::Str(x.to_string()) }
pub fn n(x: f64) -> J { J::Num(x) }
pub fn i(x: i64) -> J { J::Num(x as f64) }
pub fn b(x: bool) -> J { J::Bool(x) }
pub fn arr(x: Vec<J>) -> J { J::Arr(x) }
pub fn null() -> J { J::Null }

pub fn esc(x: &str) -> String {
    let mut o = String::with_capacity(x.len() + 2);
    o.push('"');
    for c in x.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

pub fn stringify(j: &J) -> String {
    match j {
        J::Null => "null".into(),
        J::Bool(v) => v.to_string(),
        J::Num(v) => {
            if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", *v as i64) }
            else { format!("{}", v) }
        }
        J::Str(v) => esc(v),
        J::Arr(a) => format!("[{}]", a.iter().map(stringify).collect::<Vec<_>>().join(",")),
        J::Obj(o) => format!("{{{}}}", o.iter()
            .map(|(k, v)| format!("{}:{}", esc(k), stringify(v)))
            .collect::<Vec<_>>().join(",")),
    }
}

pub fn pretty(j: &J) -> String {
    fn rec(j: &J, depth: usize, out: &mut String) {
        let pad = "  ".repeat(depth);
        let pad2 = "  ".repeat(depth + 1);
        match j {
            J::Arr(a) => {
                if a.is_empty() { out.push_str("[]"); return; }
                out.push_str("[\n");
                for (k, v) in a.iter().enumerate() {
                    out.push_str(&pad2);
                    rec(v, depth + 1, out);
                    if k + 1 < a.len() { out.push(','); }
                    out.push('\n');
                }
                out.push_str(&pad); out.push(']');
            }
            J::Obj(o) => {
                if o.is_empty() { out.push_str("{}"); return; }
                out.push_str("{\n");
                for (k, (name, v)) in o.iter().enumerate() {
                    out.push_str(&pad2); out.push_str(&esc(name)); out.push_str(": ");
                    rec(v, depth + 1, out);
                    if k + 1 < o.len() { out.push(','); }
                    out.push('\n');
                }
                out.push_str(&pad); out.push('}');
            }
            _ => out.push_str(&stringify(j)),
        }
    }
    let mut out = String::new();
    rec(j, 0, &mut out);
    out
}

struct P<'a> { b: &'a [u8], i: usize }
impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() { self.i += 1; }
    }
    fn err<T>(&self, m: &str) -> Result<T, String> {
        Err(format!("JSON @{}: {}", self.i, m))
    }
    fn val(&mut self) -> Result<J, String> {
        self.ws();
        if self.i >= self.b.len() { return self.err("意外的结尾"); }
        match self.b[self.i] {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => Ok(J::Str(self.string()?)),
            b't' => { self.lit(b"true")?; Ok(J::Bool(true)) }
            b'f' => { self.lit(b"false")?; Ok(J::Bool(false)) }
            b'n' => { self.lit(b"null")?; Ok(J::Null) }
            b'-' | b'0'..=b'9' => self.number(),
            c => self.err(&format!("非法字符 0x{:02x}", c)),
        }
    }
    fn lit(&mut self, s: &[u8]) -> Result<(), String> {
        if self.b.get(self.i..self.i + s.len()) == Some(s) { self.i += s.len(); Ok(()) }
        else { self.err("字面量不匹配") }
    }
    fn number(&mut self) -> Result<J, String> {
        let st = self.i;
        while self.i < self.b.len() && matches!(self.b[self.i], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E') {
            self.i += 1;
        }
        let t = std::str::from_utf8(&self.b[st..self.i]).unwrap_or("");
        t.parse::<f64>().map(J::Num).map_err(|_| format!("JSON @{}: 非法数字 {}", st, t))
    }
    fn string(&mut self) -> Result<String, String> {
        self.ws();
        if self.b.get(self.i) != Some(&b'"') { return self.err("期望字符串"); }
        self.i += 1;
        let mut out = String::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or("转义截断")?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'), b'\\' => out.push('\\'), b'/' => out.push('/'),
                        b'n' => out.push('\n'), b'r' => out.push('\r'), b't' => out.push('\t'),
                        b'b' => out.push('\u{8}'), b'f' => out.push('\u{c}'),
                        b'u' => {
                            let h = self.hex4()?;
                            if (0xD800..=0xDBFF).contains(&h) && self.b.get(self.i..self.i + 2) == Some(b"\\u") {
                                self.i += 2;
                                let lo = self.hex4()?;
                                let cp: u32 = 0x10000
                                    + (((h as u32).saturating_sub(0xD800)) << 10)
                                    + (lo as u32).saturating_sub(0xDC00);
                                if let Some(ch) = char::from_u32(cp) { out.push(ch); }
                            } else if let Some(ch) = char::from_u32(h as u32) { out.push(ch); }
                        }
                        _ => return self.err("非法转义"),
                    }
                }
                _ => {
                    // 多字节 UTF-8：按字节吸收，最后统一从切片解出字符。
                    let start = self.i - 1;
                    let len = utf8_len(c);
                    self.i = (start + len).min(self.b.len());
                    out.push_str(std::str::from_utf8(&self.b[start..self.i]).unwrap_or(""));
                }
            }
        }
        self.err("字符串未闭合")
    }
    fn hex4(&mut self) -> Result<u16, String> {
        let h = std::str::from_utf8(self.b.get(self.i..self.i + 4).ok_or("unicode 截断")?).unwrap_or("");
        self.i += 4;
        u16::from_str_radix(h, 16).map_err(|_| "非法 unicode".into())
    }
    fn array(&mut self) -> Result<J, String> {
        self.i += 1; let mut a = Vec::new(); self.ws();
        if self.b.get(self.i) == Some(&b']') { self.i += 1; return Ok(J::Arr(a)); }
        loop {
            a.push(self.val()?); self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => { self.i += 1; return Ok(J::Arr(a)); }
                _ => return self.err("数组缺 , 或 ]"),
            }
        }
    }
    fn object(&mut self) -> Result<J, String> {
        self.i += 1; let mut o = Vec::new(); self.ws();
        if self.b.get(self.i) == Some(&b'}') { self.i += 1; return Ok(J::Obj(o)); }
        loop {
            self.ws();
            let k = self.string()?;
            self.ws();
            if self.b.get(self.i) != Some(&b':') { return self.err("对象缺 :"); }
            self.i += 1;
            let v = self.val()?;
            o.push((k, v)); self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => { self.i += 1; return Ok(J::Obj(o)); }
                _ => return self.err("对象缺 , 或 }"),
            }
        }
    }
}
fn utf8_len(c: u8) -> usize {
    if c < 0x80 { 1 } else if c < 0xE0 { 2 } else if c < 0xF0 { 3 } else { 4 }
}

pub fn parse(s: &str) -> Result<J, String> {
    let mut p = P { b: s.as_bytes(), i: 0 };
    let v = p.val()?;
    p.ws();
    if p.i != p.b.len() { return Err(format!("JSON @{}: 结尾有多余内容", p.i)); }
    Ok(v)
}

/// 统一成功/失败信封（与 .NET 主线 CLI/MCP 契约一致）。
pub fn ok(data: J) -> J {
    obj(vec![("ok", b(true)), ("api_version", s("1.0")), ("data", data)])
}
pub fn err(code: &str, msg: &str, fix: &str) -> J {
    obj(vec![
        ("ok", b(false)),
        ("api_version", s("1.0")),
        ("error", obj(vec![
            ("code", s(code)), ("message", s(msg)), ("fix", s(fix)),
            ("retryable", b(true)),
        ])),
    ])
}

pub fn human_bytes(v: u64) -> String {
    if v >= 1073741824 { format!("{:.2} GB", v as f64 / 1073741824.0) }
    else if v >= 1048576 { format!("{:.1} MB", v as f64 / 1048576.0) }
    else if v >= 1024 { format!("{:.1} KB", v as f64 / 1024.0) }
    else { format!("{} B", v) }
}

pub fn stamp() -> String {
    // 不含外部依赖的时间戳：用系统命令 `powershell` 只在需要时调用成本过高，
    // 这里用进程启动以来的单调计数作为日志序号，日期由 CLI 传入。
    format!("{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
}
