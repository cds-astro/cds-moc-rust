//! The MOC library contains the core functionalities to create and maniuplates MOCs.
//!
//! it is used in [MOCPy](https://github.com/cds-astro/mocpy),
//! [moc-cli](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/cli) and
//! [moc-wasm](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/wasm).
//!
//! The library is not (yet?) properly documented.
//! To use it, we so far recommend to look at the source code of the tools using it
//! (moc-wasm for example).
//!

#[cfg(not(target_arch = "wasm32"))]
use rayon::ThreadPoolBuildError;

pub mod elem;
pub mod elemset;
pub mod idx;
pub mod moc;
pub mod qty;
pub mod ranges;

pub mod hpxranges2d;
pub mod moc2d;
pub mod mocranges2d;

pub mod deser;

#[cfg(feature = "storage")]
pub mod storage;
pub mod utils;

// from_fits
// from_cone
// from_...

/// Init the number of threads for parallel tasks.
/// Must be called only once!
/// If not called, the default number of threads is the number of physical core.
/// See [rayon doc](https://docs.rs/rayon/1.5.1/rayon/struct.ThreadPoolBuilder.html)
#[cfg(not(target_arch = "wasm32"))]
pub fn init_par(num_threads: usize) -> Result<(), ThreadPoolBuildError> {
  rayon::ThreadPoolBuilder::new()
    .num_threads(num_threads)
    .build_global()
}

#[cfg(test)]
mod tests {
  use num::PrimInt;

  use crate::elemset::range::{uniq::HpxUniqRanges, HpxRanges};

  #[test]
  fn test_uniq_iter() {
    let simple_nested = HpxRanges::<u64>::new_unchecked(vec![0..1]);
    let complex_nested = HpxRanges::<u64>::new_unchecked(vec![7..76]);
    let empty_nested = HpxRanges::<u64>::default();

    let simple_uniq = HpxUniqRanges::<u64>::new_unchecked(vec![4 * 4.pow(29)..(4 * 4.pow(29) + 1)]);
    let complex_uniq = HpxUniqRanges::<u64>::new_from_sorted(vec![
      (1 + 4 * 4.pow(27))..(4 + 4 * 4.pow(27)),
      (2 + 4 * 4.pow(28))..(4 + 4 * 4.pow(28)),
      (16 + 4 * 4.pow(28))..(19 + 4 * 4.pow(28)),
      (7 + 4 * 4.pow(29))..(8 + 4 * 4.pow(29)),
    ]);
    let empty_uniq = HpxUniqRanges::<u64>::new_unchecked(vec![]);

    assert_eq!(simple_nested.clone().into_hpx_uniq(), simple_uniq);
    assert_eq!(complex_nested.clone().into_hpx_uniq(), complex_uniq);
    assert_eq!(empty_nested.clone().into_hpx_uniq(), empty_uniq);

    assert_eq!(simple_uniq.into_hpx(), simple_nested);
    assert_eq!(complex_uniq.into_hpx(), complex_nested);
    assert_eq!(empty_uniq.into_hpx(), empty_nested);
  }

  #[test]
  fn test_uniq_nested_conversion() {
    let input = vec![
      1056..1057,
      1057..1058,
      1083..1084,
      1048539..1048540,
      1048574..1048575,
      1048575..1048576,
    ];

    let ranges = HpxUniqRanges::<u64>::new_from_sorted(input.clone());
    let expected = HpxUniqRanges::<u64>::new_from_sorted(input);

    assert_eq!(ranges.into_hpx().into_hpx_uniq(), expected);
  }
}
