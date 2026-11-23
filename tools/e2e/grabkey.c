/* Test stand-in for a desktop that owns a shortcut (KWin "Switch to Desktop 2"
 * on Ctrl+F2): grabs the key combination on the root window, with and without
 * NumLock/CapsLock, and keeps it for SECONDS.
 * usage: grabkey KEYSYM SECONDS   (e.g. grabkey F2 300, always with Ctrl) */
#include <X11/Xlib.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "usage: grabkey KEYSYM SECONDS\n");
        return 2;
    }
    Display *d = XOpenDisplay(NULL);
    if (!d) {
        fprintf(stderr, "no display\n");
        return 1;
    }
    KeyCode kc = XKeysymToKeycode(d, XStringToKeysym(argv[1]));
    unsigned int mods[] = {0, Mod2Mask, LockMask, Mod2Mask | LockMask};
    for (int i = 0; i < 4; i++)
        XGrabKey(d, kc, ControlMask | mods[i], DefaultRootWindow(d), True, GrabModeAsync, GrabModeAsync);
    XSync(d, False);
    printf("grabbed Ctrl+%s\n", argv[1]);
    fflush(stdout);
    sleep((unsigned int)atoi(argv[2]));
    XCloseDisplay(d);
    return 0;
}
