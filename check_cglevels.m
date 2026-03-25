#import <Cocoa/Cocoa.h>
#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>

int main() {
    NSLog(@"=== Checking CGWindowLevel Constants ===");
    NSLog(@"kCGBaseWindowLevel = %ld", (long)kCGBaseWindowLevel);
    NSLog(@"kCGNormalWindowLevel = %ld", (long)kCGNormalWindowLevel);
    NSLog(@"kCGFloatingWindowLevel = %ld", (long)kCGFloatingWindowLevel);
    NSLog(@"kCGTornOffMenuWindowLevel = %ld", (long)kCGTornOffMenuWindowLevel);
    NSLog(@"kCGMainMenuWindowLevel = %ld", (long)kCGMainMenuWindowLevel);
    NSLog(@"kCGDockWindowLevel = %ld", (long)kCGDockWindowLevel);
    NSLog(@"kCGPopUpMenuWindowLevel = %ld", (long)kCGPopUpMenuWindowLevel);
    NSLog(@"kCGOverlayWindowLevel = %ld", (long)kCGOverlayWindowLevel);
    
    NSLog(@"\n=== Testing Different Levels ===");
    NSPanel *panel = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, 200, 200) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:YES];
    [panel setFloatingPanel:YES];
    
    NSLog(@"Test 1: level = kCGMainMenuWindowLevel (%ld)", (long)kCGMainMenuWindowLevel);
    [panel setLevel:kCGMainMenuWindowLevel];
    NSLog(@"  - Actual panel level: %ld", (long)[panel level]);
    
    NSLog(@"Test 2: level = kCGMainMenuWindowLevel - 1 (%ld)", (long)(kCGMainMenuWindowLevel - 1));
    [panel setLevel:kCGMainMenuWindowLevel - 1];
    NSLog(@"  - Actual panel level: %ld", (long)[panel level]);
    
    NSLog(@"Test 3: level = kCGFloatingWindowLevel (%ld)", (long)kCGFloatingWindowLevel);
    [panel setLevel:kCGFloatingWindowLevel];
    NSLog(@"  - Actual panel level: %ld", (long)[panel level]);
    
    NSLog(@"Test 4: level = kCGTornOffMenuWindowLevel (%ld)", (long)kCGTornOffMenuWindowLevel);
    [panel setLevel:kCGTornOffMenuWindowLevel];
    NSLog(@"  - Actual panel level: %ld", (long)[panel level]);
    
    return 0;
}
