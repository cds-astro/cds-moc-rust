
use std::error::Error;

use clap::Parser;

use moc_set::{
  mk::Make,
  append::Append,
  chgstatus::ChangeStatus,
  purge::Purge,
  list::List,
  query::Query,
  extract::Extract
};

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None, allow_negative_numbers = true)]
/// Create, update and query a set of HEALPix Multi-Order Coverage maps (MOCs).
/// WARNING: use the same architecture to build, update and query a moc-set file.
enum Args {
  #[clap(name = "make")]
  Make(Make),
  #[clap(name = "append")]
  Append(Append),
  #[clap(name = "chgstatus")]
  ChangeStatus(ChangeStatus),
  #[clap(name = "purge")]
  Purge(Purge),
  #[clap(name = "list")]
  List(List), // option count to read flags only!
  #[clap(name = "query")]
  Query(Query),
  #[clap(name = "extract")]
  Extract(Extract),
}

impl Args {
  fn exec(self) -> Result<(), Box<dyn Error>> {
    match self {
      Args::Make(make) => make.exec(),
      Args::Append(append) => append.exec(),
      Args::ChangeStatus(chgstatus) => chgstatus.exec(),
      Args::Purge(purge) => purge.exec(),
      Args::List(list) => list.exec(),
      Args::Query(query) => query.exec(),
      Args::Extract(extract) => extract.exec(),
    }
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let args = Args::from_args();
  args.exec()
}
