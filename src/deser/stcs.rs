//! MOC creation from an STC-S string.

use thiserror::Error;

use nom::{
  error::{convert_error, VerboseError},
  Err,
};

use stc_s::{
  space::{
    common::{
      region::{BoxParams, CircleParams, ConvexParams, EllipseParams, PolygonParams},
      FillFrameRefposFlavor, Flavor, Frame, FromPosToVelocity, SpaceUnit,
    },
    position::Position,
    positioninterval::PositionInterval,
  },
  visitor::{impls::donothing::VoidVisitor, CompoundVisitor, SpaceVisitor, StcVisitResult},
  Stc,
};

use healpix::nested::{
  bmoc::BMOC, box_coverage, cone_coverage_approx_custom, elliptical_cone_coverage_custom,
  polygon_coverage, zone_coverage,
};

use crate::{moc::range::RangeMOC, qty::Hpx};

const HALF_PI: f64 = 0.5 * std::f64::consts::PI;
const PI: f64 = std::f64::consts::PI;
const TWICE_PI: f64 = 2.0 * std::f64::consts::PI;

#[derive(Error, Debug)]
pub enum Stc2MocError {
  #[error("Frame other than ICRS not supported (yet). Found: {found:?}")]
  FrameIsNotICRS { found: Frame },
  #[error("Flavor other than Spher2 not supported (yet). Found: {found:?}")]
  FlavorIsNotSpher2 { found: Flavor },
  #[error("Units ther than 'deg' not (yet?!) supported. Found: {found:?}")]
  UnitsNotSupported { found: Vec<SpaceUnit> },
  #[error("Convex shape not (yet?!) supported.")]
  ConvexNotSupported,
  #[error("Simple position not supported.")]
  SimplePositionNotSupported,
  #[error("Position interval not supported.")]
  PositionIntervalNotSupported,
  #[error("invalid header (expected {expected:?}, found {found:?})")]
  WrongNumberOfParams { expected: u8, found: u8 },
  #[error("Longitude value out of bounds. Expected: [0, 360[. Actual: {value:?}")]
  WrongLongitude { value: f64 },
  #[error("Latitude value out of bounds. Expected: [-90, 90[. Actual: {value:?}")]
  WrongLatitude { value: f64 },
  #[error("STC-S string parsing not complete. Remaining: {rem:?}")]
  ParseHasRemaining { rem: String },
  #[error("STC-S string parsing incomplete: {msg:?}")]
  ParseIncomplete { msg: String },
  #[error("STC-S string parsing error: {msg:?}")]
  ParseFailure { msg: String },
  #[error("STC-S string parsing failure: {msg:?}")]
  ParseError { msg: String },
  #[error("No space sub-phrase found in STC-S string")]
  NoSpaceFound,
  #[error("Custom error: {msg:?}")]
  Custom { msg: String },
}

#[derive(Debug, Clone)]
struct Stc2Moc {
  depth: u8,
  delta_depth: u8,
}
impl Stc2Moc {
  fn new(depth: u8, delta_depth: Option<u8>) -> Self {
    Self {
      depth,
      delta_depth: delta_depth.unwrap_or(2),
    }
  }
}
impl CompoundVisitor for Stc2Moc {
  type Value = BMOC;
  type Error = Stc2MocError;

  fn visit_allsky(&mut self) -> Result<Self::Value, Self::Error> {
    Ok(BMOC::new_allsky(self.depth))
  }

  fn visit_circle(&mut self, circle: &CircleParams) -> Result<Self::Value, Self::Error> {
    // Get params
    let lon_deg = circle
      .center()
      .first()
      .ok_or_else(|| Stc2MocError::Custom {
        msg: String::from("Empty circle longitude"),
      })?;
    let lat_deg = circle.center().get(1).ok_or_else(|| Stc2MocError::Custom {
      msg: String::from("Empty circle latitude"),
    })?;
    let radius_deg = circle.radius();
    // Convert params
    let lon = lon_deg2rad(*lon_deg)?;
    let lat = lat_deg2rad(*lat_deg)?;
    let r = radius_deg.to_radians();
    if r <= 0.0 || PI <= r {
      Err(Stc2MocError::Custom {
        msg: format!("Radius out of bounds. Expected: ]0, 180[. Actual: {}.", r),
      })
    } else {
      Ok(cone_coverage_approx_custom(
        self.depth,
        self.delta_depth,
        lon,
        lat,
        r,
      ))
    }
  }

