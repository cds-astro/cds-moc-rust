use std::{
  error::Error,
  fs::File,
  io::{BufRead, BufReader},
  ops::RangeInclusive,
  path::PathBuf,
  str::FromStr,
};

use structopt::StructOpt;

use mapproj::{
  cylindrical::{car::Car, cea::Cea, cyp::Cyp, mer::Mer},
  hybrid::hpx::Hpx as Hpix,
  pseudocyl::{ait::Ait, mol::Mol, par::Par, sfl::Sfl},
  zenithal::{air::Air, arc::Arc, feye::Feye, sin::Sin, stg::Stg, tan::Tan, zea::Zea},
};

use moclib::{
  deser::{
    ascii::{from_ascii_ivoa, from_ascii_stream},
    fits::{from_fits_ivoa, MocIdxType, MocQtyType, MocType},
    img::{to_png_file, to_png_file_auto},
    json::from_json_aladin,
  },
  moc::{
    range::RangeMOC, CellMOCIntoIterator, CellMOCIterator, CellOrCellRangeMOCIntoIterator,
    CellOrCellRangeMOCIterator, RangeMOCIterator,
  },
  qty::Hpx,
};

use crate::input::{fmt_from_extension, InputFormat};

#[derive(Debug)]
pub struct Bound(RangeInclusive<f64>);
impl FromStr for Bound {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    s.split_once("..")
      .ok_or_else(|| format!("'..' separator not found in '{}'", s))
      .and_then(|(l, r)| {
        l.parse::<f64>().map_err(|e| e.to_string()).and_then(|l| {
          r.parse::<f64>()
            .map(|r| Bound(l..=r))
            .map_err(|e| e.to_string())
        })
      })
  }
}

#[derive(StructOpt, Debug)]
pub enum Mode {
  #[structopt(name = "allsky")]
  /// Creates an allsky view using the Mollweide projection.  
  AllSky {
    /// Number of pixels along the y-axis
    y_size: u16,
  },
  #[structopt(name = "auto")]
  /// Generate either a Mollweide or a Sinus projection centered on the mean MOC center with
  /// automatic bounds.
  Auto {
    /// Number of pixels along the y-axis
    y_size: u16,
  },
  #[structopt(name = "custom")]
  /// Full control on the visualization (projection, center, bounds, ...)
  Custom {
    /// The chosen projection: 'car', 'cea', 'cyp', 'mer', 'hpx', 'ait', 'mol', 'par', 'sfl', 'sin'
    /// 'air', 'arc', 'feye', 'sin', 'stg', 'tan', 'zea'.
    proj: String,
    /// Size of the image along the x-axis, in pixels
    img_size_x: u16,
    /// Size of the image along the y-axis, in pixels
    img_size_y: u16,
    #[structopt(short = "l", long = "lon", default_value = "0.0")]
    /// Longitude of the center of the projection, in degrees
    center_lon: f64,
    #[structopt(short = "b", long = "lat", default_value = "0.0")]
    /// Latitude of the center of the projection, in degrees
    center_lat: f64,
    #[structopt(long = "x-bounds")]
    /// Bounds, in the projection plane, matching both image edges along the the x-axis
    proj_bounds_x: Option<Bound>,
    #[structopt(long = "y-bounds")]
    /// Bounds, in the projection plane, matching both image edges along the the y-axis
    proj_bounds_y: Option<Bound>,
  },
}

/// Save a PNG file representing the MOC and visualize it.
/// Only available for S-MOCs so far.
#[derive(StructOpt, Debug)]
pub struct View {
  #[structopt(parse(from_os_str))]
  /// Path of the input MOC file (or stdin if equals "-")
  input: PathBuf,
  #[structopt(short = "f", long = "format")]
  /// Format of the input MOC ('ascii', 'json', 'fits' or 'stream') [default: guess from the file extension]
  input_fmt: Option<InputFormat>,

  #[structopt(parse(from_os_str))]
  /// Path of the output file
  output: PathBuf,
  #[structopt(short = "s", long = "silent")]
  /// Generate the output PNG without showing it.
  hide: bool,

  #[structopt(subcommand)]
  /// Image generation parameters
  mode: Mode,
}

impl View {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    let path = self.input;
    if path == PathBuf::from("-") {
      if let Some(input_fmt) = self.input_fmt {
        let stdin = std::io::stdin();
        exec(stdin.lock(), input_fmt, self.output, self.mode, !self.hide)
      } else {
        Err(
          String::from(
            "Using stdin, the MOC format ('ascii', 'json', ...) must be provided, see options.",
          )
          .into(),
        )
      }
    } else {
      let input_fmt = match self.input_fmt {
        Some(input_fmt) => Ok(input_fmt),
        None => fmt_from_extension(&path),
      }?;
      let f = File::open(path)?;
      exec(
        BufReader::new(f),
        input_fmt,
        self.output,
        self.mode,
        !self.hide,
      )
    }
  }
}

