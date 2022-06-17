
use std::error::Error;

use structopt::StructOpt;
use structopt::clap::AppSettings;

use moc_cli::info::Info;
use moc_cli::constants::Constants;
use moc_cli::from::From;
use moc_cli::op::Op;
use moc_cli::filter::Filter;
use moc_cli::convert::Convert;
use moc_cli::hprint::HumanPrint;

#[derive(Debug, StructOpt)]
#[structopt(name = "moc", global_settings = &[AppSettings::ColoredHelp, AppSettings::AllowNegativeNumbers])]
/// Create, manipulate and filter files using HEALPix Multi-Order Coverage maps (MOCs).
///
/// See the man page for more information.
enum Args {
  #[structopt(name = "table")]
  /// Prints MOC constants
  Constants(Constants),
  #[structopt(name = "info")]
  /// Prints information on the given MOC
  Info(Info),
  #[structopt(name = "convert")]
  /// Converts an input format to the (most recent versions of) an output format
  Convert(Convert),
  #[structopt(name = "from")]
  /// Create a MOC from given parameters
  From(From),
  #[structopt(name = "op")]
  /// Perform operations on MOCs
  Op(Op),
  #[structopt(name = "filter")]
  /// Filter file rows using a MOC
  Filter(Filter),
  #[structopt(name = "hprint")]
  /// Print a MOC to a human readable form
  HumanPrint(HumanPrint)
}

impl Args {
  fn exec(self) -> Result<(), Box<dyn Error>> {
    match self {
      Args::Constants(cst) => cst.exec(),
      Args::Info(info) => info.exec(),
      Args::Convert(convert) => convert.exec(),
      Args::From(from) => from.exec(),
      Args::Op(op) => op.exec(),
      Args::Filter(filter) => filter.exec(),
      Args::HumanPrint(hprint) => hprint.exec(),
    }
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let args = Args::from_args();
  args.exec()
}
