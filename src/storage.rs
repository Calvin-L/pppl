// Durable storage stuff.

use crate::syntax::{SourceSpan,Module,Value};
use crate::parse::ModuleParser;

use lalrpop_util::ParseError;
use sqlite;
use home;
use std::fmt::Debug;
use std::collections::BTreeMap;
use bincode;


#[derive(Debug)]
pub enum StorageError {
    UnderlyingError(sqlite::Error),
    UnknownSchemaVersion(i64),
    NoHomeDirectory,
    SourceCodeIsCorrupt(String),
    MemoryIsCorrupt(String),
}

impl From<sqlite::Error> for StorageError {
    fn from(err: sqlite::Error) -> StorageError {
        if err.code == Some(SQLITE_BUSY) {
            panic!("err={:?}", err);
        }
        StorageError::UnderlyingError(err)
    }
}

impl <A: Debug, B: Debug, C: Debug> From<ParseError<A, B, C>> for StorageError {
    fn from(err: ParseError<A, B, C>) -> StorageError {
        StorageError::SourceCodeIsCorrupt(format!("{:?}", err))
    }
}

impl From<Box<bincode::ErrorKind>> for StorageError {
    fn from(err: Box<bincode::ErrorKind>) -> StorageError {
        StorageError::MemoryIsCorrupt(format!("{:?}", err))
    }
}

pub struct Storage {
    // NOTE: SQLite connections can't be used by multiple threads.  So, we'll
    // open a new connection for each transaction.
}

pub struct Transaction {
    conn: sqlite::Connection,
    mem: Value,
    mem_changed: bool,
}

impl Storage {

    pub fn open() -> Result<Storage, StorageError> {
        let mut s = Storage { };
        let tx = s.start_transaction()?;
        tx.conn.execute("CREATE TABLE IF NOT EXISTS clocks(name TEXT PRIMARY KEY, value INT) WITHOUT ROWID;")?;

        let schema_name = "schema_version";
        let mut stm = tx.conn.prepare("SELECT value FROM clocks WHERE name=?;")?;
        stm.bind(1, schema_name)?;
        let mut schema_version = 0;
        while let sqlite::State::Row = stm.next()? {
            schema_version = stm.read::<i64>(0)?;
        }

        loop {
            match schema_version {
                0 => {
                    println!("initializing db to v{}", schema_version+1);
                    tx.conn.execute("CREATE TABLE code (source_code TEXT);")?;
                    tx.conn.execute("CREATE TABLE mem (bytes BLOB);")?;
                    stm = tx.conn.prepare("INSERT INTO CLOCKS (name, value) VALUES (?, ?);")?;
                    stm.bind(1, schema_name)?;
                    stm.bind(2, schema_version + 1)?;
                    while stm.next()? != sqlite::State::Done { }
                    schema_version += 1;
                }
                1 => {
                    // current version; no change needed
                    break;
                }
                _ => {
                    return Err(StorageError::UnknownSchemaVersion(schema_version));
                }
            }
        }

        drop(stm);
        tx.commit()?;
        return Ok(s);
    }

    pub fn start_transaction(&mut self) -> Result<Transaction, StorageError> {
        match home::home_dir() {
            Some(dir) => Ok(Transaction::new(sqlite::open(dir.join(".pppl.db"))?)?),
            None => Err(StorageError::NoHomeDirectory),
        }
    }

}

const SQLITE_BUSY: isize = 5; // https://sqlite.org/rescode.html#busy

fn exec_sqlite_until_not_busy<T>(op: T) -> Result<(), sqlite::Error> where T: Fn() -> Result<(), sqlite::Error> {
    loop {
        match op() {
            Ok(x) => { return Ok(x); }
            Err(e) if e.code == Some(SQLITE_BUSY) => { continue; }
            Err(e) => { return Err(e); }
        }
    }
}

impl Transaction {

    fn new(conn: sqlite::Connection) -> Result<Self, StorageError> {
        exec_sqlite_until_not_busy(|| conn.execute("BEGIN IMMEDIATE;"))?;

        let mut stm = conn.prepare("SELECT bytes FROM mem;")?;
        let mut root = Value::Dict(BTreeMap::new());
        while let sqlite::State::Row = stm.next()? {
            let source = stm.read::<Vec<u8>>(0)?;
            let val = bincode::deserialize::<Value>(&source)?;
            root = val;
        }
        drop(stm);

        return Ok(Transaction {
            conn: conn,
            mem: root,
            mem_changed: false,
        });
    }

    pub fn read_code(&self) -> Result<Module<SourceSpan>, StorageError> {
        let mut stm = self.conn.prepare("SELECT source_code FROM code;")?;
        while let sqlite::State::Row = stm.next()? {
            let source = stm.read::<String>(0)?;
            let module = ModuleParser::new().parse(&source)?;
            return Ok(module);
        }

        return Ok(Module {
            annotation: SourceSpan { start: 0, end: 0 },
            blocks: Vec::new(),
        });
    }

    pub fn replace_code(&mut self, new_code: &str) -> Result<(), StorageError> {
        self.conn.execute("DELETE FROM code;")?;
        let mut stm = self.conn.prepare("INSERT INTO code (source_code) VALUES (?);")?;
        stm.bind(1, new_code)?;
        while stm.next()? != sqlite::State::Done { }
        return Ok(());
    }

    pub fn read_memory(&self, path: &Vec<Value>) -> Result<Option<&Value>, StorageError> {
        let mut root = &self.mem;

        for entry in path {
            match root {
                Value::Dict(mapping) => {
                    match mapping.get(entry) {
                        Option::Some(e) => { root = e; }
                        None => { return Ok(None); }
                    }
                }
                _ => {
                    return Ok(None);
                }
            }
        }

        return Ok(Some(root));
    }

    pub fn write_memory(&mut self, path: &Vec<Value>, new_value: &Value) -> Result<bool, StorageError> {
        let mut env = &mut self.mem;
        for entry in path {
            match env {
                Value::Dict(mapping) => {
                    if !mapping.contains_key(entry) {
                        mapping.insert(entry.clone(), Value::Dict(BTreeMap::new()));
                    }
                    match mapping.get_mut(entry) {
                        Option::Some(e) => { env = e; }
                        _ => { return Ok(false); /* should be unreachable */ }
                    }
                }
                _ => {
                    return Ok(false);
                }
            }
        }

        self.mem_changed = true;
        *env = new_value.clone();
        return Ok(true);
    }

    pub fn commit(self) -> Result<(), StorageError> {
        if self.mem_changed {
            self.conn.execute("DELETE FROM mem;")?;
            let mut stm = self.conn.prepare("INSERT INTO mem (bytes) VALUES (?);")?;
            stm.bind(1, &(bincode::serialize(&self.mem)?)[..])?;
            while stm.next()? != sqlite::State::Done { }
        }

        exec_sqlite_until_not_busy(|| self.conn.execute("COMMIT;"))?;
        return Ok(());
    }

}