  fn visit_ellipse(&mut self, ellipse: &EllipseParams) -> Result<Self::Value, Self::Error> {
    // Get params
    let lon_deg = ellipse
      .center()
      .first()
      .ok_or_else(|| Stc2MocError::Custom {
        msg: String::from("Empty ellipse longitude"),
      })?;
    let lat_deg = ellipse
      .center()
      .get(1)
      .ok_or_else(|| Stc2MocError::Custom {
        msg: String::from("Empty ellipse latitude"),
      })?;
    let a_deg = ellipse.radius_a();
    let b_deg = ellipse.radius_b();
    let pa_deg = ellipse.pos_angle();
    // Convert params
    let lon = lon_deg2rad(*lon_deg)?;
    let lat = lat_deg2rad(*lat_deg)?;
    let a = a_deg.to_radians();
    let b = b_deg.to_radians();
    let pa = pa_deg.to_radians();
    if a <= 0.0 || HALF_PI <= a {
      Err(Stc2MocError::Custom {
        msg: format!(
          "Semi-major axis out of bounds. Expected: ]0, 90[. Actual: {}.",
          a_deg
        ),
      })
    } else if b <= 0.0 || a <= b {
      Err(Stc2MocError::Custom {
        msg: format!(
          "Semi-minor axis out of bounds. Expected: ]0, {}[. Actual: {}.",
          a_deg, b_deg
        ),
      })
    } else if pa <= 0.0 || PI <= pa {
      Err(Stc2MocError::Custom {
        msg: format!(
          "Position angle out of bounds. Expected: [0, 180[. Actual: {}.",
          pa_deg
        ),
      })
    } else {
      Ok(elliptical_cone_coverage_custom(
        self.depth,
        self.delta_depth,
        lon,
        lat,
        a,
        b,
        pa,
      ))
    }
  }

  fn visit_box(&mut self, skybox: &BoxParams) -> Result<Self::Value, Self::Error> {
    // Get params
    let lon_deg = skybox
      .center()
      .first()
      .ok_or_else(|| Stc2MocError::Custom {
        msg: String::from("Empty ellipse longitude"),
      })?;
    let lat_deg = skybox.center().get(1).ok_or_else(|| Stc2MocError::Custom {
      msg: String::from("Empty ellipse latitude"),
    })?;
    let mut a_deg = skybox.bsize().first().ok_or_else(|| Stc2MocError::Custom {
      msg: String::from("Empty bsize on latitude"),
    })?;
    let mut b_deg = skybox.bsize().first().ok_or_else(|| Stc2MocError::Custom {
      msg: String::from("Empty bsize on longitude"),
    })?;
    let mut pa_deg = skybox.bsize().first().copied().unwrap_or(90.0);
    if a_deg < b_deg {
      std::mem::swap(&mut b_deg, &mut a_deg);
      pa_deg = 90.0 - pa_deg;
    }
    // Convert params
    let lon = lon_deg2rad(*lon_deg)?;
    let lat = lat_deg2rad(*lat_deg)?;
    let a = a_deg.to_radians();
    let b = b_deg.to_radians();
    let pa = pa_deg.to_radians();
    if a <= 0.0 || HALF_PI <= a {
      Err(Stc2MocError::Custom {
        msg: format!(
          "Box semi-major axis out of bounds. Expected: ]0, 90[. Actual: {}.",
          a_deg
        ),
      })
    } else if b <= 0.0 || a <= b {
      Err(Stc2MocError::Custom {
        msg: format!(
          "Box semi-minor axis out of bounds. Expected: ]0, {}[. Actual: {}.",
          a_deg, b_deg
        ),
      })
    } else if !(0.0..PI).contains(&pa) {
      Err(Stc2MocError::Custom {
        msg: format!(
          "Position angle out of bounds. Expected: [0, 180[. Actual: {}.",
          pa_deg
        ),
      })
    } else {
      Ok(box_coverage(self.depth, lon, lat, a, b, pa))
    }
  }

  fn visit_polygon(&mut self, polygon: &PolygonParams) -> Result<Self::Value, Self::Error> {
    let vertices_deg = polygon.vertices();
    let vertices = vertices_deg
      .iter()
      .step_by(2)
      .zip(vertices_deg.iter().skip(1).step_by(2))
      .map(|(lon_deg, lat_deg)| {
        let lon = lon_deg2rad(*lon_deg)?;
        let lat = lat_deg2rad(*lat_deg)?;
        Ok((lon, lat))
      })
      .collect::<Result<Vec<(f64, f64)>, Stc2MocError>>()?;
    Ok(polygon_coverage(self.depth, vertices.as_slice(), true))
  }

