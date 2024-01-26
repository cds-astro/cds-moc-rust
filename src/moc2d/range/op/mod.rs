//! This module contains the structures used to perform operations on 2D Range MOCs iterators.

pub mod or; // <=> union
            // pub mod and; // <=> intersection TODO

#[cfg(test)]
mod tests {

  use core::ops::Range;

  use std::fs::File;
  use std::io::BufReader;
  use std::path::PathBuf;

  use crate::deser::{
    ascii::{moc2d_from_ascii_ivoa, moc2d_to_ascii_ivoa},
    fits::{from_fits_ivoa, MocIdxType, MocQtyType, STMocType},
  };
  use crate::moc2d::{
    range::*, CellOrCellRangeMOC2IntoIterator, RangeMOC2, RangeMOC2Elem, RangeMOC2ElemIt,
    RangeMOC2IntoIterator, RangeMOC2Iterator,
  };
  use crate::qty::{Hpx, MocQty, Time};
  use crate::ranges::Ranges;

  fn create_moc2_at_max_depth(
    elems: Vec<(Range<u64>, Range<u64>)>,
  ) -> RangeMOC2<u64, Time<u64>, u64, Hpx<u64>> {
    let elems = elems
      .into_iter()
      .map(|(trange, srange)| {
        RangeMOC2Elem::new(
          RangeMOC::new(
            Time::<u64>::MAX_DEPTH,
            Ranges::new_unchecked(vec![trange]).into(),
          ),
          RangeMOC::new(
            Hpx::<u64>::MAX_DEPTH,
            Ranges::new_unchecked(vec![srange]).into(),
          ),
        )
      })
      .collect::<Vec<RangeMOC2Elem<u64, Time<u64>, u64, Hpx<u64>>>>();
    RangeMOC2::new(Time::<u64>::MAX_DEPTH, Hpx::<u64>::MAX_DEPTH, elems)
  }

  fn create_moc2_at_max_depth_v2(
    elems: Vec<(Vec<Range<u64>>, Vec<Range<u64>>)>,
  ) -> RangeMOC2<u64, Time<u64>, u64, Hpx<u64>> {
    let elems = elems
      .into_iter()
      .map(|(tranges, sranges)| {
        RangeMOC2Elem::new(
          RangeMOC::new(
            Time::<u64>::MAX_DEPTH,
            Ranges::new_unchecked(tranges).into(),
          ),
          RangeMOC::new(Hpx::<u64>::MAX_DEPTH, Ranges::new_unchecked(sranges).into()),
        )
      })
      .collect::<Vec<RangeMOC2Elem<u64, Time<u64>, u64, Hpx<u64>>>>();
    RangeMOC2::new(Time::<u64>::MAX_DEPTH, Hpx::<u64>::MAX_DEPTH, elems)
  }

  #[test]
  fn union_ranges_1_3() {
    let a = create_moc2_at_max_depth(vec![(0..10, 16..21)]);
    let b = create_moc2_at_max_depth(vec![(10..20, 16..21)]);

    let c = a.or(&b);

    let res = create_moc2_at_max_depth(vec![(0..20, 16..21)]);
    assert_eq!(res, c);
  }
  #[test]
  fn union_ranges_1_3_bis() {
    let a = create_moc2_at_max_depth(vec![(0..10, 16..21)]);
    let b = create_moc2_at_max_depth(vec![(10..20, 16..22)]);

    let c = a.or(&b);

    let res = create_moc2_at_max_depth(vec![(0..10, 16..21), (10..20, 16..22)]);
    assert_eq!(res, c);
  }
  #[test]
  fn union_ranges_covering() {
    let a = create_moc2_at_max_depth(vec![(0..10, 16..21)]);
    let b = create_moc2_at_max_depth(vec![(9..20, 0..17)]);

    let c = a.or(&b);

    let res = create_moc2_at_max_depth(vec![(0..9, 16..21), (9..10, 0..21), (10..20, 0..17)]);
    assert_eq!(res, c);
  }

