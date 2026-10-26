# Wind-tunnel measurements

Measured section characteristics used as a physics check of the viscous
solver (`crates/foil2d/tests/wind_tunnel.rs`). Unlike the XFOIL references,
they are not expected to match exactly: XFOIL's method (e<sup>n</sup>
transition, integral boundary layer) is a model of the real flow, and the
measurements carry their own uncertainties.

## Source

R. J. McGhee, B. S. Walker and B. F. Millard, *Experimental Results for
the Eppler 387 Airfoil at Low Reynolds Numbers in the Langley Low-Turbulence
Pressure Tunnel*, NASA TM-4062, October 1988
(<https://ntrs.nasa.gov/citations/19890001471>), a work of the United States
Government. Table B1 (lift and moment from surface pressures, drag from wake
surveys, standard low-speed tunnel corrections applied):

| File | Runs | Reynolds | Mach | Report page |
|---|---|---|---|---|
| `e387_ltpt_re1e5.csv` | 15, 16 | 100 000 | 0.03 | 93 |
| `e387_ltpt_re2e5.csv` | 9, 10, 13 | 200 000 | 0.06 | 92 |

The values were transcribed by hand from the scanned report (the text layer
of the PDF is unreliable OCR); drag marked `****` (not measured beyond
stall) is left empty. `e387.dat` is the UIUC design shape also used by the
XFOIL references; the tunnel model was measured slightly different (thicker
trailing edge, table I of the report).

## Agreement

Our solver (160 nodes, Ncrit 9, free transition, swept from 0° outward like
the application):

| Reynolds | Points below 9° | Worst Cl difference | Worst Cd ratio | Maximum lift (measured / ours) |
|---|---|---|---|---|
| 100 000 | 45 | 0.067 (over-predicted) | 1.22 | 1.21 / 1.33 |
| 200 000 | 24 | 0.059 (over-predicted) | 1.25 | 1.23 / 1.33 |

Up to about 7° the drag is within 10 % at Re = 200 000; the largest drag
differences are at the onset of stall. The maximum lift is over-predicted by
about 10 %, which is why the wing estimate of `aero-mission` takes 90 % of
the airfoil's maximum lift by default.

At 60 000 the report shows hysteresis (table B1, runs 31 and 32): with
decreasing angle the lift drops from 0.985 to 0.672 between 6.48° and 6.44°
when the laminar separation bubble bursts. Predictions at such Reynolds
numbers are less reliable, and the application says so.
