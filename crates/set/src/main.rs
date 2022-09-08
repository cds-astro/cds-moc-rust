
use std::error::Error;

use clap::Parser;
// use structopt::StructOpt;
// use structopt::clap::AppSettings;

use moc_set::mk::Make;
use moc_set::append::Append;
use moc_set::chgstatus::ChangeStatus;
use moc_set::purge::Purge;
use moc_set::list::List;
use moc_set::query::Query;
use moc_set::extract::Extract;

// #[derive(Debug, StructOpt)]
// #[structopt(name = "mocset", global_settings = &[AppSettings::ColoredHelp, AppSettings::AllowNegativeNumbers])]

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
  // Operation on 2 MOCs or on a set of MOCs ?
  // ...
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
