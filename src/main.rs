#![allow(unused)]
use std::{
    collections::HashMap,
    env, fs,
    path::PathBuf,
    process::{self, exit},
};

use lexer::{Lexer, PeekableLexer, Token, TokenKind};

pub mod lexer;

fn main() {
    if let Err(err) = run_app() {
        eprintln!("Error: {}", err);
        process::exit(1);
    }
}

fn run_app() -> Result<(), String> {
    let mut args = env::args();

    let program = args
        .next()
        .ok_or_else(|| "Failed to retrieve the program name.".to_string())?;

    let directive = args.next().ok_or_else(|| {
        format!(
            "No directive provided.\nUsage: {} <directive> [FILE]",
            program
        )
    })?;

    let filepath: String = args.next().unwrap_or_else(|| ".proj".into());

    let path = PathBuf::from(&filepath);
    if !path.exists() {
        return Err(format!("File '{}' does not exist.", filepath));
    }

    let blocks =
        parse_file(&filepath).map_err(|e| format!("Failed to parse file '{}': {}", filepath, e))?;

    run_commands(directive, blocks)?;

    Ok(())
}

macro_rules! error {
    ($($arg:tt)*) => {{
        return Err(format!("{}", format_args!($($arg)*)))
    }};
}

fn parse_file(filepath: &str) -> Result<HashMap<String, Block>, String> {
    let source = fs::read_to_string(&filepath).map_err(|err| format!("{err}"))?;
    let mut l = PeekableLexer::new(&source);
    let mut blocks = HashMap::default();

    loop {
        let t = l.next_token();
        if t.is_eof() {
            break;
        }

        match t.kind {
            TokenKind::Identifier => {
                let block_name = t.source;
                let block: Block = parse_block(&mut l)?;
                use std::collections::hash_map::Entry;

                match blocks.entry(block_name.clone()) {
                    Entry::Vacant(e) => {
                        e.insert(block);
                    }
                    Entry::Occupied(_) => {
                        error!("Redefinition of directive '{}'", block_name);
                    }
                }
            }
            _ => {
                error!("Invalid token in top level {t:?}");
            }
        }
    }

    Ok(blocks)
}

fn expect_token(l: &mut PeekableLexer<'_>, kind: TokenKind) -> Result<Token, String> {
    let token = l.next_token();
    if token.kind != kind {
        error!(
            "{} Unexpected token {}, Expect: {:?}",
            token.loc, token.source, kind
        );
    }
    Ok(token)
}

fn parse_block(l: &mut PeekableLexer<'_>) -> Result<Block, String> {
    let mut block = Block::default();
    expect_token(l, TokenKind::OpenBrace)?;
    loop {
        let p = l.peek_token();
        if p.kind == TokenKind::CloseBrace {
            l.next_token();
            break;
        }
        let t = l.next_token();
        match t.kind {
            TokenKind::StringLiteral => {
                block.commands.push(Command::PushStr(t.source));
            }
            TokenKind::Integer => {
                block.commands.push(Command::PushInt(
                    t.source.parse().map_err(|err| format!("{err}"))?,
                ));
            }
            TokenKind::Identifier => match t.source.as_str() {
                "echo" => block.commands.push(Command::Echo),
                "shell" => block.commands.push(Command::Shell),
                "dup" => block.commands.push(Command::Dup),
                "pop" => block.commands.push(Command::Pop),
                "swap" => block.commands.push(Command::Swap),
                "concat" => block.commands.push(Command::Concat),
                "readfile" => block.commands.push(Command::ReadFile),
                "writefile" => block.commands.push(Command::WriteFile),
                _ => error!("Unexpected identifier: {}", t.source),
            },
            _ => todo!(),
        }
    }

    Ok(block)
}

use std::process::Command as SysCommand;
use std::str;

#[derive(Debug, Default)]
struct Block {
    commands: Vec<Command>,
}

#[derive(Debug)]
enum Command {
    /// Run a shell cmd, pop a string from the stack
    /// and push stdout back to the stack
    /// TODO: Add a safety mode for Shell
    Shell,
    /// Push a String onto the stack
    PushStr(String),
    /// Push a i64 onto the stack
    PushInt(i64),
    /// Pop a String from the stack and print it
    Echo,
    /// Duplicates the top value on the stack
    Dup,
    /// Removes the top value from the stack
    Pop,
    /// Swaps the top two stack values
    Swap,
    /// Concatenates top two strings and pushes the result
    Concat,
    /// Reads a file from path on the stack, pushes file contents
    ReadFile,
    /// Writes the top of stack (string) to a file, path below it
    WriteFile,
}

fn run_commands(directive: String, blocks: HashMap<String, Block>) -> Result<(), String> {
    let mut stack = Stack::default();

    let Some(block) = blocks.get(&directive) else {
        error!("Directive '{}' not found.", directive);
    };

    for cmd in &block.commands {
        match cmd {
            Command::PushStr(s) => {
                stack.push(Value::Str(s.clone()));
            }

            Command::PushInt(s) => {
                stack.push(Value::Int(s.clone()));
            }

            Command::Echo => match stack.pop_string() {
                Ok(s) => println!("{}", s),
                Err(err) => error!("{}", err),
            },

            Command::Dup => match stack.top() {
                Some(s) => stack.push(s.clone()),
                None => error!("Dup with a empty stack"),
            },

            Command::Pop => match stack.pop() {
                Some(_) => {}
                None => error!("Pop with a empty stack"),
            },

            Command::Swap => match (stack.pop(), stack.pop()) {
                (Some(a), Some(b)) => {
                    stack.push(b);
                    stack.push(a);
                }
                (None, _) | (_, None) => error!("Swap with less than 2 elements"),
                (None, None) => error!("Swap with a empty stack"),
            },

            Command::Concat => match (stack.pop_string(), stack.pop_string()) {
                (Ok(a), Ok(b)) => {
                    stack.push(Value::Str(b + a.as_str()));
                }
                (Err(err), _) | (_, Err(err)) => {
                    error!("Concat with less than 2 elements or {}", err)
                }
                (Err(err1), Err(err2)) => {
                    error!("Concat with a empty stack: {} and {}", err1, err2)
                }
            },
            Command::ReadFile => todo!("Not sure if i should add this"),
            Command::WriteFile => todo!("Not sure if i should add this"),

            Command::Shell => match stack.pop_string() {
                Ok(cmd) => match SysCommand::new("sh").arg("-c").arg(&cmd).output() {
                    Ok(output) => {
                        if output.status.success() {
                            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            stack.push(Value::Str(stdout));
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            error!("Shell error: {}", stderr);
                        }
                    }
                    Err(e) => {
                        error!("Failed to run shell: {}", e);
                    }
                },
                Err(err) => {
                    error!("{}", err)
                }
            },
        }
    }
    Ok(())
}

#[derive(Debug, Default, Clone)]
pub enum Value {
    #[default]
    Nil,
    Str(String),
    Int(i64),
}

#[derive(Debug, Default)]
pub struct Stack {
    inner: Vec<Value>,
}

impl Stack {
    pub fn top(&self) -> Option<&Value> {
        self.inner.last()
    }

    pub fn pop(&mut self) -> Option<Value> {
        self.inner.pop()
    }

    pub fn pop_string(&mut self) -> Result<String, String> {
        match self.inner.pop() {
            Some(Value::Str(s)) => return Ok(s),
            Some(v) => error!("the value '{v:?}' is not a string."), // TODO: add Value::type_name()
            None => error!("stack is empty."),
        }
    }

    pub fn push(&mut self, v: Value) {
        self.inner.push(v);
    }
}
