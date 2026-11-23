/* Test stand-in for a fullscreen game in mouse look: a fullscreen window on
 * the given monitor rectangle that holds an active pointer grab for HOLD
 * seconds, then releases it and stays open for STAY seconds.
 * usage: grabwin X Y W H HOLD STAY */
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/Xatom.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>

static void stamp(const char *msg) {
    time_t t = time(NULL);
    char b[32];
    strftime(b, sizeof b, "%H:%M:%S", localtime(&t));
    printf("%s %s\n", b, msg);
    fflush(stdout);
}

int main(int argc, char **argv) {
    if (argc < 7) return 2;
    int x = atoi(argv[1]), y = atoi(argv[2]), w = atoi(argv[3]), h = atoi(argv[4]);
    int hold = atoi(argv[5]), stay = atoi(argv[6]);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 1;
    int s = DefaultScreen(d);
    Window win = XCreateSimpleWindow(d, RootWindow(d, s), x, y, w, h, 0, 0, 0x203040);
    XStoreName(d, win, "PlanetSide 2");
    XClassHint ch = {"steam_app_218230", "steam_app_218230"};
    XSetClassHint(d, win, &ch);
    Atom state = XInternAtom(d, "_NET_WM_STATE", False);
    Atom fs = XInternAtom(d, "_NET_WM_STATE_FULLSCREEN", False);
    XChangeProperty(d, win, state, XA_ATOM, 32, PropModeReplace, (unsigned char *)&fs, 1);
    XSizeHints *sh = XAllocSizeHints();
    sh->flags = PPosition | PSize | USPosition | USSize;
    sh->x = x; sh->y = y; sh->width = w; sh->height = h;
    XSetWMNormalHints(d, win, sh);
    XSelectInput(d, win, ExposureMask | StructureNotifyMask | ButtonPressMask);
    XMapRaised(d, win);
    XEvent ev;
    for (;;) { XNextEvent(d, &ev); if (ev.type == MapNotify) break; }
    XSetInputFocus(d, win, RevertToParent, CurrentTime);
    int tries = 0;
    while (XGrabPointer(d, win, True, ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
                        GrabModeAsync, GrabModeAsync, win, None, CurrentTime) != GrabSuccess) {
        if (++tries > 100) { stamp("grab failed"); return 3; }
        usleep(50000);
    }
    stamp("pointer grabbed");
    time_t end = time(NULL) + hold;
    while (time(NULL) < end) {
        while (XPending(d)) XNextEvent(d, &ev);
        usleep(20000);
    }
    XUngrabPointer(d, CurrentTime);
    XFlush(d);
    stamp("pointer released");
    end = time(NULL) + stay;
    while (time(NULL) < end) {
        while (XPending(d)) XNextEvent(d, &ev);
        usleep(20000);
    }
    stamp("exit");
    return 0;
}