  #[test]
  fn empty_range_union() {
    let a = create_moc2_at_max_depth(vec![(0..1, 42..43)]);
    let b = create_moc2_at_max_depth(vec![(9..20, 0..17)]);

    let c = a.or(&b);

    let res = create_moc2_at_max_depth(vec![(0..1, 42..43), (9..20, 0..17)]);
    assert_eq!(res, c);
  }

  #[test]
  fn empty_range_union_bis() {
    let b = create_moc2_at_max_depth(vec![(0..9, 0..20)]);
    let a = create_moc2_at_max_depth(vec![(9..20, 0..17)]);

    let c = a.or(&b);

    let res = create_moc2_at_max_depth(vec![(0..9, 0..20), (9..20, 0..17)]);
    assert_eq!(res, c);
  }

  #[test]
  fn complex_union() {
    let a = create_moc2_at_max_depth_v2(vec![
      (vec![0..2, 3..5], vec![2..3]),
      (vec![8..9], vec![5..6]),
      (vec![13..14], vec![7..8]),
    ]);
    let b = create_moc2_at_max_depth_v2(vec![
      (vec![1..4], vec![0..3]),
      (vec![6..7, 9..10], vec![5..6]),
      (vec![11..12], vec![10..13]),
    ]);

    let result = a.or(&b);
    let expected = create_moc2_at_max_depth_v2(vec![
      (vec![0..1], vec![2..3]), // ok
      (vec![1..4], vec![0..3]), // 1..2, 0..3
      (vec![4..5], vec![2..3]),
      (vec![6..7, 8..10], vec![5..6]),
      (vec![11..12], vec![10..13]),
      (vec![13..14], vec![7..8]),
    ]);
    if expected != result {
      for (a, b) in (&result)
        .into_range_moc2_iter()
        .zip((&expected).into_range_moc2_iter())
      {
        let (l1, l2) = a.range_mocs_it();
        let (r1, r2) = b.range_mocs_it();
        println!("MOC1 L: {:?}", l1.into_range_moc());
        println!("MOC1 R: {:?}", r1.into_range_moc());
        println!("MOC2 L: {:?}", l2.into_range_moc());
        println!("MOC2 R: {:?}", r2.into_range_moc());
        println!("--------");
      }
    }
    assert_eq!(expected, result);
  }

  fn test_union_from_ascii(label: &str, left: &str, right: &str, result: &str) {
    println!("{}", label);
    let left_it = moc2d_from_ascii_ivoa::<u64, Time<u64>, u64, Hpx<u64>>(left)
      .unwrap()
      .into_cellcellrange_moc2_iter()
      .into_range_moc2_iter(); //.into_range_moc2();
    let right_it = moc2d_from_ascii_ivoa::<u64, Time<u64>, u64, Hpx<u64>>(right)
      .unwrap()
      .into_cellcellrange_moc2_iter()
      .into_range_moc2_iter(); //.into_range_moc2();
    let expected = moc2d_from_ascii_ivoa::<u64, Time<u64>, u64, Hpx<u64>>(result)
      .unwrap()
      .into_cellcellrange_moc2_iter()
      .into_range_moc2_iter()
      .into_range_moc2();
    let actual = left_it.or(right_it).into_range_moc2();
    assert_eq!(actual, expected);
  }

