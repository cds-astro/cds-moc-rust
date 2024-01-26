use std::error::Error;

use structopt::clap::AppSettings;
use structopt::StructOpt;

use moc_cli::{
  constants::Constants, convert::Convert, filter::Filter, from::From, hprint::HumanPrint,
  info::Info, op::Op, view::View,
};

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
  HumanPrint(HumanPrint),
  #[structopt(name = "view")]
  /// Save a PNG of a S-MOC and visualize it.
  View(View),
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
      Args::View(view) => view.exec(),
    }
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let args = Args::from_args();
  args.exec()
}
