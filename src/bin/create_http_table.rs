extern crate parse_logs;
#[macro_use]
extern crate structopt;
extern crate rusqlite;

use structopt::StructOpt;
use std::error::Error;
use std::path::PathBuf;
use std::io::BufReader;
use std::fs::File;
use std::io::BufRead;
use parse_logs::http::LogEntry;
use std::borrow::Cow;
use std::collections::BTreeSet;
use rusqlite::types::ToSql;
use std::io::Write;

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

struct Tx<'a>{
    tx: rusqlite::Transaction<'a>,
    cols: Vec<String>,
    cols_set: BTreeSet<String>,
}

impl<'a> Tx<'a> {
    fn new(db: &mut rusqlite::Connection) -> Result<Tx, Box<Error>> {
        let mut tx = Tx{tx: db.transaction()?, cols: Vec::new(), cols_set: BTreeSet::new()};
        tx.create_table()?;
        Ok(tx)
    }

    fn create_table(&mut self) -> Result<(), Box<Error>> {
        self.tx.execute("CREATE TABLE logs (datetime TEXT);", &[])?;
        self.cols.push("datetime".to_string());
        Ok(())
    }

    fn sanitize_col_name(col: &str) -> Cow<str> {
        match col {
            "group" => Cow::Owned("_group".to_string()),
            col if col.contains("-") => Cow::Owned(col.replace("-", "_")),
            col => Cow::Borrowed(col),
        }
    }

    fn add_col(&mut self, col: &str) -> Result<(), Box<Error>> {
        let sanitized_col = Self::sanitize_col_name(col);
        self.tx.execute(&format!("ALTER TABLE logs ADD {} TEXT", sanitized_col), &[])?;
        self.cols.push(col.to_string());
        self.cols_set.insert(col.to_string());
        Ok(())
    }

    fn insert_log_entry(&mut self, log_entry: &LogEntry) -> Result<(), Box<Error>> {
        let cols_required: BTreeSet<String> = log_entry.attrs.keys().cloned().collect();
        let cols_to_add: Vec<String> = cols_required.difference(&self.cols_set).cloned().collect();
        for col in cols_to_add {
            self.add_col(&col)?;
        }
        let log_datetime = log_entry.datetime;
        let (mut entry_cols, entry_values): (Vec<String>, Vec<Vec<u8>>) = log_entry.attrs.iter().map(|(k,v)| (Self::sanitize_col_name(k).into(), v.clone())).unzip();
        let entry_values: Vec<rusqlite::types::Value> = entry_values.into_iter().map(|v| rusqlite::types::Value::Blob(v)).collect();
        let mut entry_values_traits: Vec<&ToSql> = entry_values.iter().map(|v| v as &ToSql).collect();
        entry_cols.push("datetime".to_string());
        entry_values_traits.push(&log_datetime);
        let insert_stmt = format!("INSERT INTO logs ({}) VALUES ({})",
                entry_cols.join(","),
                entry_cols.iter().map(|_| "?".to_string()).collect::<Vec<_>>().join(","));
        self.tx.execute(&insert_stmt, entry_values_traits.as_slice())?;
        Ok(())
    }

    fn commit(self) -> Result<(), Box<Error>> {
        self.tx.commit()?;
        Ok(())
    }
}

fn run() -> Result<(), Box<Error>> {
    let opt = Opt::from_args();
    println!("{:?}", opt);
    let mut db = rusqlite::Connection::open("output.db")?;
    let mut tx = Tx::new(&mut db)?;
    let mut failures = File::create("failures.log")?;
    let mut total_entries = 0;
    for filename in opt.files {
        let mut file_entries = 0;
        let filereader = BufReader::new(File::open(&filename)?);
        for line in filereader.split(b'\n') {
            let line = line?;
            if let Ok(log_entry) = LogEntry::new(&line) {
                tx.insert_log_entry(&log_entry)?;
                total_entries += 1;
                file_entries += 1;
            } else {
                failures.write_all(&line)?;
                failures.write_all(&b"\n"[..])?;
                println!("failed processing line: {}", String::from_utf8_lossy(&line));
            }
        }
        println!("Added {} entries from file: {}", file_entries, filename.to_string_lossy());
    }
    println!("Added {} total entries", total_entries);
    tx.commit()?;
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(-1);
    }
}
