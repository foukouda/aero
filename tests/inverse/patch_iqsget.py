#!/usr/bin/env python3
"""Instruments XFOIL's IQSGET (xqdes.f) for the reference runs.

XFOIL marks the target segment of its inverse design menus (QDES, MDES
"MARK") with the mouse only. This patch makes IQSGET read the two endpoint
node indices from the keyboard instead, so that the sessions can be
scripted; nothing else is changed. Usage: patch_iqsget.py path/to/xqdes.f
"""

import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text()
start = text.index("      SUBROUTINE IQSGET")
# Statement functions precede executable statements: insert after IQSGET's.
anchor = text.index("QCOMP(G) =", start)
line_end = text.index("\n", anchor) + 1
patch = (
    "C---- aero reference build: endpoint node indices from the keyboard\n"
    "      CALL ASKI('Enter target segment first node index^',IQ1)\n"
    "      CALL ASKI('Enter target segment last node index^',IQ2)\n"
    "      LIQSET = .TRUE.\n"
    "      RETURN\n"
)
if "aero reference build" in text:
    sys.exit(0)
path.write_text(text[:line_end] + patch + text[line_end:])
print(f"patched IQSGET in {path}")