  fn visit_convex(&mut self, _convex: &ConvexParams) -> Result<Self::Value, Self::Error> {
    Err(Stc2MocError::ConvexNotSupported)
  }

  fn visit_not(&mut self, bmoc: Self::Value) -> Result<Self::Value, Self::Error> {
    Ok(bmoc.not())
  }

  fn visit_union(&mut self, bmocs: Vec<Self::Value>) -> Result<Self::Value, Self::Error> {
    let n = bmocs.len();
    bmocs
      .into_iter()
      .reduce(|acc, curr| acc.or(&curr))
      .ok_or_else(|| Stc2MocError::Custom {
        msg: format!(
          "Wrong number of elements in union. Expected: >=2. Actual: {} ",
          n
        ),
      })
  }

  fn visit_intersection(&mut self, bmocs: Vec<Self::Value>) -> Result<Self::Value, Self::Error> {
    let n = bmocs.len();
    bmocs
      .into_iter()
      .reduce(|acc, curr| acc.and(&curr))
      .ok_or_else(|| Stc2MocError::Custom {
        msg: format!(
          "Wrong number of elements in intersection. Expected: >=2. Actual: {} ",
          n
        ),
      })
  }

  fn visit_difference(
    &mut self,
    left_bmoc: Self::Value,
    right_bmoc: Self::Value,
  ) -> Result<Self::Value, Self::Error> {
    // Warning: we interpret 'difference' as being a 'symmetrical difference', i.e. xor (not minus)
    Ok(left_bmoc.xor(&right_bmoc))
  }
}

impl SpaceVisitor for Stc2Moc {
  type Value = RangeMOC<u64, Hpx<u64>>;
  type Error = Stc2MocError;
  type C = Self;

  fn new_compound_visitor(
    &self,
    fill_frame_refpos_flavor: &FillFrameRefposFlavor,
    from_pos_to_velocity: &FromPosToVelocity,
  ) -> Result<Self, Self::Error> {
    // Check ICRS frame
    let frame = fill_frame_refpos_flavor.frame();
    if frame != Frame::ICRS {
      return Err(Stc2MocError::FrameIsNotICRS { found: frame });
    }
    // Check SPHER2 flavor
    let flavor = fill_frame_refpos_flavor.flavor();
    if let Some(flavor) = flavor {
      if flavor != Flavor::Spher2 {
        return Err(Stc2MocError::FlavorIsNotSpher2 { found: flavor });
      }
    }
    // Check units
    let opt_units = from_pos_to_velocity.unit().cloned();
    if let Some(units) = opt_units {
      for unit in units.iter().cloned() {
        if unit != SpaceUnit::Deg {
          return Err(Stc2MocError::UnitsNotSupported { found: units });
        }
      }
    }
    Ok(self.clone())
  }

  fn visit_position_simple(self, _: &Position) -> Result<Self::Value, Self::Error> {
    Err(Stc2MocError::SimplePositionNotSupported)
  }

  fn visit_position_interval(
    self,
    interval: &PositionInterval,
  ) -> Result<Self::Value, Self::Error> {
    // We use compound visitor only to check interval parameters
    self.new_compound_visitor(&interval.pre, &interval.post)?;
    let depth = self.depth;
    let corners = interval
      .lo_hi_limits
      .iter()
      .step_by(2)
      .zip(interval.lo_hi_limits.iter().skip(1).step_by(2))
      .map(|(lon_deg, lat_deg)| {
        let lon = lon_deg2rad(*lon_deg)?;
        let lat = lat_deg2rad(*lat_deg)?;
        Ok((lon, lat))
      })
      .collect::<Result<Vec<(f64, f64)>, Stc2MocError>>()?;
    let mut corners_it = corners
      .iter()
      .cloned()
      .step_by(2)
      .zip(corners.iter().cloned().skip(1).step_by(2));
    if let Some(((ra_min, dec_min), (ra_max, dec_max))) = corners_it.next() {
      let mut bmoc = zone_coverage(depth, ra_min, dec_min, ra_max, dec_max);
      for ((ra_min, dec_min), (ra_max, dec_max)) in corners_it {
        bmoc = bmoc.or(&zone_coverage(depth, ra_min, dec_min, ra_max, dec_max));
      }
      Ok(bmoc.into())
    } else {
      Ok(RangeMOC::<u64, Hpx<u64>>::new_empty(depth))
    }
  }

