use clap::Parser;
use lexer::{PeekableLexer, Token, TokenKind};
use std::collections::HashSet;
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

    match cli.command {
        cli::Command::Run { directive } => {
            if cli.verbose {
                println!("Running directive: {}", directive);
            }

            if let Err(e) = run_commands(directive, blocks) {
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
                let block: Block = parse_block(&mut l, &blocks)?;

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

fn parse_block(
    l: &mut PeekableLexer<'_>,
    blocks: &HashMap<String, Block>,
) -> Result<Block, String> {
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
                "readfile" => block.commands.push(Command::ReadFile),
                "writefile" => block.commands.push(Command::WriteFile),

                "concat" => block.commands.push(Command::Concat),

                "not" => block.commands.push(Command::Not),

                "dup" => block.commands.push(Command::Dup),
                "pop" => block.commands.push(Command::Pop),
                "swap" => block.commands.push(Command::Swap),

                "exit" => block.commands.push(Command::Exit),
                "debug" => block.commands.push(Command::Debug),
                "if" => {
                    let inner = parse_block(l, blocks)?;
                    block.deps.extend(inner.deps.into_iter());
                    block.commands.push(Command::If(inner.commands));
                }
                "while" => {
                    let inner = parse_block(l, blocks)?;
                    block.deps.extend(inner.deps.into_iter());
                    block.commands.push(Command::While(inner.commands));
                }
                "call" => {
                    let id_token = expect_token(l, TokenKind::Identifier)?;
                    block.deps.push(id_token.source.clone());
                    block.commands.push(Command::Call(id_token.source));
                }
                "let" => {
                    let id_token = expect_token(l, TokenKind::Identifier)?;
                    block.commands.push(Command::Store(id_token.source));
                }
                _ => {
                    block.commands.push(Command::LoadVar(t.source));
                }
            },
            TokenKind::MacroCall => {
                if let Some(m) = blocks.get(t.source.as_str()) {
                    block.commands.extend(m.commands.clone().into_iter());
                } else {
                    error!("Unexpected macro: {}", t.source)
                }
            }
            _ => error!("Unexpected Token: {:?} '{}'", t.kind, t.source),
        }
    }

    Ok(block)
}

use std::process::Command as SysCommand;
use std::str;

#[derive(Debug, Default)]
struct ExecutionEnv {
    stack: Stack,
    vars: HashMap<String, Value>,
}

#[derive(Debug, Default)]
struct Block {
    deps: Vec<String>,
    commands: Vec<Command>,
}

#[derive(Debug, Clone)]
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
    /// Logical not
    Not,
    /// Reads a file from path on the stack, pushes file contents
    ReadFile,
    /// Writes the top of stack (string) to a file, path below it
    WriteFile,
    /// Check if the top of stack is true and execute the block
    If(Vec<Command>),
    /// While
    While(Vec<Command>),
    /// Call another block
    Call(String),
    /// Exit the program
    Exit,
    /// Prints the current stack
    Debug,
    /// Store
    Store(String),
    /// Load
    LoadVar(String),
}

fn resolve_dependencies(blocks: &HashMap<String, Block>, directive: &str) -> Result<(), String> {
    let mut seen = HashSet::new();
    let mut ordered = Vec::new();

    fn resolve_dependencies_impl<'same>(
        blocks: &'same HashMap<String, Block>,
        directive: &'same str,
        seen: &mut HashSet<&'same str>,
        ordered: &mut Vec<String>,
    ) -> Result<(), String> {
        let seen_contains = seen.contains(directive);
        if seen_contains && ordered.iter().find(|o| o.as_str() == directive).is_none() {
            error!("Circular dependency detected at '{directive}'")
        }

        if seen_contains {
            return Ok(());
        }
        seen.insert(directive);

        match blocks.get(directive) {
            Some(b) => {
                for dep in b.deps.iter() {
                    resolve_dependencies_impl(blocks, dep, seen, ordered)?;
                }
                ordered.push(directive.into());
                Ok(())
            }
            None => error!("Directive '{directive}' not found."),
        }
    }
    resolve_dependencies_impl(blocks, directive, &mut seen, &mut ordered)?;
    Ok(())
}

fn run_commands(directive: String, blocks: HashMap<String, Block>) -> Result<(), String> {
    let mut env = ExecutionEnv::default();

    let Some(block) = blocks.get(&directive) else {
        error!("Directive '{}' not found.", directive);
    };

    resolve_dependencies(&blocks, &directive)?;

    for cmd in &block.commands {
        run_cmd(cmd, &mut env, &blocks)?;
    }
    Ok(())
}