  #[test]
  fn test_union_from_java_code() {
    test_union_from_ascii(
      "Ajout à vide",
      "",
      "t61/5-10 s29/2",
      "t60/3-4 61/5 10 s29/2",
    );
    test_union_from_ascii(
      "Ajout singleton derrière singleton",
      "t61/4 s29/1",
      "t61/5 s29/2",
      "t61/4 s29/1 t61/5 s29/2",
    );
    test_union_from_ascii(
      "Ajout singleton avant singleton",
      "t61/5 s29/2",
      "t61/4 s29/1",
      "t61/4 s29/1 t61/5 s29/2",
    );
    test_union_from_ascii(
      "Ajout intervalle entrelacés après",
      "t61/4-6 s29/1",
      "t61/5-8 s29/2",
      "t61/4 s29/1 t61/5-6 s29/1-2 t61/7-8 s29/2",
    );
    test_union_from_ascii(
      "Ajout intervalle entrelacés avant",
      "t61/5-8 s29/2",
      "t61/4-6 s29/1",
      "t61/4 s29/1 t61/5-6 s29/1-2 t61/7-8 s29/2",
    );
    test_union_from_ascii(
      "Ajout intervalle englobant (s différents)",
      "t61/2-6 s29/2",
      "t61/1-8 s29/1",
      "t61/1 s29/1 t60/1-2 61/6 s29/1-2 t61/7-8 s29/1",
    );
    test_union_from_ascii(
      "Ajout intervalle englobant (s identiques)",
      "t61/2-6 s29/2",
      "t61/1-8 s29/2",
      "t59/1 60/1 61/1 8 s29/2",
    );
    test_union_from_ascii(
      "Ajout intervalle interne (s différents)",
      "t61/1-8 s29/1",
      "t61/2-6 s29/2",
      "t61/1 s29/1 t60/1-2 61/6 s29/1-2 t61/7-8 s29/1",
    );
    test_union_from_ascii(
      "Ajout intervalle interne (s identiques)",
      "t61/1-8 s29/2",
      "t61/2-6 s29/2",
      "t59/1 60/1 61/1 8 s29/2",
    );
    test_union_from_ascii(
      "Intercallage",
      "t61/6-7 11 s29/1",
      "t61/9 s29/2",
      "t60/3 61/ s29/1 t61/9 s29/2 t61/11 s29/1",
    );
    test_union_from_ascii(
      "Fusion différents s",
      "t61/2-6 8-9 s29/2",
      "t61/7 s29/1",
      "t60/1-2 61/6 s29/2 t61/7 s29/1 t60/4 61/ s29/2",
    );
    test_union_from_ascii(
      "Fusion indentiques s",
      "t61/2-6 8-9 s29/2",
      "t61/7 s29/2",
      "t59/1 60/1 4 61/ s29/2",
    );
    test_union_from_ascii(
      "Remplacement sur début",
      "t61/2-6 s29/2 t61/7 s29/1",
      "t61/2-7 s29/2",
      "t60/1-2 61/6 s29/2 t61/7 s29/1-2",
    );
    test_union_from_ascii(
      "Remplacement sur fin",
      "t61/3-7 s29/2 t61/8 s29/1",
      "t61/2-7 s29/2",
      "t59/1 60/1 61/ s29/2 t61/8 s29/1",
    );
    test_union_from_ascii(
      "Remplacement sur fin2",
      "t61/2-4 s29/2 t61/6 s29/1",
      "t61/6 s29/2",
      "t60/1 61/4 s29/2 t61/6 s29/1-2",
    );
    test_union_from_ascii(
      "Tordu",
      "t61/3 s29/1 t61/4-5 s29/2",
      "t61/3-5 s29/3",
      "t61/3 s29/1 3 t60/2 61/ s29/2-3",
    );
    test_union_from_ascii(
      "Inter simple",
      "t61/3-5 s29/1-3",
      "t61/4-8 s29/2-4",
      "t61/3 s29/1-3 t60/2 61/ s29/1-4 t60/3 61/8 s29/2-4",
    );
    test_union_from_ascii(
      "Inter spécial",
      "t61/1 s29/1-6 t61/3-9 s29/2",
      "t61/3 s29/5-7 t61/8 s29/1-2",
      "t61/1 s29/1-6 t61/3 s29/2 5-7 t59/1 61/ s29/2 t61/8 s29/1-2 t61/9 s29/2",
    );
    test_union_from_ascii(
      "Ajout en suite",
      "t61/1-4 s29/1",
      "t61/5-6 s29/1",
      "t60/1-2 61/1 6 s29/1",
    );
  }