  fn visit_allsky(self, bmoc: BMOC) -> Result<Self::Value, Self::Error> {
    Ok(Self::Value::from(bmoc))
  }

  fn visit_circle(self, bmoc: BMOC) -> Result<Self::Value, Self::Error> {
    Ok(bmoc.into())
  }

  fn visit_ellipse(self, bmoc: BMOC) -> Result<Self::Value, Self::Error> {
    Ok(bmoc.into())
  }

  fn visit_box(self, bmoc: BMOC) -> Result<Self::Value, Self::Error> {
    Ok(bmoc.into())
  }

  fn visit_polygon(self, bmoc: BMOC) -> Result<Self::Value, Self::Error> {
    Ok(bmoc.into())
  }

  fn visit_convex(self, _: BMOC) -> Result<Self::Value, Self::Error> {
    unreachable!() // because an error is raised before calling this
  }

  fn visit_not(self, bmoc: BMOC) -> Result<Self::Value, Self::Error> {
    Ok(bmoc.into())
  }

  fn visit_union(self, bmoc: BMOC) -> Result<Self::Value, Self::Error> {
    Ok(bmoc.into())
  }

  fn visit_intersection(self, bmoc: BMOC) -> Result<Self::Value, Self::Error> {
    Ok(bmoc.into())
  }

  fn visit_difference(self, bmoc: BMOC) -> Result<Self::Value, Self::Error> {
    Ok(bmoc.into())
  }
}

fn lon_deg2rad(lon_deg: f64) -> Result<f64, Stc2MocError> {
  let mut lon = lon_deg.to_radians();
  if lon == TWICE_PI {
    lon = 0.0;
  }
  if !(0.0..TWICE_PI).contains(&lon) {
    Err(Stc2MocError::WrongLongitude { value: lon_deg })
  } else {
    Ok(lon)
  }
}

fn lat_deg2rad(lat_deg: f64) -> Result<f64, Stc2MocError> {
  let lat = lat_deg.to_radians();
  if !(-HALF_PI..=HALF_PI).contains(&lat) {
    Err(Stc2MocError::WrongLatitude { value: lat_deg })
  } else {
    Ok(lat)
  }
}

/// Create new S-MOC from the given STC-S string.
///
/// # WARNING
/// * `DIFFERENCE` is interpreted as a symmetrical difference (it is a `MINUS` in the STC standard)
/// * `Polygon` do not follow the STC-S standard: here self-intersecting polygons are supported
/// * No implicit conversion: the STC-S will be rejected if
///     + the frame is different from `ICRS`
///     + the flavor is different from `Spher2`
///     + the units are different from `degrees`
/// * Time, Spectral and Redshift sub-phrases are ignored
///
/// # Params
/// * `depth`: MOC maximum depth in `[0, 29]`
/// * `delta_depth` the difference between the MOC depth and the depth at which the computations
///   are made (should remain quite small).  
/// * `ascii_stcs`: lthe STC-S string
///
/// # Output
/// - The new S-MOC (or an error)
pub fn stcs2moc(
  depth: u8,
  delta_depth: Option<u8>,
  stcs_ascii: &str,
) -> Result<RangeMOC<u64, Hpx<u64>>, Stc2MocError> {
  match Stc::parse::<VerboseError<&str>>(stcs_ascii.trim()) {
    Ok((rem, stcs)) => {
      if !rem.is_empty() {
        return Err(Stc2MocError::ParseHasRemaining {
          rem: rem.to_string(),
        });
      }
      let stc2moc_visitor = Stc2Moc::new(depth, delta_depth);
      let StcVisitResult { space, .. } =
        stcs.accept(VoidVisitor, stc2moc_visitor, VoidVisitor, VoidVisitor);
      match space {
        None => Err(Stc2MocError::NoSpaceFound),
        Some(space_res) => space_res,
      }
    }
    Err(err) => Err(match err {
      Err::Incomplete(_) => Stc2MocError::ParseIncomplete {
        msg: String::from("Incomplete parsing."),
      },
      Err::Error(e) => Stc2MocError::ParseIncomplete {
        msg: convert_error(stcs_ascii, e),
      },
      Err::Failure(e) => Stc2MocError::ParseIncomplete {
        msg: convert_error(stcs_ascii, e),
      },
    }),
  }
}

