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
use parse_logs::dhcp::{LogEntry, DhcpMsg};

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

struct Tx<'a>{
    tx: rusqlite::Transaction<'a>,
}

impl<'a> Tx<'a> {
    fn new(db: &mut rusqlite::Connection) -> Result<Tx, Box<Error>> {
        let mut tx = Tx{tx: db.transaction()?};
        tx.create_table()?;
        Ok(tx)
    }

    fn create_table(&mut self) -> Result<(), Box<Error>> {
        self.tx.execute("CREATE TABLE dhcp_logs (datetime TEXT, ip_addr TEXT, mac_addr TEXT);", &[])?;
        Ok(())
    }

    fn insert_log_entry(&mut self, log_entry: &LogEntry) -> Result<(), Box<Error>> {
        if let LogEntry{ datetime, msg: DhcpMsg::Ack{ip_addr, mac_addr} } = log_entry {
            self.tx.execute("INSERT INTO dhcp_logs (datetime, ip_addr, mac_addr) VALUES (?, ?, ?)", &[datetime, &ip_addr.as_str(), &mac_addr.as_str()])?;
        }
        return Ok(())
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
    let mut total_entries = 0;
    for filename in opt.files {
        let mut file_entries = 0;
        let filereader = BufReader::new(File::open(&filename)?);
        for line in filereader.split(b'\n') {
            let line = line?;
            match LogEntry::new(&line) {
                Ok(log_entry) => {
                    total_entries += 1;
                    file_entries += 1;
                    tx.insert_log_entry(&log_entry)?;
                },
                Err(_) => eprintln!("Failed to parse line: {}", String::from_utf8_lossy(&line)),
            }
        }
        println!("Added {} entries from file: {}", file_entries, filename.to_string_lossy());
    }
    tx.commit()?;
    println!("Added {} total entries", total_entries);
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(-1);
    }
}
