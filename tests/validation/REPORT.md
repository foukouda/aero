# Extended XFOIL validation

100 cases compared with XFOIL 6.996 (1292 polar points plus Cp distributions), 0 failure(s); 0 case(s) on which XFOIL itself failed; 7 point(s) where the Kármán–Tsien correction breaks down (refused by aero, flagged as invalid by XFOIL).

| Case | Airfoil | Nodes | max Δnode | max ΔCL | max ΔCM | max ΔCDp | max ΔCp | |
|---|---|---|---|---|---|---|---|---|
| naca_00_4406 | NACA 4406 (open TE, random draw) | 200 | 4.9e-8 | 5.0e-5 | 4.4e-5 | 4.7e-6 | 5.0e-6 | ok |
| naca_01_25017 | NACA 25017 (open TE, random draw) | 200 | 6.6e-8 | 4.4e-5 | 4.9e-5 | 4.6e-6 | 5.0e-6 | ok |
| naca_02_21008 | NACA 21008 (open TE, random draw) | 280 | 5.0e-8 | 4.8e-5 | 3.9e-5 | 4.4e-6 | 5.0e-6 | ok |
| naca_03_5417 | NACA 5417 (open TE, random draw) | 280 | 6.7e-8 | 4.7e-5 | 4.8e-5 | 4.9e-6 | 5.0e-6 | ok |
| naca_04_2410 | NACA 2410 (open TE, random draw) | 60 | 5.0e-8 | 3.8e-5 | 4.8e-5 | 4.6e-6 | 5.0e-6 | ok |
| naca_05_23013_sharp | NACA 23013 (closed TE, random draw) | 60 | 4.7e-8 | 4.7e-5 | 5.0e-5 | 4.6e-6 | 4.9e-6 | ok |
| naca_06_25011 | NACA 25011 (open TE, random draw) | 140 | 4.9e-8 | 4.3e-5 | 4.5e-5 | 4.9e-6 | 5.0e-6 | ok |
| naca_07_2519_sharp | NACA 2519 (closed TE, random draw) | 60 | 6.7e-8 | 4.6e-5 | 5.0e-5 | 4.2e-6 | 4.9e-6 | ok |
| naca_08_5407_sharp | NACA 5407 (closed TE, random draw) | 360 | 5.0e-8 | 5.0e-5 | 4.1e-5 | 4.2e-6 | 5.0e-6 | ok |
| naca_09_24016 | NACA 24016 (open TE, random draw) | 280 | 5.0e-8 | 4.5e-5 | 4.8e-5 | 3.9e-6 | 5.0e-6 | ok |
| naca_10_4622 | NACA 4622 (open TE, random draw) | 140 | 6.2e-8 | 4.7e-5 | 4.9e-5 | 4.9e-6 | 5.0e-6 | ok |
| naca_11_4606_sharp | NACA 4606 (closed TE, random draw) | 200 | 4.9e-8 | 4.7e-5 | 4.8e-5 | 5.0e-6 | 5.0e-6 | ok |
| naca_12_2520_sharp | NACA 2520 (closed TE, random draw) | 100 | 6.4e-8 | 4.5e-5 | 4.9e-5 | 4.8e-6 | 5.0e-6 | ok |
| naca_13_23012 | NACA 23012 (open TE, random draw) | 100 | 4.8e-8 | 4.8e-5 | 4.8e-5 | 5.0e-6 | 5.0e-6 | ok |
| naca_14_25013 | NACA 25013 (open TE, random draw) | 60 | 5.0e-8 | 4.2e-5 | 4.9e-5 | 4.5e-6 | 5.0e-6 | ok |
| naca_15_5524_sharp | NACA 5524 (closed TE, random draw) | 140 | 6.8e-8 | 4.8e-5 | 4.4e-5 | 4.4e-6 | 5.0e-6 | ok |
| naca_16_21012 | NACA 21012 (open TE, random draw) | 100 | 5.0e-8 | 4.6e-5 | 4.6e-5 | 4.6e-6 | 5.0e-6 | ok |
| naca_17_6607 | NACA 6607 (open TE, random draw) | 140 | 5.0e-8 | 4.9e-5 | 4.6e-5 | 4.6e-6 | 5.0e-6 | ok |
| naca_18_25019 | NACA 25019 (open TE, random draw) | 280 | 5.7e-8 | 4.1e-5 | 4.9e-5 | 4.2e-6 | 5.0e-6 | ok |
| naca_19_25013_sharp | NACA 25013 (closed TE, random draw) | 200 | 5.0e-8 | 4.3e-5 | 4.5e-5 | 3.7e-6 | 5.0e-6 | ok |
| naca_20_25021 | NACA 25021 (open TE, random draw) | 280 | 6.1e-8 | 4.9e-5 | 4.7e-5 | 3.2e-6 | 5.0e-6 | ok |
| naca_21_3621_sharp | NACA 3621 (closed TE, random draw) | 200 | 5.7e-8 | 4.5e-5 | 5.0e-5 | 3.5e-6 | 5.0e-6 | ok |
| naca_22_22010_sharp | NACA 22010 (closed TE, random draw) | 200 | 5.0e-8 | 4.4e-5 | 5.0e-5 | 4.3e-6 | 5.0e-6 | ok |
| naca_23_0024_sharp | NACA 0024 (closed TE, random draw) | 140 | 6.4e-8 | 4.3e-5 | 4.7e-5 | 4.3e-6 | 5.0e-6 | ok |
| naca_24_0009 | NACA 0009 (open TE, random draw) | 280 | 5.0e-8 | 4.8e-5 | 4.9e-5 | 4.8e-6 | 5.0e-6 | ok |
| naca_25_0019_sharp | NACA 0019 (closed TE, random draw) | 280 | 5.0e-8 | 4.5e-5 | 5.0e-5 | 4.9e-6 | 5.0e-6 | ok |
| naca_26_25010 | NACA 25010 (open TE, random draw) | 140 | 5.0e-8 | 4.9e-5 | 5.0e-5 | 4.6e-6 | 5.0e-6 | ok |
| naca_27_1223_sharp | NACA 1223 (closed TE, random draw) | 60 | 6.1e-8 | 4.6e-5 | 4.8e-5 | 5.0e-6 | 5.0e-6 | ok |
| naca_28_6521_sharp | NACA 6521 (closed TE, random draw) | 280 | 6.7e-8 | 4.9e-5 | 5.0e-5 | 4.7e-6 | 5.0e-6 | ok |
| naca_29_5519 | NACA 5519 (open TE, random draw) | 60 | 6.2e-8 | 5.0e-5 | 4.9e-5 | 4.8e-6 | 4.9e-6 | ok |
| naca_30_0007 | NACA 0007 (open TE, random draw) | 100 | 4.9e-8 | 4.7e-5 | 4.9e-5 | 4.5e-6 | 5.0e-6 | ok |
| naca_31_5418 | NACA 5418 (open TE, random draw) | 100 | 5.7e-8 | 4.8e-5 | 4.9e-5 | 4.9e-6 | 5.0e-6 | ok |
| naca_32_23012 | NACA 23012 (open TE, random draw) | 200 | 5.0e-8 | 5.0e-5 | 4.4e-5 | 5.0e-6 | 5.0e-6 | ok |
| naca_33_2215 | NACA 2215 (open TE, random draw) | 280 | 5.0e-8 | 4.7e-5 | 4.9e-5 | 4.3e-6 | 5.0e-6 | ok |
| naca_34_24011_sharp | NACA 24011 (closed TE, random draw) | 200 | 5.0e-8 | 4.6e-5 | 4.9e-5 | 4.8e-6 | 5.0e-6 | ok |
| naca_35_2322 | NACA 2322 (open TE, random draw) | 360 | 6.6e-8 | 5.0e-5 | 4.9e-5 | 4.9e-6 | 5.0e-6 | ok |
| naca_36_21017 | NACA 21017 (open TE, random draw) | 60 | 4.9e-8 | 4.5e-5 | 4.8e-5 | 4.6e-6 | 5.0e-6 | ok |
| naca_37_23021_sharp | NACA 23021 (closed TE, random draw) | 100 | 6.5e-8 | 4.9e-5 | 4.9e-5 | 4.9e-6 | 5.0e-6 | ok |
| naca_38_23019 | NACA 23019 (open TE, random draw) | 60 | 5.8e-8 | 4.7e-5 | 4.9e-5 | 4.8e-6 | 4.9e-6 | ok |
| naca_39_0023_sharp | NACA 0023 (closed TE, random draw) | 360 | 7.0e-8 | 5.0e-5 | 4.4e-5 | 4.6e-6 | 5.0e-6 | ok |
| uiuc_ah21_7 | UIUC ah21-7 (random draw) | 160 | 5.0e-8 | 4.6e-5 | 4.9e-5 | 5.0e-6 | 5.0e-6 | ok |
| uiuc_ah81k144wfklappe | UIUC ah81k144wfKlappe (random draw) | 100 | 5.3e-8 | 4.5e-5 | 4.7e-5 | 4.4e-6 | 5.0e-6 | ok |
| uiuc_bw3 | UIUC bw3 (random draw) | 60 | 4.9e-8 | 4.8e-5 | 5.0e-5 | 4.8e-6 | 5.0e-6 | ok |
| uiuc_c5a | UIUC c5a (random draw) | 100 | 4.9e-8 | 4.6e-5 | 4.0e-5 | 4.4e-6 | 5.0e-6 | ok |
| uiuc_c5d | UIUC c5d (random draw) | 360 | 5.0e-8 | 4.6e-5 | 4.6e-5 | 4.7e-6 | 5.0e-6 | ok |
| uiuc_clarym15 | UIUC clarym15 (random draw) | 200 | 6.8e-8 | 4.4e-5 | 4.2e-5 | 4.8e-6 | 5.0e-6 | ok |
| uiuc_clarym18 | UIUC clarym18 (random draw) | 280 | 6.4e-8 | 4.0e-5 | 4.9e-5 | 4.5e-6 | 5.0e-6 | ok |
| uiuc_davissm | UIUC davissm (random draw) | 160 | 4.9e-8 | 3.7e-5 | 4.7e-5 | 4.3e-6 | 5.0e-6 | ok |
| uiuc_defcnd3 | UIUC defcnd3 (random draw) | 100 | 5.0e-8 | 5.0e-5 | 4.3e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_e211 | UIUC e211 (random draw) | 160 | 5.0e-8 | 4.2e-5 | 4.9e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_e342 | UIUC e342 (random draw) | 160 | 5.7e-8 | 4.8e-5 | 4.9e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_e343 | UIUC e343 (random draw) | 160 | 6.7e-8 | 5.0e-5 | 4.2e-5 | 5.0e-6 | 5.0e-6 | ok |
| uiuc_e387 | UIUC e387 (random draw) | 160 | 5.0e-8 | 4.9e-5 | 5.0e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_e550 | UIUC e550 (random draw) | 160 | 6.0e-8 | 5.0e-5 | 4.9e-5 | 4.0e-6 | 5.0e-6 | ok |
| uiuc_e635 | UIUC e635 (random draw) | 160 | 5.0e-8 | 5.0e-5 | 5.0e-5 | 4.2e-6 | 5.0e-6 | ok |
| uiuc_e748 | UIUC e748 (random draw) | 160 | 5.2e-8 | 4.9e-5 | 4.5e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_fx05191 | UIUC fx05191 (random draw) | 280 | 6.7e-8 | 3.5e-5 | 4.9e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_fx60126 | UIUC fx60126 (random draw) | 160 | 5.0e-8 | 4.8e-5 | 4.7e-5 | 5.0e-6 | 5.0e-6 | ok |
| uiuc_fx6617a2 | UIUC fx6617a2 (random draw) | 140 | 6.5e-8 | 4.6e-5 | 4.6e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_fx711520 | UIUC fx711520 (random draw) | 160 | 5.0e-8 | 4.8e-5 | 3.2e-5 | 4.4e-6 | 5.0e-6 | ok |
| uiuc_goe08k | UIUC goe08k (random draw) | 160 | 6.3e-8 | 4.5e-5 | 4.8e-5 | 3.3e-6 | 5.0e-6 | ok |
| uiuc_goe113 | UIUC goe113 (random draw) | 100 | 4.9e-8 | 4.3e-5 | 4.3e-5 | 5.0e-6 | 5.0e-6 | ok |
| uiuc_goe182 | UIUC goe182 (random draw) | 160 | 4.9e-8 | 4.1e-5 | 4.8e-5 | 4.6e-6 | 5.0e-6 | ok |
| uiuc_goe223 | UIUC goe223 (random draw) | 100 | 6.6e-8 | 4.9e-5 | 5.0e-5 | 4.8e-6 | 5.0e-6 | ok |
| uiuc_goe301 | UIUC goe301 (random draw) | 100 | 6.8e-8 | 4.8e-5 | 4.9e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_goe303 | UIUC goe303 (random draw) | 160 | 6.0e-8 | 4.7e-5 | 4.3e-5 | 4.5e-6 | 4.9e-6 | ok |
| uiuc_goe366 | UIUC goe366 (random draw) | 160 | 6.7e-8 | 4.6e-5 | 4.8e-5 | 4.6e-6 | 5.0e-6 | ok |
| uiuc_goe412 | UIUC goe412 (random draw) | 160 | 6.6e-8 | 4.9e-5 | 4.9e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_goe423 | UIUC goe423 (random draw) | 160 | 5.8e-8 | 4.7e-5 | 4.7e-5 | 3.5e-6 | 5.0e-6 | ok |
| uiuc_goe441 | UIUC goe441 (random draw) | 360 | 6.6e-8 | 4.9e-5 | 5.0e-5 | 4.0e-6 | 5.0e-6 | ok |
| uiuc_goe481a | UIUC goe481a (random draw) | 160 | 6.5e-8 | 4.8e-5 | 5.0e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_goe523 | UIUC goe523 (random draw) | 160 | 6.2e-8 | 4.3e-5 | 4.5e-5 | 4.2e-6 | 5.0e-6 | ok |
| uiuc_goe546 | UIUC goe546 (random draw) | 140 | 5.0e-8 | 4.5e-5 | 4.7e-5 | 4.6e-6 | 5.0e-6 | ok |
| uiuc_goe547 | UIUC goe547 (random draw) | 160 | 5.0e-8 | 5.0e-5 | 4.8e-5 | 4.3e-6 | 5.0e-6 | ok |
| uiuc_goe572 | UIUC goe572 (random draw) | 200 | 6.2e-8 | 3.9e-5 | 4.9e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_goe655 | UIUC goe655 (random draw) | 160 | 5.3e-8 | 4.3e-5 | 4.7e-5 | 4.4e-6 | 5.0e-6 | ok |
| uiuc_goe795 | UIUC goe795 (random draw) | 160 | 5.0e-8 | 4.4e-5 | 4.9e-5 | 4.6e-6 | 5.0e-6 | ok |
| uiuc_ht33 | UIUC ht33 (random draw) | 100 | 5.0e-8 | 4.9e-5 | 4.2e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_ls413 | UIUC ls413 (random draw) | 160 | 5.0e-8 | 4.6e-5 | 3.6e-5 | 4.7e-6 | 5.0e-6 | ok |
| uiuc_ls421mod | UIUC ls421mod (random draw) | 160 | 5.9e-8 | 4.4e-5 | 4.8e-5 | 4.8e-6 | 5.0e-6 | ok |
| uiuc_lwk80080 | UIUC lwk80080 (random draw) | 160 | 4.8e-8 | 3.0e-5 | 4.1e-5 | 4.5e-6 | 5.0e-6 | ok |
| uiuc_m15 | UIUC m15 (random draw) | 160 | 5.0e-8 | 4.5e-5 | 4.4e-5 | 4.6e-6 | 5.0e-6 | ok |
| uiuc_marsden | UIUC marsden (random draw) | 160 | 6.8e-8 | 4.7e-5 | 4.9e-5 | 4.8e-6 | 5.0e-6 | ok |
| uiuc_marske1 | UIUC marske1 (random draw) | 200 | 5.6e-8 | 5.0e-5 | 4.5e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_mh83 | UIUC mh83 (random draw) | 100 | 5.9e-8 | 4.6e-5 | 4.6e-5 | 4.8e-6 | 5.0e-6 | ok |
| uiuc_mh92 | UIUC mh92 (random draw) | 160 | 5.0e-8 | 4.9e-5 | 4.1e-5 | 3.5e-6 | 5.0e-6 | ok |
| uiuc_mrc16_95 | UIUC mrc16-95 (random draw) | 160 | 5.0e-8 | 4.7e-5 | 4.8e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_n14 | UIUC n14 (random draw) | 200 | 5.0e-8 | 3.9e-5 | 4.6e-5 | 4.6e-6 | 5.0e-6 | ok |
| uiuc_n6h15 | UIUC n6h15 (random draw) | 160 | 6.2e-8 | 4.9e-5 | 4.6e-5 | 4.5e-6 | 5.0e-6 | ok |
| uiuc_naca001034a08cli0_2 | UIUC naca001034a08cli0.2 (random draw) | 160 | 5.0e-8 | 4.8e-5 | 5.0e-5 | 4.8e-6 | 5.0e-6 | ok |
| uiuc_naca1410 | UIUC naca1410 (random draw) | 160 | 5.0e-8 | 4.3e-5 | 4.8e-5 | 3.7e-6 | 5.0e-6 | ok |
| uiuc_naca63206 | UIUC naca63206 (random draw) | 160 | 5.0e-8 | 4.6e-5 | 3.8e-5 | 5.0e-6 | 5.0e-6 | ok |
| uiuc_naca64208 | UIUC naca64208 (random draw) | 160 | 5.0e-8 | 4.9e-5 | 4.7e-5 | 4.9e-6 | 5.0e-6 | ok |
| uiuc_s3 | UIUC s3 (random draw) | 160 | 5.0e-8 | 4.8e-5 | 4.7e-5 | 5.0e-6 | 5.0e-6 | ok |
| uiuc_s8052 | UIUC s8052 (random draw) | 160 | 5.0e-8 | 4.3e-5 | 4.8e-5 | 5.0e-6 | 5.0e-6 | ok |
| uiuc_sc20412 | UIUC sc20412 (random draw) | 200 | 5.0e-8 | 4.9e-5 | 4.3e-5 | 4.8e-6 | 5.0e-6 | ok |
| uiuc_sd6080 | UIUC sd6080 (random draw) | 160 | 5.0e-8 | 5.0e-5 | 4.9e-5 | 4.7e-6 | 5.0e-6 | ok |
| uiuc_tempest2 | UIUC tempest2 (random draw) | 160 | 5.0e-8 | 4.8e-5 | 4.5e-5 | 5.0e-6 | 5.0e-6 | ok |
| uiuc_usa35a | UIUC usa35a (random draw) | 160 | 5.3e-8 | 4.8e-5 | 4.9e-5 | 4.6e-6 | 5.0e-6 | ok |
| uiuc_wb140 | UIUC wb140 (random draw) | 160 | 6.0e-8 | 3.8e-5 | 4.7e-5 | 3.9e-6 | 5.0e-6 | ok |
