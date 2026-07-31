use std::error::Error;

use clap::Parser;

use stmoc_pp::{info::Info, list::List, make::Make, query::Query};

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None, allow_negative_numbers = true)]
/// Create and query a ST-MOC precursor, i.e. a list of time (range, S-MOC) couples.
enum Args {
  #[clap(name = "make")]
  Make(Make),
  #[clap(name = "info")]
  Info(Info),
  #[clap(name = "list")]
  List(List),
  #[clap(name = "query")]
  Query(Query),
}

impl Args {
  fn exec(self) -> Result<(), Box<dyn Error>> {
    match self {
      Args::Make(make) => make.exec(),
      Args::Info(info) => info.exec(),
      Args::List(list) => list.exec(),
      Args::Query(query) => query.exec(),
    }
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let args = Args::parse();
  args.exec()
}
