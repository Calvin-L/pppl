use crate::syntax::*;
use crate::storage::{Storage,Transaction,StorageError};
use rand::Rng;
use std::fmt::Debug;
use std::collections::BTreeMap;


#[derive(Debug)]
pub enum ExecutionError {
    NotImplemented(& 'static str),
    CannotEvalUnary(UnaryOp, Value),
    CannotEvalBinary(BinaryOp, Value, Value),
    CannotEvalIfOnNonBooleanCond(Value),
    CannotWriteToBoundParameter(String),
    MissingKey(Value, Value),
    StorageFault(StorageError),
    StorageRootSomehowVanished,
}

impl From<StorageError> for ExecutionError {
    fn from(e: StorageError) -> Self {
        ExecutionError::StorageFault(e)
    }
}

type BoundNames = BTreeMap<String, Value>;

fn eval_unary(op: UnaryOp, v: &Value) -> Result<Value, ExecutionError> {
    match (op, v) {
        (UnaryOp::NOT,    Value::Bool(b)) => Ok(Value::Bool(!b)),
        (UnaryOp::NEGATE, Value::Int(i)) => Ok(Value::Int(-i)),
        _ => Err(ExecutionError::CannotEvalUnary(op, v.clone())),
    }
}

fn eval_binary(op: BinaryOp, v1: &Value, v2: &Value) -> Result<Value, ExecutionError> {
    match (op, v1, v2) {
        (BinaryOp::EQ,     _, _) => Ok(Value::Bool(v1 == v2)),
        (BinaryOp::NE,     _, _) => Ok(Value::Bool(v1 != v2)),
        (BinaryOp::LT,     _, _) => Ok(Value::Bool(v1 <  v2)),
        (BinaryOp::LE,     _, _) => Ok(Value::Bool(v1 <= v2)),
        (BinaryOp::GT,     _, _) => Ok(Value::Bool(v1 >  v2)),
        (BinaryOp::GE,     _, _) => Ok(Value::Bool(v1 >= v2)),
        (BinaryOp::AND,    _, _) => Err(ExecutionError::NotImplemented("eval_binary AND")),
        (BinaryOp::OR,     _, _) => Err(ExecutionError::NotImplemented("eval_binary OR")),
        (BinaryOp::PLUS,   Value::Int(x), Value::Int(y)) => Ok(Value::Int(x + y)),
        (BinaryOp::MINUS,  Value::Int(x), Value::Int(y)) => Ok(Value::Int(x - y)),
        (BinaryOp::TIMES,  Value::Int(x), Value::Int(y)) => Ok(Value::Int(x * y)),
        (BinaryOp::DIVIDE, Value::Int(x), Value::Int(y)) => Ok(Value::Int(x / y)),
        (BinaryOp::MOD,    Value::Int(x), Value::Int(y)) => Ok(Value::Int(x % y)),
        (BinaryOp::CONCAT, _, _) => Err(ExecutionError::NotImplemented("eval_binary CONCAT")),
        (BinaryOp::IN,     key, Value::Dict(mapping)) => Ok(Value::Bool(mapping.contains_key(key))),
        (BinaryOp::INDEX,  Value::Dict(mapping), key) => match mapping.get(key) {
            Some(val) => Ok(val.clone()),
            None => Err(ExecutionError::MissingKey(v1.clone(), key.clone())),
        },
        _ => Err(ExecutionError::CannotEvalBinary(op, v1.clone(), v2.clone())),
    }
}

fn eval_ternary<A:Copy + Debug>(op: TernaryOp, e1: &Exp<A>, e2: &Exp<A>, e3: &Exp<A>, env: &Transaction, names: &BoundNames) -> Result<Value, ExecutionError> {
    match (op, eval(e1, env, names)?) {
        (TernaryOp::IF, Value::Bool(b)) => if b { eval(e2, env, names) } else { eval(e3, env, names) },
        (TernaryOp::IF, v1) => Err(ExecutionError::CannotEvalIfOnNonBooleanCond(v1)),
    }
}

fn _eval<A:Copy + Debug>(e: &Exp<A>, env: &Transaction, names: &BoundNames) -> Result<Value, ExecutionError> {
    match e {
        Exp::Root(_) => match env.read_memory(&Vec::new())? {
            Some(root) => Ok(root.clone()),
            None => Err(ExecutionError::StorageRootSomehowVanished),
        },
        Exp::Name(loc, n) => match names.get(n) {
            Some(v) => Ok(v.clone()),
            _ => eval::<A>(&Exp::Binary(*loc, BinaryOp::INDEX, Box::new(Exp::Root(*loc)), Box::new(Exp::Literal(*loc, Value::Blob(str2blob(&n))))), env, names),
        },
        Exp::Literal(_, v) => Ok(v.clone()),
        Exp::Unary(_, op, e1) => eval_unary(*op, &eval::<A>(e1, env, names)?),
        Exp::Binary(_, op, e1, e2) => eval_binary(*op, &eval::<A>(e1, env, names)?, &eval::<A>(e2, env, names)?),
        Exp::Ternary(_, op, e1, e2, e3) => eval_ternary(*op, e1, e2, e3, env, names),
    }
}

pub fn eval<A:Copy + Debug>(e: &Exp<A>, env: &Transaction, names: &BoundNames) -> Result<Value, ExecutionError> {
    let res = _eval(e, env, names);
    // match &res {
    //     Ok(val) => { println!("{:?} ==> {:?}", e, val); },
    //     Err(err) => { println!("{:?} ==> {:?}", e, err); },
    // }
    return res;
}

fn instantiate_params<A:Copy + Debug, F>(params: &Vec<(String, Exp<A>)>, index: usize, env: &Transaction, out: &mut BoundNames, callback: &mut F) where F: FnMut(&BoundNames) -> () {
    if index >= params.len() {
        callback(out);
    } else {
        let (name, exp) = &params[index];
        match eval(exp, env, out) {
            Ok(Value::Dict(m)) => {
                for k in m.keys() {
                    out.insert(name.clone(), k.clone());
                    instantiate_params(params, index + 1, env, out, callback);
                    out.remove(name);
                }
            }
            _ => { }
        }
    }
}

fn find_eligible_blocks<A:Copy + Debug>(m: &Module<A>, env: &Transaction) -> Vec<(usize, BoundNames)> {
    let blocks = &m.blocks;
    let mut res = Vec::new();
    for i in 0 .. blocks.len() {
        let block = &blocks[i];
        instantiate_params(&block.parameters, 0, env, &mut BoundNames::new(), &mut |names| {
            let mut eligible = true;
            for cond in &blocks[i].guards {
                match eval(&cond, env, names) {
                    Ok(Value::Bool(true)) => { }
                    _ => { eligible = false; }
                }
            }
            if eligible {
                res.push((i, names.clone()));
            }
        });
    }
    return res;
}

fn append_in_place<T>(mut v: Vec<T>, x: T) -> Vec<T> {
    v.push(x);
    return v;
}

pub fn eval_lval<A:Copy + Debug>(lv: &LVal<A>, env: &Transaction, names: &BoundNames) -> Result<Vec<Value>, ExecutionError> {
    match lv {
        LVal::Root(_) => Ok(Vec::new()),
        LVal::Name(a, x) =>
            if names.contains_key(x) { Err(ExecutionError::CannotWriteToBoundParameter(x.clone())) }
            else { eval_lval::<A>(&LVal::Index(a.clone(), Box::new(LVal::Root(a.clone())), Box::new(Exp::Literal(a.clone(), Value::Blob(str2blob(&x))))), env, names) }
        LVal::Index(_, x, i) => Ok(append_in_place(eval_lval(&x, env, names)?, eval(i, env, names)?)),
    }
}

pub fn do_assignment(path: &Vec<Value>, new_val: &Value, env: &mut Transaction) -> Result<(), ExecutionError> {
    env.write_memory(path, new_val)?;
    return Ok(());
}

fn exec_block<A:Copy + Debug>(b: &Block<A>, env: &mut Transaction, names: &BoundNames) -> Result<(), ExecutionError> {
    let mut prepped_assignments = Vec::new();
    for (lval, exp) in &b.assignments {
        prepped_assignments.push((
            eval_lval(&lval, env, names)?,
            eval(&exp, env, names)?));
    }
    // TODO: check for aliasing
    for (chain, val) in prepped_assignments {
        do_assignment(&chain, &val, env)?;
    }
    Ok(())
}

pub enum StepOutcome {
    TriggeredBlock(String, BoundNames),
    Deadlock,
}

pub fn sim_step(store: &mut Storage, rng: &mut rand::rngs::ThreadRng) -> Result<StepOutcome, ExecutionError> {
    let mut tx = store.start_transaction()?;
    let code = tx.read_code()?;

    let mut eligible_blocks = find_eligible_blocks(&code, &tx);
    if eligible_blocks.len() == 0 {
        return Ok(StepOutcome::Deadlock);
    }
    // println!("eligible blocks ~~> {}", blocks.len());
    let i: usize = rng.gen_range(0..eligible_blocks.len());
    let (block_index, names) = eligible_blocks.swap_remove(i);
    let block = &code.blocks[block_index];
    exec_block(block, &mut tx, &names)?;
    tx.commit()?;
    return Ok(StepOutcome::TriggeredBlock(block.name.clone(), names));
}
