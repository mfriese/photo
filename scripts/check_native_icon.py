#!/usr/bin/env python3
"""Validate PhotoSuite's actual AppKit application icon without starting the library app."""
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    if sys.platform != "darwin":
        raise SystemExit("This check requires macOS/AppKit.")
    with tempfile.TemporaryDirectory(prefix="photosuite-icon-") as folder:
        folder = Path(folder)
        source = folder / "check.m"
        source.write_text(r'''
#import <AppKit/AppKit.h>
#include <stdint.h>
#include <math.h>
extern int photosuite_set_application_icon(const uint8_t *, size_t);
int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc != 2) return 1;
        [NSApplication sharedApplication];
        NSData *png = [NSData dataWithContentsOfFile:[NSString stringWithUTF8String:argv[1]]];
        NSImage *expected = [[NSImage alloc] initWithData:png];
        if (!expected.isValid || !photosuite_set_application_icon(png.bytes, png.length)) return 2;
        NSData *actual = NSApp.applicationIconImage.TIFFRepresentation;
        NSBitmapImageRep *actualRep = [NSBitmapImageRep imageRepWithData:actual];
        NSBitmapImageRep *expectedRep = [NSBitmapImageRep imageRepWithData:expected.TIFFRepresentation];
        double error = 0; unsigned samples = 0;
        for (unsigned y = 1; y < 8; ++y) for (unsigned x = 1; x < 8; ++x) {
            NSColor *a = [[actualRep colorAtX:actualRep.pixelsWide*x/8 y:actualRep.pixelsHigh*y/8] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
            NSColor *e = [[expectedRep colorAtX:expectedRep.pixelsWide*x/8 y:expectedRep.pixelsHigh*y/8] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
            if (!a || !e) return 3;
            error += fabs(a.redComponent-e.redComponent) + fabs(a.greenComponent-e.greenComponent) + fabs(a.blueComponent-e.blueComponent) + fabs(a.alphaComponent-e.alphaComponent);
            samples += 4;
        }
        if (error/samples > 0.08) {
            fprintf(stderr,"Dock icon pixel difference: %f\n",error/samples);
            return 3;
        }
        if (photosuite_set_application_icon(NULL, 0)) return 4;
        const uint8_t invalid[] = {1, 2, 3};
        if (photosuite_set_application_icon(invalid, sizeof(invalid))) return 5;
        if (![NSApp.applicationIconImage.TIFFRepresentation isEqualToData:actual]) return 6;
        puts("PASS: AppKit Dock icon matches PhotoSuite PNG; invalid data preserves the icon.");
    }
    return 0;
}
''')
        executable = folder / "check"
        subprocess.run(["clang", "-fobjc-arc", "-framework", "AppKit", str(source),
                        str(ROOT / "native/app_icon.m"), "-o", str(executable)], check=True)
        subprocess.run([str(executable), str(ROOT / "assets/app-icon/photosuite.png")],
                       check=True, timeout=30)


if __name__ == "__main__":
    main()
