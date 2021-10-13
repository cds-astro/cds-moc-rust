#!/bin/bash

moc="moc"
resources="resources"

${moc} -V
[[ "$?" != 0 ]] && { echo "'moc' command line not found!"; exit 1; }

# 1: cmd, 2: expected
test(){
  cmd="$1"
  actual=$(bash -c "${cmd}")
  expected="$2"
  [[ "${actual}" != "${expected}" ]] && { printf "Test failed!\n- cmd: '${cmd}'\n- actual: '${actual}'\n- expect: '${expected}'\n"; }
}

# 1: cmd, 2: cmd
test_eq(){
  cmd1="$1"
  cmd2="$2"
  res1=$(bash -c "${cmd1}")
  res2=$(bash -c "${cmd2}")
  [[ "${res1}" != "${res2}" ]] && { printf "Test failed!\n- cmd1: '${cmd1}'\n- cmd2: '${cmd2}'\n- res1: '${res1}'\n- res2: '${res2}'\n"; }
}

echo "Conversion tests..."
# Conversion
${moc} convert ${resources}/glimpse.moc.json --type smoc fits --force-u64 SMOC_GLIMPSE_u64.fits
${moc} convert ${resources}/glimpse.moc.json --type smoc fits             SMOC_GLIMPSE_u32.fits
test_eq "${moc} convert SMOC_GLIMPSE_u32.fits ascii --fold 80" "${moc} convert SMOC_GLIMPSE_u64.fits ascii --fold 80"

echo "Operation tests..."
# Compute new mocs from operations
${moc} op degrade 0 SMOC_GLIMPSE_u32.fits fits SMOC_GLIMPSE_degrade_d0_u32.fits
${moc} op complement SMOC_GLIMPSE_u32.fits fits SMOC_GLIMPSE_not_u32.fits

${moc} op intborder SMOC_GLIMPSE_u32.fits fits SMOC_GLIMPSE_u32_intb.fits
${moc} op extborder SMOC_GLIMPSE_u32.fits fits SMOC_GLIMPSE_u32_extb.fits

${moc} op extborder SMOC_GLIMPSE_u64.fits fits SMOC_GLIMPSE_u64_extb.fits
${moc} op intborder SMOC_GLIMPSE_u64.fits fits SMOC_GLIMPSE_u64_intb.fits
${moc} op extend    SMOC_GLIMPSE_u64.fits fits SMOC_GLIMPSE_u64_ext.fits
${moc} op contract  SMOC_GLIMPSE_u64.fits fits SMOC_GLIMPSE_u64_con.fits
# Mix u32/u64 MOC operations
${moc} op union SMOC_GLIMPSE_u32_extb.fits SMOC_GLIMPSE_u64.fits fits SMOC_GLIMPSE_u32u64_ext.fits
test_eq "${moc} info SMOC_GLIMPSE_u64_ext.fits" "${moc} info SMOC_GLIMPSE_u32u64_ext.fits"

# Operation
test "${moc} op degrade 0 SMOC_GLIMPSE_u32.fits ascii" "0/0 3 5 7 9-10 "
test "${moc} op complement SMOC_GLIMPSE_degrade_d0_u32.fits ascii" "0/1-2 4 6 8 11 "
test "${moc} op diff  SMOC_GLIMPSE_u32.fits SMOC_GLIMPSE_u64.fits ascii" "9/ "
test "${moc} op minus SMOC_GLIMPSE_u32.fits SMOC_GLIMPSE_u64.fits ascii" "9/ "
test "${moc} op union SMOC_GLIMPSE_u32.fits SMOC_GLIMPSE_not_u32.fits ascii" "0/0-11 9/ "
test "${moc} op inter SMOC_GLIMPSE_u32.fits SMOC_GLIMPSE_not_u32.fits ascii" "9/ "
# Filter
test "${moc} filter position SMOC_GLIMPSE_u32.fits ${resources}/hip.psv --delimiter '|' --has-header --lon RAICRS --lat DEICRS | wc -l" "5222"
test "${moc} filter position SMOC_GLIMPSE_u64.fits ${resources}/hip.psv --delimiter '|' --has-header --lon RAICRS --lat DEICRS | wc -l" "5222"
test "${moc} filter position SMOC_GLIMPSE_u64.fits ${resources}/hip.psv --delimiter '|' --has-header --lon RAICRS --lat DEICRS --n-threads 4 --chunk-size 10000 | wc -l" "5222"
# From
test "egrep '^ *[0-9]' ${resources}/hip.psv | cut -d '|' -f 2-3 | tr -d ' ' | egrep '^[0-9]+' | ${moc} from pos 5 - --separator '|' ascii" "0/0-11 5/ "

# Clean
rm -f SMOC_GLIMPSE_*
echo "All tests passed."

exit 0



# From cone
moc from cone 14 0.0 +0.0 0.25 fits moc_cone_d14_l0_b0_r0.25.fits 
# From zone
moc from zone 14 359.5 -0.5 0.5 0.5 fits moc_zone_d14_359.5_-0.5_0.5_0.5.fits 
# From box
moc from box  14 0.0 +0.0 0.75 0.5  0.0 fits moc_box_d14_0.0+0.0_0.75_0.5_00.fits
moc from box  14 0.0 +0.0 0.75 0.5 45.0 fits moc_box_d14_0.0+0.0_0.75_0.5_45.fits
moc from box  14 0.0 +0.0 0.75 0.5 90.0 fits moc_box_d14_0.0+0.0_0.75_0.5_90.fits
# From ellipse
moc from ellipse 14 0.0 +0.0 0.75 0.5 45.0 fits moc_ellipse_d14_0.0+0.0_0.75_0.5_45.fits
moc from ellipse 14 0.0 +0.0 1.0 0.5 75.0 fits moc_ellipse_d14_0.0+0.0_1.0_0.5_75.fits
# From polygon
moc from polygon 14 (000.9160848095,+01.0736381331),(001.5961114529,-00.7062969568),(359.6079412529,+01.1296198985),\
(358.7836886856,-01.3663552354),(001.8720201899,+00.4097184220),(358.4159783831,+00.2376811155),\
(359.8319515193,-01.2824324848),(358.7798765255,+00.9896544935),(001.5440798843,+00.8056786162)\
  fits moc_polygon_d14_autointer.fits
# moc from polygon 14 "(000.9160848095,+01.0736381331),(001.5961114529,-00.7062969568),(359.6079412529,+01.1296198985),(358.7836886856,-01.3663552354),(001.8720201899,+00.4097184220),(358.4159783831,+00.2376811155),(359.8319515193,-01.2824324848),(358.7798765255,+00.9896544935),(001.5440798843,+00.8056786162)" fits moc_polygon_d14_autointersect.fits

# time moc from pos 7 kids_dr2.v3.csv -s , ascii --fold 80
# time moc filter position SMOC_GLIMPSE_u32.fits kids_dr2.csv --has-header --lon RAJ2000 --lat DECJ2000 --n-threads 4
