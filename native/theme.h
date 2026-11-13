// Presentation tokens are generated from the central src/theme.rs palette.
#import <AppKit/AppKit.h>
static inline NSColor *PSThemeColor(CGFloat r,CGFloat g,CGFloat b) {
    return [NSColor colorWithSRGBRed:r/255. green:g/255. blue:b/255. alpha:1];
}
#include <theme_palette.h>