  #[test]
  fn test_union_from_resource() {
    let left = r#"
      t23/771387
      s7/88307-88308
      8/353147 353222-353223 353236 353238 353253 353268-353270 353272 353930
      9/71272 1412586-1412587 1412602 1412878-1412879 1412883 1412885-1412887
       1412900-1412901 1412903 1412909 1412956 1412958-1412959 1413009 1413021
       1413023 1413084 1413092-1413093 1415312 1415324 1415406 1415712 1415714
       1415726 1415808-1415810
    "#;

    let right = r#"
      t29/49368772
      s10/7482753
      t29/49368773
      s10/5651750
      t29/49368774
      s10/5651553
      t29/49368778
      s10/5662880 10706912
    "#;

    let expected = r#"t27/12342192 
29/
s7/88307-88308 
8/353147 353222-353223 353236 353238 353253 353268-353270 353272 353930 
9/71272 1412586-1412587 1412602 1412878-1412879 1412883 1412885-1412887 
 1412900-1412901 1412903 1412909 1412956 1412958-1412959 1413009 1413021 
 1413023 1413084 1413092-1413093 1415312 1415324 1415406 1415712 1415714 
 1415726 1415808-1415810 
t29/49368772 
s7/88307-88308 
8/353147 353222-353223 353236 353238 353253 353268-353270 353272 353930 
9/71272 1412586-1412587 1412602 1412878-1412879 1412883 1412885-1412887 
 1412900-1412901 1412903 1412909 1412956 1412958-1412959 1413009 1413021 
 1413023 1413084 1413092-1413093 1415312 1415324 1415406 1415712 1415714 
 1415726 1415808-1415810 
10/7482753 
t28/24684387-24684388 
29/49368773 
s7/88307-88308 
8/353147 353222-353223 353236 353238 353253 353268-353270 353272 353930 
9/71272 1412586-1412587 1412602 1412878-1412879 1412883 1412885-1412887 
 1412900-1412901 1412903 1412909 1412956 1412958-1412959 1413009 1413021 
 1413023 1413084 1413092-1413093 1415312 1415324 1415406 1415712 1415714 
 1415726 1415808-1415810 
t29/49368778 
s7/88307-88308 
8/353147 353222-353223 353236 353238 353253 353268-353270 353272 353930 
9/71272 1412586-1412587 1412602 1412878-1412879 1412883 1412885-1412887 
 1412900-1412901 1412903 1412909 1412956 1412958-1412959 1413009 1413021 
 1413023 1413084 1413092-1413093 1415312 1415324 1415406 1415712 1415714 
 1415726 1415808-1415810 
10/10706912 
t23/771387 
s7/88307-88308 
8/353147 353222-353223 353236 353238 353253 353268-353270 353272 353930 
9/71272 1412586-1412587 1412602 1412878-1412879 1412883 1412885-1412887 
 1412900-1412901 1412903 1412909 1412956 1412958-1412959 1413009 1413021 
 1413023 1413084 1413092-1413093 1415312 1415324 1415406 1415712 1415714 
 1415726 1415808-1415810 
"#;

    let left_it = moc2d_from_ascii_ivoa::<u64, Time<u64>, u64, Hpx<u64>>(left)
      .unwrap()
      .into_cellcellrange_moc2_iter()
      .into_range_moc2_iter(); //.into_range_moc2();
    let right_it = moc2d_from_ascii_ivoa::<u64, Time<u64>, u64, Hpx<u64>>(right)
      .unwrap()
      .into_cellcellrange_moc2_iter()
      .into_range_moc2_iter(); //.into_range_moc2();

    let actual = left_it.or(right_it).into_range_moc2();

    /*for elem in (&actual).into_range_moc2_iter() {
      let (moc1_it, moc2_it) = elem.range_mocs_it();
      for e in moc1_it {
        println!(" - t: {:?}", e);
      }
      for e in moc2_it {
        println!(" - s: {:?}", e);
      }
    }*/

    /*
    for elem in actual.into_range_moc2_iter().into_cellcellrange_moc2_iter() {
      let (moc1_it, moc2_it) = elem.cellcellrange_mocs_it();
      for e in moc1_it {
        println!(" - t: {:?}", e);
      }
      for e in moc2_it {
        println!(" - s: {:?}", e);
      }
    }
    */

    let mut actual_ascii = Vec::new();
    moc2d_to_ascii_ivoa(
      (&actual)
        .into_range_moc2_iter()
        .into_cellcellrange_moc2_iter(),
      &Some(80),
      false,
      &mut actual_ascii,
    )
    .unwrap();
    // println!("{}", std::str::from_utf8(&actual_ascii).unwrap());

    assert_eq!(actual_ascii, expected.as_bytes());
  }

