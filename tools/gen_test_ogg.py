#!/usr/bin/env python3
"""Erzeugt kurze, selbst generierte Ogg/Vorbis-Töne für Testfixtures.

Nutzt die System-libsndfile per ctypes (kein pip nötig). Die Töne sind reine
Sinus-Signale und frei von Rechten Dritter (CC0).

Aufruf: python3 -I tools/gen_test_ogg.py <ausgabe.ogg|.wav|.mp3|.flac> <frequenz_hz> [dauer_ms]
"""
import ctypes
import ctypes.util
import math
import sys

SFM_WRITE = 0x20
SF_FORMAT_OGG = 0x200000
SF_FORMAT_VORBIS = 0x0060
SF_FORMAT_WAV = 0x010000
SF_FORMAT_PCM_16 = 0x0002


class SFInfo(ctypes.Structure):
    _fields_ = [
        ("frames", ctypes.c_int64),
        ("samplerate", ctypes.c_int),
        ("channels", ctypes.c_int),
        ("format", ctypes.c_int),
        ("sections", ctypes.c_int),
        ("seekable", ctypes.c_int),
    ]


def main() -> int:
    out, freq = sys.argv[1], float(sys.argv[2])
    ms = int(sys.argv[3]) if len(sys.argv) > 3 else 200
    lib = ctypes.CDLL(ctypes.util.find_library("sndfile") or "libsndfile.so.1")
    lib.sf_open.restype = ctypes.c_void_p
    lib.sf_open.argtypes = [ctypes.c_char_p, ctypes.c_int, ctypes.POINTER(SFInfo)]
    lib.sf_write_short.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_short), ctypes.c_int64]
    lib.sf_close.argtypes = [ctypes.c_void_p]
    rate = 22050
    low = out.lower()
    if low.endswith(".wav"):
        fmt = SF_FORMAT_WAV | SF_FORMAT_PCM_16
    elif low.endswith(".mp3"):
        fmt = 0x230000 | 0x0082  # SF_FORMAT_MPEG | SF_FORMAT_MPEG_LAYER_III
    elif low.endswith(".flac"):
        fmt = 0x170000 | SF_FORMAT_PCM_16  # SF_FORMAT_FLAC
    else:
        fmt = SF_FORMAT_OGG | SF_FORMAT_VORBIS
    info = SFInfo(0, rate, 1, fmt, 0, 0)
    h = lib.sf_open(out.encode(), SFM_WRITE, ctypes.byref(info))
    if not h:
        print("sf_open fehlgeschlagen", file=sys.stderr)
        return 1
    n = rate * ms // 1000
    fade = max(1, rate // 100)
    data = (ctypes.c_short * n)()
    for i in range(n):
        env = min(1.0, i / fade, (n - i) / fade)
        data[i] = int(math.sin(2 * math.pi * freq * i / rate) * env * 12000)
    lib.sf_write_short(h, data, n)
    lib.sf_close(h)
    return 0


if __name__ == "__main__":
    sys.exit(main())