#[cfg(test)]
mod tests {
  use crate::{moc::range::RangeMOC, qty::Hpx};

  use super::stcs2moc;

  #[test]
  fn test_from_stcs_circle() {
    let moc1 = stcs2moc(10, Some(2), "Circle ICRS 147.6 69.9 0.4").unwrap();
    let moc2 = RangeMOC::<u64, Hpx<u64>>::from_cone(
      147.6_f64.to_radians(),
      69.9_f64.to_radians(),
      0.4_f64.to_radians(),
      10,
      2,
    );
    assert_eq!(moc1, moc2)
  }

  #[test]
  fn test_from_stcs_allsky() {
    let moc1 = stcs2moc(10, Some(2), "Allsky ICRS").unwrap();
    let moc2 = RangeMOC::<u64, Hpx<u64>>::new_full_domain(10);
    assert_eq!(moc1, moc2);
    let moc1 = stcs2moc(18, Some(2), "Allsky ICRS").unwrap();
    let moc2 = RangeMOC::<u64, Hpx<u64>>::new_full_domain(18);
    assert_eq!(moc1, moc2);
  }

  #[test]
  fn test_from_stcs_not_allsky() {
    let moc1 = stcs2moc(15, Some(2), "Not ICRS(Allsky)").unwrap();
    let moc2 = RangeMOC::<u64, Hpx<u64>>::new_empty(15);
    assert_eq!(moc1, moc2);
  }