  fn test_union_with_resource(l_path: &str, r_path: &str, expected_stats: (u64, u64, u64)) {
    let path_buf_l = PathBuf::from(l_path);
    let reader_l = BufReader::new(File::open(&path_buf_l).unwrap());
    let it_left = match from_fits_ivoa(reader_l).unwrap() {
      MocIdxType::U64(MocQtyType::TimeHpx(STMocType::V2(it))) => it,
      _ => unreachable!(),
    };

    let path_buf_r = PathBuf::from(r_path);
    let reader_r = BufReader::new(File::open(&path_buf_r).unwrap());
    let it_right = match from_fits_ivoa(reader_r).unwrap() {
      MocIdxType::U64(MocQtyType::TimeHpx(STMocType::V2(it))) => it,
      _ => unreachable!(),
    };

    // Bench (else we could have directly used it_left.or(it_right)
    let l = it_left.into_range_moc2();
    let r = it_right.into_range_moc2();

    use std::time::SystemTime;
    let now = SystemTime::now();
    let actual = l.into_or(r);
    match now.elapsed() {
      Ok(elapsed) => println!("{} ms", elapsed.as_millis()),
      Err(e) => println!("Error: {:?}", e),
    };
    // Stats
    let stats = actual.into_range_moc2_iter().stats();
    // println!("{:?}", &stats);
    assert_eq!(stats, expected_stats);

    /*
    use std::io::BufWriter;
    use crate::deser::fits::ranges2d_to_fits_ivoa;
    use crate::deser::json::cellmoc2d_to_json_aladin;
    */
    // let file = File::create(PathBuf::from("resources/MOC2.0/STMOC_union_assocdata_vsx.txt")).unwrap();
    // moc2d_to_ascii_ivoa(it_left.or(it_right).into_cellcellrange_moc2_iter(), &Some(80), false, BufWriter::new(file)).unwrap();
    // moc2d_to_ascii_ivoa((&actual).into_range_moc2_iter().into_cellcellrange_moc2_iter(), &Some(80), false, BufWriter::new(file)).unwrap();

    // let file = File::create(PathBuf::from("resources/MOC2.0/STMOC_union_assocdata_vsx.from_tests.json")).unwrap();
    // cellmoc2d_to_json_aladin(it_left.or(it_right).into_cell_moc2_iter(), &Some(80), BufWriter::new(file)).unwrap();
    /*
    let file = File::create(PathBuf::from("resources/MOC2.0/STMOC_union_assocdata_vsx.fits")).unwrap();
    ranges2d_to_fits_ivoa(it_left.or(it_right), None, None, BufWriter::new(file)).unwrap();
    */
    /*let path_buf_res = PathBuf::from("resources/MOC2.0/res.fits");
    let reader_res = BufReader::new(File::open(&path_buf_res).unwrap());
    let mut it_res = match from_fits_ivoa(reader_res).unwrap() {
      MocIdxType::U64(MocQtyType::TimeHpx(STMocType::V2(it))) => it,
      _ => unreachable!(),
    };*/
    // let res = it_res.into_range_moc2();
  }

  #[test]
  fn test_union_xmm_chandra() {
    test_union_with_resource(
      "resources/MOC2.0/STMOC_XMMLog.fits",
      "resources/MOC2.0/STMOC_chandra.fits",
      (36196, 46210, 37722),
    );
  }

  #[test]
  fn test_union_assocdata_vsx() {
    test_union_with_resource(
      "resources/MOC2.0/STMOC_assocdata.fits",
      "resources/MOC2.0/STMOC_vsx.fits",
      (82996, 83003, 21188795),
    );
  }
}
