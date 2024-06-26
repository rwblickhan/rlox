mod chunk;
mod compiler;
mod debug;
mod memory;
mod object_closure;
mod object_function;
mod object_native;
mod object_string;
mod object_upvalue;
mod scanner;
mod value;
mod vm;

use std::fs::File;
use std::io::Write;
use std::{io::Read, process::exit};
use vm::{InterpretResult, VM};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut garbage_collector = memory::Allocator::new();
    let mut vm = VM::new(&mut garbage_collector, true, true);
    if args.len() == 1 {
        repl(&mut vm);
    } else if args.len() == 2 {
        run_file(&mut vm, args[1].as_str());
    } else {
        eprintln!("Usage: clox [path]\n");
        exit(64);
    }
}

fn repl(vm: &mut VM) {
    let mut line = String::new();
    loop {
        print!("> ");
        std::io::stdout().flush().unwrap();

        std::io::stdin()
            .read_line(&mut line)
            .expect("Failed to read line");

        vm.interpret(line.clone());
    }
}

fn run_file(vm: &mut VM, path: &str) {
    let source = read_file(path);
    let result = vm.interpret(source);

    match result {
        InterpretResult::Ok => (),
        InterpretResult::CompileError => exit(65),
        InterpretResult::RuntimeError => exit(70),
    }
}

fn read_file(path: &str) -> String {
    let mut file = File::open(path).unwrap_or_else(|_| panic!("Failed to open file {path}"));
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .unwrap_or_else(|_| panic!("Failed to read contents of {path}"));
    contents
}