fn run_cmd(
    cmd: &Command,
    env: &mut ExecutionEnv,
    blocks: &HashMap<String, Block>,
) -> Result<(), String> {
    match cmd {
        Command::Debug => {
            println!("DEBUG {:?}", env.stack.inner)
        }

        Command::PushStr(s) => {
            env.stack.push(Value::Str(s.clone()));
        }

        Command::PushInt(s) => {
            env.stack.push(Value::Int(*s));
        }

        Command::Echo => {
            let msg: String = env.stack.pop()?.try_into()?;
            println!("{msg}")
        }

        Command::Dup => match env.stack.top() {
            Some(s) => env.stack.push(s.clone()),
            None => error!("Dup with a empty stack"),
        },

        Command::Pop => {
            env.stack.pop()?;
        }

        Command::Swap => {
            let a = env.stack.pop()?;
            let b = env.stack.pop()?;
            env.stack.push(a);
            env.stack.push(b);
        }

        Command::Concat => {
            let b: String = env.stack.pop()?.try_into()?;
            let a: String = env.stack.pop()?.try_into()?;
            env.stack.push(Value::Str(a + b.as_str()));
        }

        Command::Not => {
            let a: bool = env.stack.pop()?.try_into()?;
            env.stack.push(Value::Bool(!a));
        }

        Command::ReadFile => {
            let path: String = env.stack.pop()?.try_into()?;
            match fs::read_to_string(&path) {
                Ok(content) => env.stack.push(Value::Str(content)),
                Err(e) => error!("readfile error: {}", e),
            }
        }

        Command::WriteFile => {
            let content: String = env.stack.pop()?.try_into()?;
            let path: String = env.stack.pop()?.try_into()?;
            match fs::write(&path, content) {
                Ok(_) => {}
                Err(e) => error!("writefile error: {}", e),
            }
        }

        Command::Exit => {
            let code: i64 = env.stack.pop()?.try_into()?;
            process::exit(code as i32);
        }

        Command::If(cmds) => {
            let cond: bool = env.stack.pop()?.try_into()?;

            if cond {
                for cmd in cmds {
                    run_cmd(cmd, env, blocks)?;
                }
            }
        }

        Command::While(cmds) => loop {
            let cond: bool = env.stack.pop()?.try_into()?;

            if !cond {
                break;
            }
            for cmd in cmds {
                run_cmd(cmd, env, blocks)?;
            }
        },

        Command::Call(block_name) => {
            let Some(b) = blocks.get(block_name) else {
                error!("call block '{}' not found", block_name);
            };
            for cmd in &b.commands {
                run_cmd(cmd, env, blocks)?;
            }
        }

        Command::Store(var) => {
            let v = env.stack.pop()?;
            env.vars.insert(var.clone(), v);
        }

        Command::LoadVar(var) => {
            let v = env.vars.get(var).cloned().unwrap();
            env.stack.push(v);
        }

        Command::Shell => {
            let cmd: String = env.stack.pop()?.try_into()?;
            match SysCommand::new("sh").arg("-c").arg(&cmd).output() {
                Ok(output) => {
                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        println!("Shell -> '{cmd}'");
                        env.stack.push(Value::Str(stdout));
                        env.stack.push(Value::Bool(true));
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        println!("Shell -> '{cmd}'");
                        env.stack.push(Value::Str(stderr));
                        env.stack.push(Value::Bool(false));
                    }
                }
                Err(e) => {
                    error!("Failed to run shell: {}", e);
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Value {
    #[default]
    Nil,
    Str(String),
    Int(i64),
    Bool(bool),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "Nil",
            Value::Str(_) => "Str",
            Value::Int(_) => "Int",
            Value::Bool(_) => "Bool",
        }
    }
}

impl TryFrom<Value> for String {
    type Error = String;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Str(s) => Ok(s),
            v => Err(format!("expected string but got {}", v.type_name())),
        }
    }
}

impl TryFrom<Value> for i64 {
    type Error = String;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Int(s) => Ok(s),
            v => Err(format!("expected int but got {}", v.type_name())),
        }
    }
}

impl TryFrom<Value> for bool {
    type Error = String;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bool(s) => Ok(s),
            v => Err(format!("expected bool but got {}", v.type_name())),
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

    pub fn pop(&mut self) -> Result<Value, String> {
        match self.inner.pop() {
            Some(v) => Ok(v),
            None => error!("stack is empty."),
        }
    }

    pub fn push(&mut self, v: Value) {
        self.inner.push(v);
    }
}
