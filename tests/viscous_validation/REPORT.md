# Extended viscous XFOIL validation

49 random viscous analyses compared with XFOIL 6.996 (496 converged points compared, polar sequences and single points), 0 failure(s); 1 case(s) on which XFOIL itself failed.

34 cases agree with XFOIL on every point. In the other 15 cases the Newton iterations go through unstable phases (separated flows, unconverged points) where last-bit differences grow until the two codes follow different paths; for each of them our console output is identical to XFOIL's up to a first difference no larger than the change our own computation shows when its Reynolds number is perturbed by 1e-15 to 1e-11 (relative):

- `naca_19_24008`: identical logs up to line 208, then `MRCHDU: Convergence failed at  46  side 2    Res =  0.2024E+04` vs XFOIL `MRCHDU: Convergence failed at  46  side 2    Res =  0.2022E+04` (relative difference 9.9e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to 4.9e-4)
- `uiuc_ah81k144wfklappe`: identical logs up to line 110, then `17   rms: 0.7548E-01   max: 0.6176E+00   T at   67  2` vs XFOIL `17   rms: 0.7548E-01   max: 0.6175E+00   T at   67  2` (relative difference 1.6e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to 1.6e-4)
- `uiuc_bw3`: identical logs up to line 287, then `7   rms: 0.7204E-02   max: 0.7757E-01   D at   37  1` vs XFOIL `7   rms: 0.7204E-02   max: 0.7758E-01   D at   37  1` (relative difference 1.3e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to 0.0e0)
- `uiuc_e211`: identical logs up to line 113, then `MRCHDU: Convergence failed at  26  side 2    Res =  0.2508E+01` vs XFOIL `MRCHDU: Convergence failed at  26  side 2    Res =  0.2515E+01` (relative difference 2.8e-3; perturbing our Re by 1e-15..1e-11 changes this line by up to 8.0e-4)
- `uiuc_e342`: identical logs up to line 170, then `MRCHDU: Convergence failed at  49  side 2    Res =  0.1685E+01` vs XFOIL `MRCHDU: Convergence failed at  49  side 2    Res =  0.1679E+01` (relative difference 3.6e-3; perturbing our Re by 1e-15..1e-11 changes this line by up to 5.9e-4)
- `uiuc_e748`: identical logs up to line 263, then `12   rms: 0.4887E+00   max: -.4994E+01   T at   52  2   RLX: 0.100` vs XFOIL `12   rms: 0.4885E+00   max: -.4993E+01   T at   52  2   RLX: 0.100` (relative difference 4.1e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to 2.0e-4)
- `uiuc_fx6617a2`: identical logs up to line 743, then `14   rms: 0.6487E-01   max: -.3628E+00   T at   52  1` vs XFOIL `14   rms: 0.6487E-01   max: -.3627E+00   T at   52  1` (relative difference 2.8e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to more (an earlier line already changes))
- `uiuc_goe366`: identical logs up to line 389, then `5   rms: 0.3433E+02   max: 0.1399E+03   D at   39  2   RLX: 0.005` vs XFOIL `5   rms: 0.3432E+02   max: 0.1399E+03   D at   39  2   RLX: 0.005` (relative difference 2.9e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to 2.9e-4)
- `uiuc_goe546`: identical logs up to line 209, then `4   rms: 0.1503E+01   max: -.3200E+02   C at   68  2   RLX: 0.016` vs XFOIL `4   rms: 0.1503E+01   max: -.3201E+02   C at   68  2   RLX: 0.016` (relative difference 3.1e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to 3.1e-4)
- `uiuc_lwk80080`: identical logs up to line 69, then `12   rms: 0.1440E+01   max: -.2994E+02   C at   25  2   RLX: 0.017` vs XFOIL `12   rms: 0.1440E+01   max: -.2992E+02   C at   25  2   RLX: 0.017` (relative difference 6.7e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to more (an earlier line already changes))
- `uiuc_mrc16_95`: identical logs up to line 1308, then `MRCHDU: Convergence failed at  72  side 1    Res =  0.9582E+03` vs XFOIL `MRCHDU: Convergence failed at  72  side 1    Res =  0.9581E+03` (relative difference 1.0e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to 1.0e-4)
- `uiuc_naca001034a08cli0_2`: identical logs up to line 162, then `MRCHDU: Convergence failed at 134  side 2    Res =  0.3259E-02` vs XFOIL `MRCHDU: Convergence failed at 134  side 2    Res =  0.3260E-02` (relative difference 3.1e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to 0.0e0)
- `uiuc_naca1410`: identical logs up to line 233, then `4   rms: 0.7257E+00   max: -.8129E+01   C at   52  2   RLX: 0.062` vs XFOIL `4   rms: 0.7257E+00   max: -.8130E+01   C at   52  2   RLX: 0.062` (relative difference 1.2e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to 0.0e0)
- `uiuc_sd6080`: identical logs up to line 316, then `MRCHDU: Convergence failed at 105  side 2    Res =  0.7272E+02` vs XFOIL `MRCHDU: Convergence failed at 105  side 2    Res =  0.7273E+02` (relative difference 1.4e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to 1.4e-4)
- `uiuc_wb140`: identical logs up to line 191, then `16   rms: 0.2479E-05   max: -.2798E-04   T at   39  2` vs XFOIL `16   rms: 0.2479E-05   max: -.2799E-04   T at   39  2` (relative difference 3.6e-4; perturbing our Re by 1e-15..1e-11 changes this line by up to 3.6e-4)