pub(crate) fn exec<R: BufRead>(
  mut input: R,
  input_fmt: InputFormat,
  output: PathBuf,
  mode: Mode,
  view: bool,
) -> Result<(), Box<dyn Error>> {
  let smoc = match input_fmt {
    // SMOC
    InputFormat::Ascii => {
      let mut input_str = Default::default();
      input.read_to_string(&mut input_str)?;
      let cellcellranges = from_ascii_ivoa::<u64, Hpx<u64>>(&input_str)?;
      Ok::<RangeMOC<u64, Hpx<u64>>, String>(
        cellcellranges
          .into_cellcellrange_moc_iter()
          .ranges()
          .into_range_moc(),
      )
    }
    InputFormat::Json => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cells = from_json_aladin::<u64, Hpx<u64>>(&input_str)?;
      Ok(cells.into_cell_moc_iter().ranges().into_range_moc())
    }
    InputFormat::Stream => {
      let cellrange_it = from_ascii_stream::<u64, Hpx<u64>, _>(input)?;
      Ok(cellrange_it.ranges().into_range_moc())
    }
    InputFormat::Fits => {
      let fits_res = from_fits_ivoa(input)?;
      match fits_res {
        MocIdxType::U16(moc) => match moc {
          MocQtyType::Hpx(moc) => match moc {
            MocType::Ranges(moc) => Ok(moc.convert::<u64, Hpx<u64>>().into_range_moc()),
            MocType::Cells(moc) => Ok(
              moc
                .into_cell_moc_iter()
                .ranges()
                .convert::<u64, Hpx<u64>>()
                .into_range_moc(),
            ),
          },
          _ => Err(String::from("Input MOC type must be SMOC.")),
        },
        MocIdxType::U32(moc) => match moc {
          MocQtyType::Hpx(moc) => match moc {
            MocType::Ranges(moc) => Ok(moc.convert::<u64, Hpx<u64>>().into_range_moc()),
            MocType::Cells(moc) => Ok(
              moc
                .into_cell_moc_iter()
                .ranges()
                .convert::<u64, Hpx<u64>>()
                .into_range_moc(),
            ),
          },
          _ => Err(String::from("Input MOC type must be SMOC.")),
        },
        MocIdxType::U64(moc) => match moc {
          MocQtyType::Hpx(moc) => match moc {
            MocType::Ranges(moc) => Ok(moc.into_range_moc()),
            MocType::Cells(moc) => Ok(moc.into_cell_moc_iter().ranges().into_range_moc()),
          },
          _ => Err(String::from("Input MOC type must be SMOC.")),
        },
      }
    }
  }?;
  match mode {
    Mode::AllSky { y_size } => to_png_file(
      &smoc,
      (y_size << 1, y_size),
      Some(Mol::new()),
      None,
      None,
      output.as_path(),
      view,
    )
    .map_err(|e| e.into()),
    Mode::Auto { y_size } => to_png_file_auto(&smoc, y_size, output.as_path(), view)
      .map(|_| ())
      .map_err(|e| e.into()),
    Mode::Custom {
      proj,
      center_lon,
      center_lat,
      img_size_x,
      img_size_y,
      proj_bounds_x,
      proj_bounds_y,
    } => {
      let img_size = (img_size_x, img_size_y);
      let center = Some((center_lon.to_radians(), center_lat.to_radians()));
      let proj_bounds =
        if let (Some(proj_bounds_x), Some(proj_bounds_y)) = (proj_bounds_x, proj_bounds_y) {
          Some((proj_bounds_x.0, proj_bounds_y.0))
        } else {
          None
        };
      match proj.as_str() {
        // Cylindrical
        "car" => to_png_file(
          &smoc,
          img_size,
          Some(Car::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "cea" => to_png_file(
          &smoc,
          img_size,
          Some(Cea::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "cyp" => to_png_file(
          &smoc,
          img_size,
          Some(Cyp::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "mer" => to_png_file(
          &smoc,
          img_size,
          Some(Mer::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        // Hybrid
        "hpx" => to_png_file(
          &smoc,
          img_size,
          Some(Hpix::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        // Psudocylindrical
        "ait" => to_png_file(
          &smoc,
          img_size,
          Some(Ait::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "mol" => to_png_file(
          &smoc,
          img_size,
          Some(Mol::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "par" => to_png_file(
          &smoc,
          img_size,
          Some(Par::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "sfl" => to_png_file(
          &smoc,
          img_size,
          Some(Sfl::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        // Zenithal
        "air" => to_png_file(
          &smoc,
          img_size,
          Some(Air::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "arc" => to_png_file(
          &smoc,
          img_size,
          Some(Arc::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "feye" => to_png_file(
          &smoc,
          img_size,
          Some(Feye::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "sin" => to_png_file(
          &smoc,
          img_size,
          Some(Sin::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "stg" => to_png_file(
          &smoc,
          img_size,
          Some(Stg::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "tan" => to_png_file(
          &smoc,
          img_size,
          Some(Tan::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        "zea" => to_png_file(
          &smoc,
          img_size,
          Some(Zea::new()),
          center,
          proj_bounds,
          output.as_path(),
          view,
        )
        .map_err(|e| e.into()),
        _ => Err(format!("Unknown projection '{}'", &proj).into()),
      }
    }
  }
}
