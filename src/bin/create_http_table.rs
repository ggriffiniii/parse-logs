extern crate parse_logs;
#[macro_use]
extern crate structopt;
extern crate rusqlite;
extern crate chrono;
extern crate phf;

include!(concat!(env!("OUT_DIR"), "/friendly_names.rs"));

use structopt::StructOpt;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::io::BufReader;
use std::fs::File;
use std::io::BufRead;
use parse_logs::{http, dhcp};
use std::borrow::Cow;
use std::collections::BTreeSet;
use rusqlite::types::ToSql;
use std::io::Write;
use std::collections::HashMap;
use std::fs;
use chrono::NaiveDateTime;

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(long = "dhcp_dir", parse(from_os_str))]
    dhcp_dir: PathBuf,

    #[structopt(long = "http_dir", parse(from_os_str))]
    http_dir: PathBuf,
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
        self.tx.execute("CREATE TABLE http_logs (datetime TEXT, mac_addr TEXT, friendly_name TEXT);", &[])?;
        self.cols.push("datetime".to_string());
        self.cols.push("mac_addr".to_string());
        self.cols.push("friendly_name".to_string());
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
        self.tx.execute(&format!("ALTER TABLE http_logs ADD {} TEXT", sanitized_col), &[])?;
        self.cols.push(col.to_string());
        self.cols_set.insert(col.to_string());
        Ok(())
    }

    fn insert_log_entry(&mut self, mac_addr: Option<&str>, friendly_name: Option<&str>, log_entry: &http::LogEntry) -> Result<(), Box<Error>> {
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
        entry_cols.push("mac_addr".to_string());
        entry_cols.push("friendly_name".to_string());
        entry_values_traits.push(&log_datetime);
        entry_values_traits.push(&mac_addr);
        entry_values_traits.push(&friendly_name);
        let insert_stmt = format!("INSERT INTO http_logs ({}) VALUES ({})",
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

#[derive(Debug)]
struct IpToMacBuilder(HashMap<String, Vec<(NaiveDateTime, String)>>);
#[derive(Debug)]
struct IpToMacLookup(HashMap<String, Vec<(NaiveDateTime, String)>>);

impl IpToMacBuilder {
    fn new() -> Self {
        IpToMacBuilder(HashMap::new())
    }

    fn add_dhcp_ack(&mut self, date: NaiveDateTime, ip_addr: &str, mac_addr: &str) {
        self.0.entry(ip_addr.to_string()).or_default().push((date, mac_addr.to_string()));
    }

    fn finalize(self) -> IpToMacLookup {
        // Sort the (date, mac_addr) entries within each ip address, and then
        // remove any consecutive entries that have the same mac address.
        let finalized = self.0.into_iter().map(|(k, mut v)| {
            v.sort_unstable();
            let remove_dups = v.iter().take(1).cloned().chain(v.windows(2).filter_map(|window| {
                if let &[(_, ref mac1), (ref date2, ref mac2)] = window {
                    if mac1 == mac2 {
                        None
                    } else {
                        Some((date2.clone(), mac2.clone()))
                    }
                } else {
                    unreachable!();
                }
            }));
            (k, remove_dups.collect())
        }).collect();
        IpToMacLookup(finalized)
    }
}

impl IpToMacLookup {
    fn get_mac(&self, date: NaiveDateTime, ip_addr: &str) -> Option<&str> {
        let v: &[(NaiveDateTime, String)] = self.0.get(ip_addr)?;
        v.iter().take_while(|&&(ack_date, _) : &&(NaiveDateTime, String)| -> bool {ack_date < date}).map(|(_, mac)| mac.as_str()).last()
    }
}

fn read_dhcp_logs<P: AsRef<Path>>(dir: P) -> Result<(IpToMacLookup, HashMap<String, String>), Box<Error>> {
    let mut ip_to_mac = IpToMacBuilder::new();
    let mut mac_to_friendly_name = HashMap::new();
    for dir_entry in fs::read_dir(dir)? {
        let filename = dir_entry?.path();
        let filereader = BufReader::new(File::open(&filename)?);
        for line in filereader.split(b'\n') {
            let line = line?;
            match dhcp::LogEntry::new(&line) {
                Ok(dhcp::LogEntry{ datetime, msg: dhcp::DhcpMsg::Ack{ip_addr, mac_addr, friendly_name} }) => {
                    if let Some(friendly_name) = friendly_name {
                        println!("friendly_name: {}", friendly_name);
                        use std::collections::hash_map::Entry::*;
                        match mac_to_friendly_name.entry(mac_addr.clone()) {
                            Occupied(occupied) => {
                                if *occupied.get() != friendly_name {
                                    eprintln!("mac {} has multiple friendly names: ({}, {})", &mac_addr, occupied.get(), friendly_name);
                                }
                            },
                            Vacant(vacant) => {
                                vacant.insert(friendly_name);
                            },
                        }
                    }
                    ip_to_mac.add_dhcp_ack(datetime, &ip_addr, &mac_addr);
                },
                Ok(_) => {},
                Err(_) => eprintln!("Failed to parse line: {}", String::from_utf8_lossy(&line)),
            }
        }
    }
    Ok((ip_to_mac.finalize(), mac_to_friendly_name))
}

fn run() -> Result<(), Box<Error>> {
    let opt = Opt::from_args();
    println!("{:?}", opt);
    let (ip_to_mac, mac_to_friendly_name) = read_dhcp_logs(opt.dhcp_dir)?;
    println!("{:?}", ip_to_mac);
    let mut db = rusqlite::Connection::open("output.db")?;
    let mut tx = Tx::new(&mut db)?;
    let mut failures = File::create("failures.log")?;
    let mut total_entries = 0;
    for dir_entry in fs::read_dir(opt.http_dir)? {
        let filename = dir_entry?.path();
        let mut file_entries = 0;
        let filereader = BufReader::new(File::open(&filename)?);
        for line in filereader.split(b'\n') {
            let line = line?;
            if let Ok(log_entry) = http::LogEntry::new(&line) {
                let mac_addr: Option<&str> = log_entry.attrs.get("srcip").and_then(|b| std::str::from_utf8(b).ok()).and_then(|ip| ip_to_mac.get_mac(log_entry.datetime, ip));
                let friendly_name: Option<&str> = mac_addr.and_then(|mac_addr| mac_to_friendly_name.get(mac_addr).map(String::as_ref));
                if let Some(friendly_name) = friendly_name {
                    let friendly_name = friendly_name.to_lowercase();
                    if FRIENDLY_NAMES.contains(friendly_name.as_str()) {
                        tx.insert_log_entry(mac_addr, Some(&friendly_name), &log_entry)?;
                        total_entries += 1;
                        file_entries += 1;
                    }
                }
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
