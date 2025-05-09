# Proj Project manager

# 📄 `.proj` File Format Specification & Runtime Documentation

## 🔧 Overview

`.proj` is a **stack-based scripting file format** used to describe command pipelines. It is interpreted by a Rust CLI that supports directives (top-level named blocks), each containing commands to manipulate a runtime stack, interact with the file system, print output, and run shell commands.

---

## 🗂️ File Structure

A `.proj` file is composed of **named blocks**, each associated with a directive. Each block is a set of **commands** enclosed in `{ ... }`.

### ✅ Syntax

```proj
main {
  "Hello,"
  " world!"
  concat
  echo
}
```

### ✅ Rules

* Each **block** starts with an identifier (directive name) followed by `{`.
* Commands inside blocks are interpreted in order.
* Duplicate block names are disallowed.

---

## 🔧 Commands

Each block contains a sequence of the following commands:

| Command      | Stack Behavior                                             |
| ------------ | ---------------------------------------------------------- |
| `"string"`   | Push a string to the stack                                 |
| `123`        | Push an integer to the stack                               |
| `echo`       | Pop a string and print it to stdout                        |
| `dup`        | Duplicate the top stack element                            |
| `pop`        | Remove the top stack element                               |
| `swap`       | Swap the top two elements                                  |
| `concat`     | Pop two strings, concatenate, and push result              |
| `readfile`   | Pop a file path string, read the file, and push contents   |
| `writefile`  | Pop (content, path) strings and write to file              |
| `if { ... }` | Pop an integer. If non-zero, execute enclosed block        |
| `load name`  | Load another block named `name` and execute its commands   |
| `shell`      | Pop a command string, run it with `sh -c`, and push output |

### 🧠 Macros

* `log_shell`: Expands to `dup`, `echo`, and `shell`.

---

## 📁 Example

```proj
main {
  "echo 'Hello from Shell'"
  log_shell
}
```

This will:

1. Push the shell command string.
2. Duplicate it.
3. Echo it to stdout.
4. Run it with the shell and print the result.

---

## 🛠️ Tooling CLI

Usage:

```sh
CLI tool to run .proj scripting files

Usage: proj [OPTIONS] <COMMAND>

Commands:
  run   Run a directive
  list  List all available directives
  help  Print this message or the help of the given subcommand(s)

Options:
  -f, --file <FILE>  Path to the .proj file [default: .proj]
  -v, --verbose      Activate verbose output
  -h, --help         Print help
  -V, --version      Print version
```
