use std::fmt;
use std::marker::PhantomData;

pub struct Lexer<'src> {
    source: &'src str,
    data: &'src [u8],
    pos: usize,
    loc: Loc,
}

pub struct PeekableLexer<'src> {
    pub peeked: Option<Token>,
    pub lexer: Lexer<'src>,
}

impl<'src> PeekableLexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            peeked: None,
            lexer: Lexer::new(source),
        }
    }
    pub fn next_token(&mut self) -> Token {
        self.peeked
            .take()
            .unwrap_or_else(|| self.lexer.next_token())
    }
    pub fn peek_token(&mut self) -> &Token {
        if self.peeked.is_none() {
            self.peeked = Some(self.next_token());
        }
        self.peeked.as_ref().unwrap()
    }
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            data: source.as_bytes(),
            loc: Loc::new(1, 1),
            pos: 0,
        }
    }

    fn advance(&mut self) -> u8 {
        let ch = self.read_char();
        self.pos += 1;
        self.loc.next(ch);
        ch
    }

    fn peek_suffix(&self) -> Option<String> {
        let mut buf = String::new();
        let mut i = self.pos;
        while i < self.data.len() && self.data[i].is_ascii_alphanumeric() {
            buf.push(self.data[i] as char);
            i += 1;
            if buf.len() > 3 {
                break;
            }
        }
        match buf.as_str() {
            "i32" | "u32" | "i64" | "u64" => Some(buf),
            _ => None,
        }
    }

    fn advance_n(&mut self, n: usize) {
        for _ in 0..n {
            self.advance();
        }
    }

    fn read_char(&mut self) -> u8 {
        let pos = self.pos;
        if pos >= self.data.len() {
            0
        } else {
            self.data[pos]
        }
    }

    pub fn next_token(&mut self) -> Token {
        while self.pos <= self.data.len() {
            let begin = self.pos;
            let ch = self.advance();
            let loc = self.loc;
            match ch {
                b'/' if self.read_char() == b'/' => {
                    while self.advance() != b'\n' {}
                    continue;
                }
                b'-' if self.read_char() == b'>' => {
                    self.advance();
                    return Token::new(TokenKind::Arrow, loc, self.source[begin..self.pos].into());
                }
                b'=' if self.read_char() == b'=' => {
                    self.advance();
                    return Token::new(TokenKind::Eq, loc, self.source[begin..self.pos].into());
                }
                b'!' if self.read_char() == b'=' => {
                    self.advance();
                    return Token::new(TokenKind::NotEq, loc, self.source[begin..self.pos].into());
                }
                b'&' if self.read_char() == b'&' => {
                    self.advance();
                    return Token::new(
                        TokenKind::DoubleAmpersand,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b'|' if self.read_char() == b'|' => {
                    self.advance();
                    return Token::new(
                        TokenKind::DoublePipe,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b'.' if self.read_char() == b'*' => {
                    self.advance();
                    return Token::new(TokenKind::Deref, loc, self.source[begin..self.pos].into());
                }
                b'.' if self.read_char() == b'.' && self.read_char() == b'.' => {
                    self.advance();
                    self.advance();
                    return Token::new(TokenKind::Splat, loc, self.source[begin..self.pos].into());
                }
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                    return self.lex_identfier_or_keyword(begin);
                }
                b'0'..=b'9' => return self.lex_number(begin),
                b'"' => return self.lex_string(begin),
                b'@' => return self.lex_macro(begin),

                b',' => {
                    return Token::new(TokenKind::Comma, loc, self.source[begin..self.pos].into());
                }
                b';' => {
                    return Token::new(
                        TokenKind::SemiColon,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b':' => {
                    return Token::new(TokenKind::Colon, loc, self.source[begin..self.pos].into());
                }
                b'=' => {
                    return Token::new(TokenKind::Assign, loc, self.source[begin..self.pos].into());
                }
                b'<' => return Token::new(TokenKind::Lt, loc, self.source[begin..self.pos].into()),
                b'>' => return Token::new(TokenKind::Gt, loc, self.source[begin..self.pos].into()),

                b'!' => {
                    return Token::new(TokenKind::Bang, loc, self.source[begin..self.pos].into());
                }
                b'+' => {
                    return Token::new(TokenKind::Plus, loc, self.source[begin..self.pos].into());
                }
                b'-' => {
                    return Token::new(TokenKind::Minus, loc, self.source[begin..self.pos].into());
                }
                b'*' => {
                    return Token::new(
                        TokenKind::Asterisk,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b'/' => {
                    return Token::new(TokenKind::Slash, loc, self.source[begin..self.pos].into());
                }
                b'%' => {
                    return Token::new(TokenKind::Mod, loc, self.source[begin..self.pos].into());
                }
                b'$' => {
                    return Token::new(TokenKind::Dollar, loc, self.source[begin..self.pos].into());
                }
                b'&' => {
                    return Token::new(
                        TokenKind::Ampersand,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b'|' => {
                    return Token::new(TokenKind::Pipe, loc, self.source[begin..self.pos].into());
                }
                b'(' => {
                    return Token::new(
                        TokenKind::OpenParen,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b')' => {
                    return Token::new(
                        TokenKind::CloseParen,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b'[' => {
                    return Token::new(
                        TokenKind::OpenSquare,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b']' => {
                    return Token::new(
                        TokenKind::CloseSquare,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b'{' => {
                    return Token::new(
                        TokenKind::OpenBrace,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b'}' => {
                    return Token::new(
                        TokenKind::CloseBrace,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                ch if ch.is_ascii_whitespace() => continue,
                0 => return Token::new(TokenKind::EOF, self.loc, "\0".into()),
                _ => {
                    return Token::new(
                        TokenKind::Invalid,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
            }
        }
        Token::new(
            TokenKind::EOF,
            self.loc,
            self.source[self.pos..self.pos].into(),
        )
    }

    fn lex_identfier_or_keyword(&mut self, begin: usize) -> Token {
        let loc = self.loc;
        loop {
            let ch = self.read_char();
            match ch {
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => (),
                b'0'..=b'9' => (),
                _ => break,
            }
            self.advance();
        }
        Token::new(
            TokenKind::Identifier,
            loc,
            self.source[begin..self.pos].into(),
        )
    }

    fn lex_number(&mut self, begin: usize) -> Token {
        let loc = self.loc();
        let mut kind = TokenKind::Integer;

        while let b'0'..=b'9' = self.read_char() {
            self.advance();
        }

        let suffix_start = self.pos;
        let suffix = self.peek_suffix();

        match suffix.as_deref() {
            Some("i32") => {
                kind = TokenKind::IntegerNumber;
                self.advance_n(3);
            }
            Some("u32") => {
                kind = TokenKind::UnsignedIntegerNumber;
                self.advance_n(3);
            }
            Some("i64") => {
                kind = TokenKind::LongIntegerNumber;
                self.advance_n(3);
            }
            Some("u64") => {
                kind = TokenKind::LongUnsignedIntegerNumber;
                self.advance_n(3);
            }
            Some(_) => {
                return Token::new(TokenKind::Invalid, loc, self.source[begin..self.pos].into());
            }
            None => (),
        }

        Token::new(kind, loc, self.source[begin..suffix_start].into())
    }

    // TODO: Fix the lexer. `lex_string` should handle escape sequences or RawModule should handle escape sequences
    fn lex_string(&mut self, begin: usize) -> Token {
        let mut buffer = String::new();
        let kind = TokenKind::StringLiteral;
        let loc = self.loc();
        loop {
            let ch = self.read_char();
            match ch {
                b'"' => {
                    self.advance();
                    break;
                }
                b'\0' => {
                    return Token::new(
                        TokenKind::Invalid,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b'\\' => {
                    self.advance();
                    let ch = self.read_char();
                    match ch {
                        b'r' => buffer.push('\r'),
                        b'"' => buffer.push('"'),
                        b'\'' => buffer.push('\''),
                        b'n' => buffer.push('\n'),
                        b'\\' => buffer.push('\\'),
                        b'0' => buffer.push('\0'),
                        _ => {
                            return Token::new(
                                TokenKind::Invalid,
                                loc,
                                self.source[begin..self.pos].into(),
                            );
                        }
                    }
                }
                _ => buffer.push(ch as char),
            }
            self.advance();
        }
        Token::new(kind, loc, buffer)
    }

    pub fn loc(&self) -> Loc {
        self.loc
    }

    fn lex_macro(&mut self, begin: usize) -> Token {
        let mut buffer = String::new();
        let mut kind = TokenKind::Macro;
        let loc = self.loc();
        loop {
            let ch = self.read_char();
            match ch {
                _ if ch.is_ascii_whitespace() => {
                    self.advance();
                    break;
                }
                b'(' => {
                    kind = TokenKind::MacroWithArgs;
                    buffer.push(ch as char);
                    self.advance();
                    loop {
                        let ch = self.read_char();
                        buffer.push(ch as char);
                        if ch == b')' {
                            self.advance();
                            break;
                        }
                        self.advance();
                    }
                    break;
                }
                b'\0' => {
                    return Token::new(
                        TokenKind::Invalid,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                _ => buffer.push(ch as char),
            }
            self.advance();
        }
        Token::new(kind, loc, buffer)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Span<T> {
    _marker: PhantomData<T>,
    pub loc: Loc,
    pub start: usize,
    pub end: usize,
}

impl<T> Span<T> {
    pub fn to_span<E>(&self) -> Span<E> {
        Span {
            _marker: PhantomData,
            loc: self.loc,
            start: self.start,
            end: self.end,
        }
    }
}

impl<T> Span<T> {
    pub fn new(loc: Loc, start: usize, end: usize) -> Self {
        Self {
            _marker: PhantomData,
            loc,
            start,
            end,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub loc: Loc,
    pub source: String,
}

impl Token {
    pub fn new(kind: TokenKind, loc: Loc, source: String) -> Self {
        Self { kind, loc, source }
    }

    pub fn is_eof(&self) -> bool {
        matches!(self.kind, TokenKind::EOF)
    }

    pub fn is_invalid(&self) -> bool {
        matches!(self.kind, TokenKind::Invalid)
    }

    pub fn is_macro(&self) -> bool {
        matches!(self.kind, TokenKind::Macro)
    }

    pub fn is_ident(&self) -> bool {
        matches!(self.kind, TokenKind::Identifier)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    #[default]
    EOF,
    Invalid,

    OpenParen,
    CloseParen,
    OpenSquare,
    CloseSquare,
    OpenBrace,
    CloseBrace,

    Macro,
    MacroWithArgs,

    Identifier,
    Keyword,

    RealNumber,
    Integer,
    IntegerNumber,
    LongIntegerNumber,
    UnsignedIntegerNumber,
    LongUnsignedIntegerNumber,
    StringLiteral,
    CharacterLiteral,

    Dot,
    Splat,
    Comma,
    Colon,
    SemiColon,
    Arrow,

    Assign,
    Bang,
    Plus,
    Minus,
    Asterisk,
    Slash,
    Eq,
    NotEq,
    Gt,
    Lt,
    Mod,
    Ampersand,
    Pipe,
    DoubleAmpersand,
    DoublePipe,

    Dollar,
    Deref,
}
impl TokenKind {
    pub fn is_binop(&self) -> bool {
        use TokenKind::*;
        matches!(
            self,
            Assign
                | Plus
                | Minus
                | Asterisk
                | Slash
                | Eq
                | NotEq
                | Gt
                | Lt
                | Mod
                | Ampersand
                | Pipe
                | DoubleAmpersand
                | DoublePipe
        )
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Loc {
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

impl Loc {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    pub fn next_column(&mut self) {
        self.col += 1;
    }

    pub fn next_line(&mut self) {
        self.line += 1;
        self.col = 1;
    }

    pub fn next(&mut self, c: u8) {
        match c {
            b'\n' => self.next_line(),
            b'\t' => {
                let ts = 8;
                self.col = (self.col / ts) * ts + ts;
            }
            c if c.is_ascii_control() => {}
            _ => self.next_column(),
        }
    }
}
