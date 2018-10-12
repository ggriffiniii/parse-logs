extern crate phf_codegen;

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() {
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("friendly_names.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());

    write!(&mut file, "static FRIENDLY_NAMES: phf::Set<&'static str> = ").unwrap();
    phf_codegen::Set::new()
        .entry("adriennes-mbp")
        .entry("adriennsmacbook")
        .entry("adriennssiphone")
        .entry("amaras-ipad")
        .entry("ashleys-ipad")
        .entry("ashleys-iphone")
        .entry("ashleysplewatch")
        .entry("benjaminsiphone")
        .entry("bethany-i5")
        .entry("bethanys-air")
        .entry("bobbybonsiphone")
        .entry("bobbysipadmini")
        .entry("briannas-iphone")
        .entry("brittanys-ipad")
        .entry("brittanysiphone")
        .entry("courtneysiphone")
        .entry("crystalesiphone")
        .entry("crystals-ipad")
        .entry("dianas-ipad")
        .entry("elainas-iphone")
        .entry("ellies-iphone")
        .entry("ericas-ipad")
        .entry("hanks-ipad")
        .entry("joe")
        .entry("joshuas-ipad")
        .entry("joshuas-iphone")
        .entry("katharines-ipad")
        .entry("kristens-ipad")
        .entry("kristens-iphone")
        .entry("kristi-anderson")
        .entry("kristismithipad")
        .entry("kristyns-iphone")
        .entry("krystals-ipad")
        .entry("lorrie")
        .entry("lucindas-ipad")
        .entry("mareans-ipad")
        .entry("mareans-iphone")
        .entry("meaganbtsiphone")
        .entry("meagans-ipad")
        .entry("megans-iphone")
        .entry("missythang")
        .entry("monicas-iphone")
        .entry("monicas-mbp")
        .entry("olivias-iphone")
        .entry("olivias-phone")
        .entry("pauls-ipad")
        .entry("remastrssiphone")
        .entry("robinhansiphone")
        .entry("robins-ipad")
        .entry("robins-iphone")
        .entry("roslyns-iphone")
        .entry("tolson")
        .entry("wendys-ipad")
        .build(&mut file)
        .unwrap();
    write!(&mut file, ";\n").unwrap();
}

