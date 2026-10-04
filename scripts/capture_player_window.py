"""Optional Linux/X11 smoke evidence: capture only the SDK's synthetic window.

Requires libX11 and ffmpeg (x11grab). No desktop/root screenshot is taken.
Run while cine-player-spike --visible --gpu-context x11egl is playing.
"""
import ctypes as c
import os
import subprocess
from pathlib import Path


def main():
    x = c.CDLL("libX11.so.6")
    x.XOpenDisplay.argtypes = [c.c_char_p]
    x.XOpenDisplay.restype = c.c_void_p
    x.XDefaultRootWindow.argtypes = [c.c_void_p]
    x.XDefaultRootWindow.restype = c.c_ulong
    x.XQueryTree.argtypes = [c.c_void_p, c.c_ulong, c.POINTER(c.c_ulong), c.POINTER(c.c_ulong),
                            c.POINTER(c.POINTER(c.c_ulong)), c.POINTER(c.c_uint)]
    x.XFetchName.argtypes = [c.c_void_p, c.c_ulong, c.POINTER(c.c_char_p)]
    x.XFree.argtypes = [c.c_void_p]
    x.XGetGeometry.argtypes = [c.c_void_p, c.c_ulong, c.POINTER(c.c_ulong), c.POINTER(c.c_int),
                              c.POINTER(c.c_int), c.POINTER(c.c_uint), c.POINTER(c.c_uint),
                              c.POINTER(c.c_uint), c.POINTER(c.c_uint)]
    x.XCloseDisplay.argtypes = [c.c_void_p]
    display = x.XOpenDisplay(None)
    if not display:
        raise SystemExit("No accessible X11 display")
    root = x.XDefaultRootWindow(display)

    def find(window):
        name = c.c_char_p()
        if x.XFetchName(display, window, c.byref(name)) and name:
            title = name.value.decode(errors="replace")
            x.XFree(name)
            if title == "Cine Virtual - Spike B":
                return window
        tree_root, parent, count = c.c_ulong(), c.c_ulong(), c.c_uint()
        children = c.POINTER(c.c_ulong)()
        if x.XQueryTree(display, window, c.byref(tree_root), c.byref(parent),
                        c.byref(children), c.byref(count)):
            ids = [children[i] for i in range(count.value)]
            if children:
                x.XFree(children)
            for child in ids:
                found = find(child)
                if found:
                    return found
        return None

    try:
        window = find(root)
        if not window:
            raise SystemExit("SDK window not found; no screenshot taken")
        geometry_root, border, depth = c.c_ulong(), c.c_uint(), c.c_uint()
        left, top, width, height = c.c_int(), c.c_int(), c.c_uint(), c.c_uint()
        if not x.XGetGeometry(display, window, c.byref(geometry_root), c.byref(left),
                              c.byref(top), c.byref(width), c.byref(height), c.byref(border), c.byref(depth)):
            raise SystemExit("Cannot query SDK window")
        output = Path("test-media/visible-window.png")
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "x11grab",
                        "-video_size", f"{width.value}x{height.value}", "-window_id", str(window), "-i",
                        os.environ.get("DISPLAY", ":0"),
                        "-frames:v", "1", str(output)], check=True)
        print(f"Captured only SDK client rectangle: {width.value}x{height.value}")
    finally:
        x.XCloseDisplay(display)


if __name__ == "__main__":
    main()
