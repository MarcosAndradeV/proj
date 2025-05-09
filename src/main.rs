use clap::Parser;
use lexer::{PeekableLexer, Token, TokenKind};
use std::path::Path;
use std::{collections::HashMap, fs, process};

pub mod cli;
pub mod lexer;

fn main() {
    let cli = cli::Cli::parse();

    if !cli.file.exists() {
        eprintln!("File '{}' does not exist.", cli.file.display());
        process::exit(1);
    }

    let blocks = match parse_file(&cli.file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    };

    match &cli.command {
        cli::Command::Run { directive } => {
            if cli.verbose {
                println!("Running directive: {}", directive);
            }

            if let Err(e) = run_commands(directive.clone(), blocks) {
                eprintln!("Execution error: {}", e);
                process::exit(1);
            }
        }
        cli::Command::List => {
            println!("Available directives:");
            for name in blocks.keys() {
                println!("- {}", name);
            }
        }
    }
}

macro_rules! error {
    ($($arg:tt)*) => {{
        return Err(format!("{}", format_args!($($arg)*)))
    }};
}

fn parse_file<P: AsRef<Path>>(filepath: P) -> Result<HashMap<String, Block>, String> {
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
                "if" => {
                    let inner = parse_block(l)?;
                    block.commands.push(Command::If(inner.commands));
                }
                "load" => {
                    let id_token = expect_token(l, TokenKind::Identifier)?;
                    block.commands.push(Command::Load(id_token.source));
                }
                _ => error!("Unexpected identifier: {}", t.source),
            },
            TokenKind::Macro => match t.source.as_str() {
                "log_shell" => block
                    .commands
                    .extend([Command::Dup, Command::Echo, Command::Shell]),
                _ => error!("Unexpected macro: {}", t.source),
            },
            _ => error!("Unexpected Token: {:?} '{}'", t.kind, t.source),
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
    // Check if the top of stack is true and execute the block
    If(Vec<Command>),
    // Load another block
    Load(String),
}

fn run_commands(directive: String, blocks: HashMap<String, Block>) -> Result<(), String> {
    let mut stack = Stack::default();

    let Some(block) = blocks.get(&directive) else {
        error!("Directive '{}' not found.", directive);
    };

    for cmd in &block.commands {
        run_cmd(&mut stack, &blocks, cmd)?;
    }
    Ok(())
}

fn run_cmd(
    stack: &mut Stack,
    blocks: &HashMap<String, Block>,
    cmd: &Command,
) -> Result<(), String> {
    match cmd {
        Command::PushStr(s) => {
            stack.push(Value::Str(s.clone()));
        }

        Command::PushInt(s) => {
            stack.push(Value::Int(*s));
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
        },

        Command::Concat => match (stack.pop_string(), stack.pop_string()) {
            (Ok(a), Ok(b)) => {
                stack.push(Value::Str(b + a.as_str()));
            }
            (Err(err), _) | (_, Err(err)) => {
                error!("Concat with less than 2 elements or {}", err)
            }
        },

        Command::ReadFile => match stack.pop_string() {
            Ok(path) => match fs::read_to_string(&path) {
                Ok(content) => stack.push(Value::Str(content)),
                Err(e) => error!("readfile error: {}", e),
            },
            Err(e) => error!("{}", e),
        },

        Command::WriteFile => {
            let content = stack.pop_string()?;
            let path = stack.pop_string()?;
            match fs::write(&path, content) {
                Ok(_) => {}
                Err(e) => error!("writefile error: {}", e),
            }
        }

        Command::If(cmds) => {
            let cond = match stack.pop() {
                Some(Value::Int(v)) => v,
                Some(v) => error!("if expected Int on stack, got {}", v.type_name()),
                None => error!("if with empty stack"),
            };

            if cond != 0 {
                for cmd in cmds {
                    run_cmd(stack, blocks, cmd)?;
                }
            }
        }

        Command::Load(block_name) => {
            let Some(b) = blocks.get(block_name) else {
                error!("load: block '{}' not found", block_name);
            };

            for cmd in &b.commands {
                run_cmd(stack, blocks, cmd)?;
            }
        }

        Command::Shell => match stack.pop_string() {
            Ok(cmd) => match SysCommand::new("sh").arg("-c").arg(&cmd).output() {
                Ok(output) => {
                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        println!("Shell '{cmd}'\n{}", stdout)
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        error!("Shell '{cmd}'\nstderr:\n{}", stderr);
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
    Ok(())
}

#[derive(Debug, Default, Clone)]
pub enum Value {
    #[default]
    Nil,
    Str(String),
    Int(i64),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "Nil",
            Value::Str(_) => "Str",
            Value::Int(_) => "Int",
        }
    }
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
            Some(Value::Str(s)) => Ok(s),
            Some(v) => error!("expected string but got {}", v.type_name()),
            None => error!("stack is empty."),
        }
    }

    pub fn push(&mut self, v: Value) {
        self.inner.push(v);
    }
}
