# SPDX-License-Identifier: GPL-3.0-or-later
# Reference generator: drives the *upstream* Python Rev C encoders (no serial port needed)
# and writes the same byte streams as lcd-core/examples/dump_frames.rs.
import os
import sys
from math import ceil

upstream = sys.argv[1]
out = sys.argv[2]
sys.path.insert(0, upstream)

import numpy as np
from PIL import Image

from library.lcd.lcd_comm import Orientation
from library.lcd import lcd_comm_rev_c as rc
from library.lcd.lcd_comm_rev_c import LcdCommRevC, Command, Padding, SubRevision

os.makedirs(out, exist_ok=True)


def test_image(w, h):
    a = np.zeros((h, w, 4), dtype=np.uint8)
    for y in range(h):
        for x in range(w):
            a[y, x] = ((x * 7 + y * 3) % 256, (x * 5 + y * 11) % 256, (x + y * 2) % 256, (x ^ y) % 256)
    return Image.fromarray(a, "RGBA")


def pad(cmd_bytes, payload=None, padding=Padding.NULL):
    # identical to LcdCommRevC._send_command padding logic
    message = bytearray(cmd_bytes)
    if payload:
        message.extend(payload)
    n = len(message)
    if not (n / 250).is_integer():
        message += bytearray(padding.value * (250 * ceil(n / 250) - n))
    return bytes(message)


def make_lcd(orientation):
    lcd = object.__new__(LcdCommRevC)
    lcd.display_width = 480
    lcd.display_height = 480
    lcd.orientation = orientation
    lcd.sub_revision = SubRevision.REV_2INCH
    lcd.rom_version = 87
    lcd.get_width = lambda: 480
    lcd.get_height = lambda: 480
    return lcd


full = test_image(480, 480)
part = test_image(100, 60)
names = {
    "portrait": Orientation.PORTRAIT,
    "landscape": Orientation.LANDSCAPE,
    "rportrait": Orientation.REVERSE_PORTRAIT,
    "rlandscape": Orientation.REVERSE_LANDSCAPE,
}

for name, o in names.items():
    lcd = make_lcd(o)

    b = b""
    b += pad(Command.PRE_UPDATE_BITMAP.value)
    b += pad(Command.START_DISPLAY_BITMAP.value, padding=Padding.START_DISPLAY_BITMAP)
    b += pad(Command.DISPLAY_BITMAP_2INCH.value, bytearray(int(480 * 480 / 64).to_bytes(2, "big")))
    b += pad(b"", bytearray(lcd._generate_full_image(full)))
    b += pad(Command.QUERY_STATUS.value)
    open(os.path.join(out, f"full_{name}.bin"), "wb").write(b)

    img, pyd = lcd._generate_update_image(part, 30, 50, 5, Command.UPDATE_BITMAP)
    b = pad(b"", pyd) + pad(b"", img) + pad(Command.QUERY_STATUS.value)
    open(os.path.join(out, f"update_{name}.bin"), "wb").write(b)

b = b""
for lvl in [0, 1, 25, 50, 99, 100]:
    b += pad(Command.SET_BRIGHTNESS.value, bytearray((int((lvl / 100) * 255),)))
b += pad(Command.OPTIONS.value,
         Command.STARTMODE_DEFAULT.value + Padding.NULL.value + Command.NO_FLIP.value + rc.SleepInterval.OFF.value)
open(os.path.join(out, "misc.bin"), "wb").write(b)
print("python dump done")