  #[test]
  fn test_from_stcs_union_difference_intersection() {
    let stcs1 = r#"
Difference ICRS (
    Polygon 272.536719 -19.461249 272.542612 -19.476380 272.537389 -19.491509 272.540192 -19.499823
            272.535455 -19.505218 272.528024 -19.505216 272.523437 -19.500298 272.514082 -19.503376
            272.502271 -19.500966 272.488647 -19.490390  272.481932 -19.490913 272.476737 -19.486589
            272.487633 -19.455645 272.500386 -19.444996 272.503003 -19.437557 272.512303 -19.432436
            272.514132 -19.423973 272.522103 -19.421523 272.524511 -19.413250 272.541021 -19.400024
            272.566264 -19.397500 272.564202 -19.389111 272.569055 -19.383210 272.588186 -19.386539
            272.593376 -19.381832 272.596327 -19.370541 272.624911 -19.358915 272.629256 -19.347842
            272.642277 -19.341020 272.651322 -19.330424 272.653174 -19.325079 272.648903 -19.313708
            272.639616 -19.311098 272.638128 -19.303083 272.632705 -19.299839 272.627971 -19.289408
            272.628226 -19.276293 272.633750 -19.270590 272.615109 -19.241810 272.614704 -19.221196
            272.618224 -19.215572 272.630809 -19.209945 272.633540 -19.198681 272.640711 -19.195292
            272.643028 -19.186751 272.651477 -19.182729 272.649821 -19.174859 272.656782 -19.169272
            272.658933 -19.161883 272.678012 -19.159481 272.689173 -19.176982 272.689395 -19.183512
            272.678006 -19.204016 272.671112 -19.206598 272.664854 -19.203523 272.662760 -19.211156
            272.654435 -19.214434 272.652969 -19.222085 272.656724 -19.242136 272.650071 -19.265092
            272.652868 -19.274296 272.660871 -19.249462 272.670041 -19.247807 272.675533 -19.254935
            272.673291 -19.273917 272.668710 -19.279245 272.671460 -19.287043 272.667507 -19.293933
            272.669261 -19.300601 272.663969 -19.307130 272.672626 -19.308954 272.675225 -19.316490
            272.657188 -19.349105 272.657638 -19.367455 272.662447 -19.372035 272.662232 -19.378566
            272.652479 -19.386871 272.645819 -19.387933 272.642279 -19.398277 272.629282 -19.402739
            272.621487 -19.398197 272.611782 -19.405716 272.603367 -19.404667 272.586162 -19.422703
            272.561792 -19.420008 272.555815 -19.413012 272.546500 -19.415611 272.537427 -19.424213
            272.533081 -19.441402
    Union (
        Polygon 272.511081 -19.487278 272.515300 -19.486595 272.517029 -19.471442
                272.511714 -19.458837 272.506430 -19.459001 272.496401 -19.474322 272.504821 -19.484924
        Polygon 272.630446 -19.234210 272.637274 -19.248542 272.638942 -19.231476 272.630868 -19.226364
    )
)"#;
    let stcs2 = r#"
Intersection ICRS (
    Polygon 272.536719 -19.461249 272.542612 -19.476380 272.537389 -19.491509 272.540192 -19.499823
            272.535455 -19.505218 272.528024 -19.505216 272.523437 -19.500298 272.514082 -19.503376 
            272.502271 -19.500966 272.488647 -19.490390  272.481932 -19.490913 272.476737 -19.486589 
            272.487633 -19.455645 272.500386 -19.444996 272.503003 -19.437557 272.512303 -19.432436 
            272.514132 -19.423973 272.522103 -19.421523 272.524511 -19.413250 272.541021 -19.400024 
            272.566264 -19.397500 272.564202 -19.389111 272.569055 -19.383210 272.588186 -19.386539 
            272.593376 -19.381832 272.596327 -19.370541 272.624911 -19.358915 272.629256 -19.347842 
            272.642277 -19.341020 272.651322 -19.330424 272.653174 -19.325079 272.648903 -19.313708 
            272.639616 -19.311098 272.638128 -19.303083 272.632705 -19.299839 272.627971 -19.289408 
            272.628226 -19.276293 272.633750 -19.270590 272.615109 -19.241810 272.614704 -19.221196 
            272.618224 -19.215572 272.630809 -19.209945 272.633540 -19.198681 272.640711 -19.195292 
            272.643028 -19.186751 272.651477 -19.182729 272.649821 -19.174859 272.656782 -19.169272 
            272.658933 -19.161883 272.678012 -19.159481 272.689173 -19.176982 272.689395 -19.183512 
            272.678006 -19.204016 272.671112 -19.206598 272.664854 -19.203523 272.662760 -19.211156 
            272.654435 -19.214434 272.652969 -19.222085 272.656724 -19.242136 272.650071 -19.265092
            272.652868 -19.274296 272.660871 -19.249462 272.670041 -19.247807 272.675533 -19.254935 
            272.673291 -19.273917 272.668710 -19.279245 272.671460 -19.287043 272.667507 -19.293933
            272.669261 -19.300601 272.663969 -19.307130 272.672626 -19.308954 272.675225 -19.316490
            272.657188 -19.349105 272.657638 -19.367455 272.662447 -19.372035 272.662232 -19.378566
            272.652479 -19.386871 272.645819 -19.387933 272.642279 -19.398277 272.629282 -19.402739
            272.621487 -19.398197 272.611782 -19.405716 272.603367 -19.404667 272.586162 -19.422703
            272.561792 -19.420008 272.555815 -19.413012 272.546500 -19.415611 272.537427 -19.424213
            272.533081 -19.441402 
    Not (Polygon 272.511081 -19.487278 272.515300 -19.486595 272.517029 -19.471442 
                 272.511714 -19.458837 272.506430 -19.459001 272.496401 -19.474322 272.504821 -19.484924)
    Not (Polygon 272.630446 -19.234210 272.637274 -19.248542 272.638942 -19.231476 272.630868 -19.226364)
)"#;
    let moc1 = stcs2moc(16, Some(2), stcs1).unwrap();
    let moc2 = stcs2moc(16, Some(2), stcs2).unwrap();
    assert_eq!(moc1, moc2);

    // Write file to check it manually
    /*
    use crate::moc::{RangeMOCIntoIterator, RangeMOCIterator};
    use std::{fs::File, io::BufWriter, path::PathBuf};
    let path_buf1 = PathBuf::from("resources/stcs/eso.res.moc.fits");
    let file = File::create(&path_buf1).unwrap();
    let writer = BufWriter::new(file);
    moc1
      .into_range_moc_iter()
      .to_fits_ivoa(None, None, writer)
      .unwrap();
     */
  }

  #[test]
  fn test_from_stcs_position_interval() {
    let moc = stcs2moc(
      10,
      Some(2),
      "PositionInterval ICRS 170 -20 190 10 Resolution 0.0001",
    )
    .unwrap();
    assert_eq!(moc.len(), 1015);

    let moc = stcs2moc(
      10,
      Some(2),
      "PositionInterval ICRS 170 -20 190 10 200 10 210 20 Resolution 0.0001",
    )
    .unwrap();
    assert_eq!(moc.len(), 1383);
  }
}
