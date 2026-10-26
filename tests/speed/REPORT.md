# Speed: Aero compared with XFOIL 6.996

Same machine: AMD EPYC 9V74 80-Core Processor, 4 logical cores, Linux 6.17.0-1022-azure. Median of 5 runs. NACA 2412 paneled with 160 nodes, Re = 10⁶, Ncrit 9; the batch uses 8 NACA sections (0012, 2412, 4412, 4415, 23012, 6409, 0009, 2415) and 4 worker(s).

XFOIL times are whole sessions minus an empty session (start-up, NACA, PANE: 7.6 ms for official (-O, double), 7.6 ms for -O3 (double), 7.6 ms for -O2 (single)), except the batch rows, which count whole sessions (start-up included), as running XFOIL on many airfoils does. Aero times start from the paneled airfoil; the batch rows include the geometry and the paneling.

| Task | XFOIL official (-O, double) | XFOIL -O3 (double) | XFOIL -O2 (single) | Aero | Aero faster by |
|---|---|---|---|---|---|
| Inviscid polar, 37 angles (−4° to 14°) | 8.1 ms | 8.1 ms | 8.1 ms | 2.7 ms | × 2.9 |
| Viscous point from scratch (α = 4°) | 56.5 ms | 24.2 ms | 56.5 ms | 22.3 ms | × 1.1 |
| Viscous polar, 37 angles (−4° to 14°) | 658.2 ms | 507.7 ms | 758.5 ms | 486.9 ms | × 1.0 |
| Viscous polar at prescribed Cl, 13 values (0 to 1.2) | 357.3 ms | 307.1 ms | 407.6 ms | 265.7 ms | × 1.2 |
| 8 airfoils × viscous polar, one core | 5.03 s | 4.17 s | 6.38 s | 3.79 s | × 1.1 |
| 8 airfoils × viscous polar, all cores | 2.14 s | 1.74 s | 2.84 s | 1.60 s | × 1.1 |

“Aero faster by” compares with the fastest XFOIL build of the row.

Points that did not converge (same work on both sides):

| Task | XFOIL official (-O, double) | XFOIL -O3 (double) | XFOIL -O2 (single) | Aero |
|---|---|---|---|---|
| Inviscid polar, 37 angles (−4° to 14°) | 0 | 0 | 0 | 0 |
| Viscous point from scratch (α = 4°) | 0 | 0 | 0 | 0 |
| Viscous polar, 37 angles (−4° to 14°) | 2 | 2 | 1 | 2 |
| Viscous polar at prescribed Cl, 13 values (0 to 1.2) | 1 | 1 | 1 | 1 |