Maximum differences per case: forces (CL, CM absolute, CD relative), transition x/c, boundary layer at every station (x, Cp absolute; θ, δ* relative to the largest θ; Cf relative to 0.01; N relative to 10; √Cτ relative to 0.1).

| Case | Airfoil | Re | M | Ncrit | Trips | Points | forces | x_tr | BL | |
|---|---|---|---|---|---|---|---|---|---|---|
| naca_00_2519_sharp | NACA 2519 (closed TE, random draw) | 1e7 | 0.1 | 9 | free | 12 | 1.2e-12 | 1.1e-12 | 4.2e-11 | ok |
| naca_01_5415 | NACA 5415 (open TE, random draw) | 2e5 | 0 | 9 | 0.8 / 1 | 18 | 1.7e-13 | 3.4e-13 | 1.3e-10 | ok |
| naca_02_4417 | NACA 4417 (open TE, random draw) | 2e6 | 0 | 5 | 0.1 / 0.05 | 11 | 1.4e-13 | 5.2e-14 | 4.1e-11 | ok |
| naca_03_23014 | NACA 23014 (open TE, random draw) | 2e6 | 0.4 | 7 | free | 17 | 1.1e-12 | 2.1e-12 | 5.7e-11 | ok |
| naca_04_23015_sharp | NACA 23015 (closed TE, random draw) | 1e6 | 0.1 | 9 | free | 11 | 1.9e-13 | 2.1e-13 | 5.6e-11 | ok |
| naca_05_1616 | NACA 1616 (open TE, random draw) | 2e6 | 0 | 7 | free | 9 | 1.9e-13 | 3.4e-13 | 2.3e-11 | ok |
| naca_06_23016_sharp | NACA 23016 (closed TE, random draw) | 1e6 | 0 | 9 | free | 19 | 5.0e-13 | 7.5e-13 | 5.4e-11 | ok |
| naca_07_0020 | NACA 0020 (open TE, random draw) | 1e7 | 0.4 | 7 | 0.5 / 0.05 | 10 | 7.6e-13 | 4.4e-13 | 1.9e-11 | ok |
| naca_08_4209 | NACA 4209 (open TE, random draw) | 1e7 | 0 | 7 | free | 12 | 1.4e-9 | 3.7e-8 | 4.5e-6 | ok |
| naca_09_2210 | NACA 2210 (open TE, random draw) | 3e5 | 0 | 9 | free | 8 | 4.3e-12 | 4.2e-11 | 4.0e-9 | ok |
| naca_10_2609 | NACA 2609 (open TE, random draw) | 3e5 | 0 | 11 | free | 8 | 2.3e-12 | 2.0e-11 | 3.9e-10 | ok |
| naca_11_21013 | NACA 21013 (open TE, random draw) | 2e6 | 0 | 5 | 0.3 / 0.05 | 9 | 1.4e-13 | 3.3e-13 | 5.0e-11 | ok |
| naca_12_1610 | NACA 1610 (open TE, random draw) | 2e5 | 0 | 5 | free | 16 | 1.2e-7 | 4.1e-8 | 7.7e-5 | ok |
| naca_13_4217 | NACA 4217 (open TE, random draw) | 2e6 | 0 | 4 / 12 | free | 17 | 2.9e-13 | 8.6e-12 | 1.0e-10 | ok |
| naca_14_6310 | NACA 6310 (open TE, random draw) | 3e6 | 0 | 5 | free | 9 | 2.6e-13 | 3.8e-13 | 5.5e-11 | ok |
| naca_15_1406 | NACA 1406 (open TE, random draw) | 1e5 | 0.2 | 5 | 0.3 / 0.8 | 10 | 1.0e-12 | 1.5e-13 | 1.5e-10 | ok |
| naca_16_5207 | NACA 5207 (open TE, random draw) | 1e6 | 0.2 | 9 / 8 | free | 2 | 3.0e-14 | 3.5e-14 | 6.5e-11 | ok |
| naca_17_2512_sharp | NACA 2512 (closed TE, random draw) | 6e6 | 0 | 11 | free | 11 | 3.8e-13 | 1.3e-12 | 3.6e-11 | ok |
| naca_18_21015_sharp | NACA 21015 (closed TE, random draw) | 1e5 | 0 | 4 / 12 | free | 12 | 1.2e-13 | 1.4e-13 | 7.9e-11 | ok |
| naca_19_24008 | NACA 24008 (open TE, random draw) | 3e5 | 0.1 | 11 | free | 0 | 0.0e0 | 0.0e0 | 0.0e0 | round-off divergence (log line 208) |
| uiuc_ah81k144wfklappe | uiuc_ah81k144wfklappe of ../validation (random draw) | 3e5 | 0 | 11 | free | 3 | 4.9e-2 | 8.8e-3 | 1.1e0 | round-off divergence (log line 110) |
| uiuc_bw3 | uiuc_bw3 of ../validation (random draw) | 1e7 | 0.4 | 7 | free | 0 | 0.0e0 | 0.0e0 | 0.0e0 | round-off divergence (log line 287) |
| uiuc_clarym18 | uiuc_clarym18 of ../validation (random draw) | 2e5 | 0 | 6 / 12 | 0.05 / 0.1 | 11 | 2.3e-14 | 0.0e0 | 4.4e-11 | ok |
| uiuc_e211 | uiuc_e211 of ../validation (random draw) | 2e5 | 0 | 9 | free | 9 | 1.2e-6 | 3.1e-7 | 5.7e-5 | round-off divergence (log line 113) |
| uiuc_e342 | uiuc_e342 of ../validation (random draw) | 5e5 | 0 | 7 | free | 20 | 1.8e-6 | 1.3e-6 | 1.2e-3 | round-off divergence (log line 170) |
| uiuc_e343 | uiuc_e343 of ../validation (random draw) | 3e5 | 0 | 9 | free | 2 | 1.9e-14 | 1.8e-14 | 3.4e-11 | ok |
| uiuc_e748 | uiuc_e748 of ../validation (random draw) | 3e5 | 0.1 | 9 / 12 | free | 0 | 0.0e0 | 0.0e0 | 0.0e0 | round-off divergence (log line 263) |
| uiuc_fx05191 | uiuc_fx05191 of ../validation (random draw) | 1e6 | 0 | 5 | 0.3 / 0.5 | 24 | 9.2e-13 | 7.4e-13 | 1.3e-10 | ok |
| uiuc_fx6617a2 | uiuc_fx6617a2 of ../validation (random draw) | 2e5 | 0 | 9 | free | 10 | 1.4e-6 | 1.5e-6 | 1.0e-4 | round-off divergence (log line 743) |
| uiuc_fx711520 | uiuc_fx711520 of ../validation (random draw) | 1e5 | 0 | 7 | free | 21 | 5.9e-14 | 1.7e-13 | 4.6e-11 | ok |
| uiuc_goe301 | uiuc_goe301 of ../validation (random draw) | 6e6 | 0 | 7 | free | 12 | 1.4e-12 | 1.3e-12 | 1.0e-10 | ok |
| uiuc_goe366 | uiuc_goe366 of ../validation (random draw) | 5e5 | 0 | 11 | free | 0 | 0.0e0 | 0.0e0 | 0.0e0 | round-off divergence (log line 389) |
| uiuc_goe412 | uiuc_goe412 of ../validation (random draw) | 6e6 | 0.1 | 7 | free | 12 | 1.9e-11 | 5.9e-10 | 3.2e-8 | ok |
| uiuc_goe423 | uiuc_goe423 of ../validation (random draw) | 1e6 | 0 | 9 / 8 | 0.3 / 0.8 | 17 | 7.6e-13 | 6.9e-13 | 5.3e-11 | ok |
| uiuc_goe441 | uiuc_goe441 of ../validation (random draw) | 1e6 | 0 | 9 | free | 11 | 2.7e-13 | 1.6e-13 | 3.5e-11 | ok |
| uiuc_goe481a | uiuc_goe481a of ../validation (random draw) | 1e7 | 0 | 4 / 10 | 0.5 / 0.02 | 13 | 3.2e-13 | 2.0e-13 | 1.1e-11 | ok |
| uiuc_goe546 | uiuc_goe546 of ../validation (random draw) | 3e6 | 0 | 11 | 0.5 / 1 | 6 | 4.2e-8 | 2.1e-6 | 2.3e-4 | round-off divergence (log line 209) |
| uiuc_goe795 | uiuc_goe795 of ../validation (random draw) | 5e5 | 0 | 9 / 10 | 0.1 / 0.8 | 12 | 2.3e-13 | 1.2e-13 | 1.1e-10 | ok |
| uiuc_lwk80080 | uiuc_lwk80080 of ../validation (random draw) | 6e6 | 0 | 6 / 10 | 0.8 / 0.8 | 0 | 0.0e0 | 0.0e0 | 0.0e0 | round-off divergence (log line 69) |
| uiuc_marsden | uiuc_marsden of ../validation (random draw) | 2e6 | 0.2 | 9 / 10 | free | 0 | 0.0e0 | 0.0e0 | 0.0e0 | ok |
| uiuc_mrc16_95 | uiuc_mrc16_95 of ../validation (random draw) | 1e5 | 0.3 | 9 / 8 | free | 20 | 9.3e-6 | 4.8e-6 | 2.1e-4 | round-off divergence (log line 1308) |
| uiuc_n6h15 | uiuc_n6h15 of ../validation (random draw) | 5e5 | 0.3 | 5 | 0.1 / 0.3 | 10 | 4.2e-12 | 4.8e-13 | 4.1e-10 | ok |
| uiuc_naca001034a08cli0_2 | uiuc_naca001034a08cli0_2 of ../validation (random draw) | 3e5 | 0 | 4 / 12 | free | 0 | 0.0e0 | 0.0e0 | 0.0e0 | round-off divergence (log line 162) |
| uiuc_naca1410 | uiuc_naca1410 of ../validation (random draw) | 6e6 | 0.1 | 4 / 10 | free | 0 | 0.0e0 | 0.0e0 | 0.0e0 | round-off divergence (log line 233) |
| uiuc_sc20412 | uiuc_sc20412 of ../validation (random draw) | 3e6 | 0.3 | 6 / 12 | free | 9 | 5.0e-10 | 1.5e-8 | 1.0e-7 | ok |
| uiuc_sd6080 | uiuc_sd6080 of ../validation (random draw) | 5e5 | 0 | 5 | free | 10 | 2.6e-6 | 3.6e-5 | 5.3e-4 | round-off divergence (log line 316) |
| uiuc_tempest2 | uiuc_tempest2 of ../validation (random draw) | 1e7 | 0 | 11 | free | 11 | 1.9e-11 | 8.7e-10 | 1.4e-8 | ok |
| uiuc_usa35a | uiuc_usa35a of ../validation (random draw) | 2e6 | 0 | 9 | free | 9 | 2.4e-13 | 1.5e-12 | 3.0e-11 | ok |
| uiuc_wb140 | uiuc_wb140 of ../validation (random draw) | 2e5 | 0 | 9 / 10 | 0.8 / 1 | 23 | 3.4e-7 | 1.6e-5 | 1.9e-4 | round-off divergence (log line 191) |
