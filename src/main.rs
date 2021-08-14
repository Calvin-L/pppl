mod syntax;
mod eval;
mod storage;

use storage::{Storage, Transaction};
use parse::{ModuleParser, ExpParser, AssignParser};
use std::fs;
use rand;
use clap::{App, Arg, SubCommand};
use std::collections::BTreeMap;

#[macro_use] extern crate lalrpop_util;
lalrpop_mod!(pub parse); // synthesized by LALRPOP

fn load(mut tx: Transaction, filename: &str, code: &str) {
    ModuleParser::new().parse(code).unwrap(); // check parseability
    tx.replace_code(code).unwrap();
    tx.commit().unwrap();
    println!("Loaded {}", filename);
}

fn run(storage: &mut Storage) {
    let mut rng = rand::thread_rng();
    loop {
        match eval::sim_step(storage, &mut rng) {
            Ok(eval::StepOutcome::Deadlock) => {
                println!("deadlock");
            }
            Ok(eval::StepOutcome::TriggeredBlock(name, args)) => {
                if args.is_empty() {
                    println!("triggered: `{}`", name);
                } else {
                    println!("triggered: `{}` with arguments {:?}", name, args);
                }
            }
            Err(e) => {
                println!("fault: {:?}", e);
            }
        }
    }
}

fn main() {
    let matches = App::new("ppppl")
        .about("Interface to the Persistent Parallel Programming Language")
        .subcommand(SubCommand::with_name("load")
            .arg(Arg::with_name("INPUT")
                .help("The input file to load")
                .required(true)
                .index(1)))
        .subcommand(SubCommand::with_name("run")
            .arg(Arg::with_name("INPUT")
                .help("An input file to load before running")
                .required(false)
                .index(1)))
        .subcommand(SubCommand::with_name("read")
            .arg(Arg::with_name("EXPR")
                .help("The expression to evaluate")
                .required(true)
                .index(1)))
        .subcommand(SubCommand::with_name("write")
            .arg(Arg::with_name("ASSIGNMENT")
                .help("An assignment statement to evaluate")
                .required(true)
                .index(1)))
        .get_matches();

    let no_bound_names = BTreeMap::new();

    if let Some(load_args) = matches.subcommand_matches("load") {
        let mut s = Storage::open().unwrap();
        let filename = load_args.value_of("INPUT").unwrap();
        load(
            s.start_transaction().unwrap(),
            &filename,
            &fs::read_to_string(filename).unwrap());
    } else if let Some(run_args) = matches.subcommand_matches("run") {
        let mut s = Storage::open().unwrap();
        if let Some(filename) = run_args.value_of("INPUT") {
            load(
                s.start_transaction().unwrap(),
                &filename,
                &fs::read_to_string(filename).unwrap());
        }
        run(&mut s);
    } else if let Some(read_args) = matches.subcommand_matches("read") {
        let mut s = Storage::open().unwrap();
        let e = ExpParser::new().parse(read_args.value_of("EXPR").unwrap()).unwrap();
        let tx = s.start_transaction().unwrap();
        let res = eval::eval(&e, &tx, &no_bound_names).unwrap();
        println!("{}", res);
    } else if let Some(write_args) = matches.subcommand_matches("write") {
        let mut s = Storage::open().unwrap();
        let (lval, e) = AssignParser::new().parse(write_args.value_of("ASSIGNMENT").unwrap()).unwrap();
        let mut tx = s.start_transaction().unwrap();
        eval::do_assignment(
            &eval::eval_lval(&lval, &tx, &no_bound_names).unwrap(),
            &eval::eval(&e, &tx, &no_bound_names).unwrap(),
            &mut tx).unwrap();
        tx.commit().unwrap();
        println!("So it is.");
    } else {
        println!("You didn't tell me anything to do.");
    }

}
